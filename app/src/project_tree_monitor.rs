use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

const MONITOR_RATE_LIMIT_MS: i32 = 250;
const DIRECTORY_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DIRECTORY_POLL_ATTRIBUTES: &str = "standard::name,standard::type";
const DIRECTORY_POLL_BATCH_SIZE: i32 = 200;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectDirectorySnapshot {
    entries: Vec<ProjectDirectorySnapshotEntry>,
}

impl ProjectDirectorySnapshot {
    #[must_use]
    pub(crate) fn from_infos(infos: &[gio::FileInfo], show_hidden: bool) -> Self {
        let mut entries = Vec::new();
        for info in infos {
            let name = info.name().to_string_lossy().to_string();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            let Some(kind) = snapshot_kind(info.file_type()) else {
                continue;
            };
            entries.push(ProjectDirectorySnapshotEntry { name, kind });
        }
        entries.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.kind.cmp(&right.kind))
        });
        Self { entries }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ProjectDirectorySnapshotEntry {
    name: String,
    kind: u8,
}

pub(crate) struct ProjectDirectoryMonitor {
    monitor: gio::FileMonitor,
    poll_source: RefCell<Option<glib::SourceId>>,
    poll_cancellable: gio::Cancellable,
    poll_cancelled: Rc<Cell<bool>>,
}

impl ProjectDirectoryMonitor {
    pub(crate) fn new(
        directory: &gio::File,
        initial_snapshot: ProjectDirectorySnapshot,
        show_hidden: bool,
        on_structural_change: Rc<dyn Fn()>,
    ) -> Result<Self, glib::Error> {
        let monitor = directory.monitor_directory(
            gio::FileMonitorFlags::WATCH_MOVES,
            None::<&gio::Cancellable>,
        )?;
        monitor.set_rate_limit(MONITOR_RATE_LIMIT_MS);
        let poll_state = Rc::new(RefCell::new(Some(initial_snapshot)));
        let poll_cancellable = gio::Cancellable::new();
        let poll_cancelled = Rc::new(Cell::new(false));
        let poll_source = if uses_document_portal(directory) {
            Some(start_directory_poll(
                directory,
                show_hidden,
                poll_state,
                on_structural_change.clone(),
                &poll_cancellable,
                poll_cancelled.clone(),
            ))
        } else {
            None
        };
        monitor.connect_changed(move |_, _file, _other_file, event_type| {
            if normalize_project_tree_event(event_type) {
                on_structural_change();
            }
        });
        Ok(Self {
            monitor,
            poll_source: RefCell::new(poll_source),
            poll_cancellable,
            poll_cancelled,
        })
    }

    pub(crate) fn cancel(&self) {
        self.poll_cancelled.set(true);
        self.poll_cancellable.cancel();
        if let Some(source) = self.poll_source.borrow_mut().take() {
            source.remove();
        }
        let _cancelled = self.monitor.cancel();
    }
}

impl Drop for ProjectDirectoryMonitor {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn start_directory_poll(
    directory: &gio::File,
    show_hidden: bool,
    poll_state: Rc<RefCell<Option<ProjectDirectorySnapshot>>>,
    on_structural_change: Rc<dyn Fn()>,
    cancellable: &gio::Cancellable,
    cancelled: Rc<Cell<bool>>,
) -> glib::SourceId {
    let directory = directory.clone();
    let cancellable = cancellable.clone();
    let query_in_flight = Rc::new(Cell::new(false));
    glib::timeout_add_local(DIRECTORY_POLL_INTERVAL, move || {
        if cancelled.get() {
            return glib::ControlFlow::Break;
        }
        if query_in_flight.replace(true) {
            return glib::ControlFlow::Continue;
        }
        query_directory_snapshot(
            &directory,
            show_hidden,
            DirectoryPoll {
                poll_state: poll_state.clone(),
                on_structural_change: on_structural_change.clone(),
                cancellable: cancellable.clone(),
                cancelled: cancelled.clone(),
                query_in_flight: query_in_flight.clone(),
            },
        );
        glib::ControlFlow::Continue
    })
}

fn uses_document_portal(file: &gio::File) -> bool {
    file.path().is_some_and(|path| {
        let path = path.to_string_lossy();
        path.starts_with("/run/flatpak/doc/")
            || (path.starts_with("/run/user/") && path.contains("/doc/"))
    })
}

#[derive(Clone)]
struct DirectoryPoll {
    poll_state: Rc<RefCell<Option<ProjectDirectorySnapshot>>>,
    on_structural_change: Rc<dyn Fn()>,
    cancellable: gio::Cancellable,
    cancelled: Rc<Cell<bool>>,
    query_in_flight: Rc<Cell<bool>>,
}

fn query_directory_snapshot(directory: &gio::File, show_hidden: bool, poll: DirectoryPoll) {
    let directory = directory.clone();
    let cancellable = poll.cancellable.clone();
    directory.enumerate_children_async(
        DIRECTORY_POLL_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        Some(&cancellable),
        move |result| match result {
            Ok(enumerator) => {
                collect_directory_snapshot(&enumerator, show_hidden, &poll, Vec::new());
            }
            Err(_) => finish_directory_snapshot(&poll, None),
        },
    );
}

fn collect_directory_snapshot(
    enumerator: &gio::FileEnumerator,
    show_hidden: bool,
    poll: &DirectoryPoll,
    mut infos: Vec<gio::FileInfo>,
) {
    let poll_for_callback = poll.clone();
    let enumerator_for_next = enumerator.clone();
    enumerator.next_files_async(
        DIRECTORY_POLL_BATCH_SIZE,
        glib::Priority::default(),
        Some(&poll.cancellable),
        move |result| match result {
            Ok(batch) => {
                if batch.is_empty() {
                    finish_directory_snapshot(
                        &poll_for_callback,
                        Some(ProjectDirectorySnapshot::from_infos(&infos, show_hidden)),
                    );
                    return;
                }
                infos.extend(batch);
                collect_directory_snapshot(
                    &enumerator_for_next,
                    show_hidden,
                    &poll_for_callback,
                    infos,
                );
            }
            Err(_) => finish_directory_snapshot(&poll_for_callback, None),
        },
    );
}

fn finish_directory_snapshot(
    poll: &DirectoryPoll,
    next_snapshot: Option<ProjectDirectorySnapshot>,
) {
    poll.query_in_flight.set(false);
    if poll.cancelled.get() {
        return;
    }
    if apply_directory_snapshot_result(&poll.poll_state, next_snapshot) {
        (poll.on_structural_change)();
    }
}

fn apply_directory_snapshot_result(
    poll_state: &Rc<RefCell<Option<ProjectDirectorySnapshot>>>,
    next_snapshot: Option<ProjectDirectorySnapshot>,
) -> bool {
    let mut state = poll_state.borrow_mut();
    match (&*state, &next_snapshot) {
        (Some(previous), Some(current)) if previous != current => {
            *state = next_snapshot;
            true
        }
        (None, Some(_)) | (Some(_), None) => {
            *state = next_snapshot;
            true
        }
        (None, None) | (Some(_), Some(_)) => false,
    }
}

#[must_use]
pub(crate) fn normalize_project_tree_event(event_type: gio::FileMonitorEvent) -> bool {
    matches!(
        event_type,
        gio::FileMonitorEvent::Created
            | gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::MovedIn
            | gio::FileMonitorEvent::MovedOut
            | gio::FileMonitorEvent::Renamed
            | gio::FileMonitorEvent::Moved
    )
}

fn snapshot_kind(file_type: gio::FileType) -> Option<u8> {
    match file_type {
        gio::FileType::Directory => Some(0),
        gio::FileType::Regular => Some(1),
        gio::FileType::SymbolicLink => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gtk4::gio;

    use super::{
        ProjectDirectorySnapshot, apply_directory_snapshot_result, normalize_project_tree_event,
        uses_document_portal,
    };

    #[test]
    fn structural_events_refresh_project_tree() {
        for event in [
            gio::FileMonitorEvent::Created,
            gio::FileMonitorEvent::Deleted,
            gio::FileMonitorEvent::MovedIn,
            gio::FileMonitorEvent::MovedOut,
            gio::FileMonitorEvent::Renamed,
            gio::FileMonitorEvent::Moved,
        ] {
            assert!(normalize_project_tree_event(event));
        }
    }

    #[test]
    fn non_structural_events_do_not_refresh_project_tree() {
        for event in [
            gio::FileMonitorEvent::Changed,
            gio::FileMonitorEvent::ChangesDoneHint,
            gio::FileMonitorEvent::AttributeChanged,
            gio::FileMonitorEvent::PreUnmount,
            gio::FileMonitorEvent::Unmounted,
        ] {
            assert!(!normalize_project_tree_event(event));
        }
    }

    #[test]
    fn directory_snapshot_filters_hidden_entries() {
        let visible = file_info("visible.txt", gio::FileType::Regular);
        let hidden = file_info(".hidden", gio::FileType::Regular);
        let snapshot = ProjectDirectorySnapshot::from_infos(&[visible, hidden], false);
        assert_eq!(snapshot.entries.len(), 1);
        assert_eq!(snapshot.entries[0].name, "visible.txt");
    }

    #[test]
    fn polling_snapshot_change_requests_refresh() {
        let initial = ProjectDirectorySnapshot::from_infos(
            &[file_info("old.txt", gio::FileType::Regular)],
            false,
        );
        let changed = ProjectDirectorySnapshot::from_infos(
            &[file_info("new.txt", gio::FileType::Regular)],
            false,
        );
        let state = Rc::new(RefCell::new(Some(initial)));
        assert!(apply_directory_snapshot_result(
            &state,
            Some(changed.clone())
        ));
        assert_eq!(*state.borrow(), Some(changed));
        let unchanged = state.borrow().clone();
        assert!(!apply_directory_snapshot_result(&state, unchanged));
    }

    #[test]
    fn polling_snapshot_appearing_requests_refresh() {
        let changed = ProjectDirectorySnapshot::from_infos(
            &[file_info("new.txt", gio::FileType::Regular)],
            false,
        );
        let state = Rc::new(RefCell::new(None));
        assert!(apply_directory_snapshot_result(
            &state,
            Some(changed.clone())
        ));
        assert_eq!(*state.borrow(), Some(changed));
    }

    #[test]
    fn portal_paths_enable_directory_polling_fallback() {
        assert!(uses_document_portal(&gio::File::for_path(
            "/run/user/1000/doc/abc"
        )));
        assert!(uses_document_portal(&gio::File::for_path(
            "/run/flatpak/doc/abc"
        )));
        assert!(!uses_document_portal(&gio::File::for_path("/tmp/abc")));
    }

    fn file_info(name: &str, file_type: gio::FileType) -> gio::FileInfo {
        let info = gio::FileInfo::new();
        info.set_name(name);
        info.set_file_type(file_type);
        info
    }
}
