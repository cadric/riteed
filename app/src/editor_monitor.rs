use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

const FILE_POLL_ATTRIBUTES: &str =
    "standard::type,standard::size,time::modified,time::modified-usec,etag::value";
const FILE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const FILE_MISSING_SETTLE_DELAY: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub enum ExternalFileEvent {
    ContentPossiblyChanged,
    Missing,
    Moved { new_file: gio::File },
}

#[derive(Clone, Debug, Default)]
pub enum PendingExternalState {
    #[default]
    Idle,
    ContentPossiblyChanged {
        acknowledged: bool,
    },
    Missing {
        acknowledged: bool,
    },
    Moved {
        new_file: gio::File,
    },
}

impl PendingExternalState {
    #[must_use]
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    #[must_use]
    pub fn is_content_changed(&self) -> bool {
        matches!(self, Self::ContentPossiblyChanged { .. })
    }

    #[must_use]
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }

    #[must_use]
    pub fn is_acknowledged(&self) -> bool {
        matches!(
            self,
            Self::ContentPossiblyChanged { acknowledged: true }
                | Self::Missing { acknowledged: true }
        )
    }

    pub fn acknowledge(&mut self) {
        match self {
            Self::ContentPossiblyChanged { acknowledged } | Self::Missing { acknowledged } => {
                *acknowledged = true;
            }
            Self::Idle | Self::Moved { .. } => {}
        }
    }
}

pub struct MonitorBinding {
    monitor: gio::FileMonitor,
    poll_source: RefCell<Option<glib::SourceId>>,
    poll_cancellable: gio::Cancellable,
    poll_cancelled: Rc<Cell<bool>>,
    target_uri: String,
}

impl MonitorBinding {
    /// # Errors
    ///
    /// Returns an error when Gio cannot create a monitor for the local file.
    pub fn new(
        file: &gio::File,
        on_event: Rc<dyn Fn(ExternalFileEvent)>,
    ) -> Result<Self, glib::Error> {
        let monitor = file.monitor_file(
            gio::FileMonitorFlags::WATCH_MOVES | gio::FileMonitorFlags::WATCH_HARD_LINKS,
            None::<&gio::Cancellable>,
        )?;
        monitor.set_rate_limit(250);
        let on_event_for_monitor = on_event.clone();
        let use_polling = uses_document_portal(file);
        let poll_state = Rc::new(RefCell::new(if use_polling {
            current_file_stamp(file)
        } else {
            None
        }));
        let poll_cancellable = gio::Cancellable::new();
        let poll_cancelled = Rc::new(Cell::new(false));
        let poll_source = if use_polling {
            Some(start_file_poll(
                file,
                poll_state.clone(),
                on_event,
                &poll_cancellable,
                poll_cancelled.clone(),
            ))
        } else {
            drop(on_event);
            None
        };
        let queued_change = Rc::new(Cell::new(false));
        let monitored_file = file.clone();
        monitor.connect_changed({
            let queued_change = queued_change.clone();
            let poll_state = poll_state.clone();
            let poll_cancelled = poll_cancelled.clone();
            move |_, _file, other_file, event_type| {
                let Some(event) = normalize_monitor_event(other_file, event_type) else {
                    return;
                };
                match event {
                    ExternalFileEvent::ContentPossiblyChanged => {
                        if queued_change.replace(true) {
                            return;
                        }
                        let queued_change = queued_change.clone();
                        let on_event = on_event_for_monitor.clone();
                        glib::idle_add_local_once(move || {
                            queued_change.set(false);
                            on_event(ExternalFileEvent::ContentPossiblyChanged);
                        });
                    }
                    ExternalFileEvent::Missing => {
                        let file = monitored_file.clone();
                        let poll_state = poll_state.clone();
                        let poll_cancelled = poll_cancelled.clone();
                        let on_event = on_event_for_monitor.clone();
                        let _source =
                            glib::timeout_add_local_once(FILE_MISSING_SETTLE_DELAY, move || {
                                if poll_cancelled.get() {
                                    return;
                                }
                                let next = current_file_stamp(&file);
                                if next.is_some() {
                                    *poll_state.borrow_mut() = next;
                                    on_event(ExternalFileEvent::ContentPossiblyChanged);
                                } else {
                                    *poll_state.borrow_mut() = None;
                                    on_event(ExternalFileEvent::Missing);
                                }
                            });
                    }
                    moved @ ExternalFileEvent::Moved { .. } => on_event_for_monitor(moved),
                }
            }
        });

        Ok(Self {
            monitor,
            poll_source: RefCell::new(poll_source),
            poll_cancellable,
            poll_cancelled,
            target_uri: file.uri().to_string(),
        })
    }

    pub fn cancel(&self) {
        self.poll_cancelled.set(true);
        self.poll_cancellable.cancel();
        if let Some(source) = self.poll_source.borrow_mut().take() {
            source.remove();
        }
        let _cancelled = self.monitor.cancel();
    }

    #[must_use]
    pub fn target_uri(&self) -> &str {
        &self.target_uri
    }
}

impl Drop for MonitorBinding {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileStamp {
    file_type: gio::FileType,
    size: i64,
    modified: u64,
    modified_usec: u32,
    etag: Option<String>,
}

impl FileStamp {
    fn from_info(info: &gio::FileInfo) -> Self {
        Self {
            file_type: info.file_type(),
            size: info.size(),
            modified: info.attribute_uint64("time::modified"),
            modified_usec: info.attribute_uint32("time::modified-usec"),
            etag: info
                .attribute_string("etag::value")
                .map(|etag| etag.to_string()),
        }
    }
}

fn current_file_stamp(file: &gio::File) -> Option<FileStamp> {
    file.query_info(
        FILE_POLL_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        None::<&gio::Cancellable>,
    )
    .ok()
    .map(|info| FileStamp::from_info(&info))
}

fn uses_document_portal(file: &gio::File) -> bool {
    file.path().is_some_and(|path| {
        let path = path.to_string_lossy();
        path.starts_with("/run/flatpak/doc/")
            || (path.starts_with("/run/user/") && path.contains("/doc/"))
    })
}

fn start_file_poll(
    file: &gio::File,
    poll_state: Rc<RefCell<Option<FileStamp>>>,
    on_event: Rc<dyn Fn(ExternalFileEvent)>,
    cancellable: &gio::Cancellable,
    cancelled: Rc<Cell<bool>>,
) -> glib::SourceId {
    let file = file.clone();
    let cancellable = cancellable.clone();
    let query_in_flight = Rc::new(Cell::new(false));
    glib::timeout_add_local(FILE_POLL_INTERVAL, move || {
        if cancelled.get() {
            return glib::ControlFlow::Break;
        }
        if query_in_flight.replace(true) {
            return glib::ControlFlow::Continue;
        }
        query_file_stamp(
            &file,
            poll_state.clone(),
            on_event.clone(),
            &cancellable,
            cancelled.clone(),
            query_in_flight.clone(),
        );
        glib::ControlFlow::Continue
    })
}

fn query_file_stamp(
    file: &gio::File,
    poll_state: Rc<RefCell<Option<FileStamp>>>,
    on_event: Rc<dyn Fn(ExternalFileEvent)>,
    cancellable: &gio::Cancellable,
    cancelled: Rc<Cell<bool>>,
    query_in_flight: Rc<Cell<bool>>,
) {
    file.query_info_async(
        FILE_POLL_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        Some(cancellable),
        move |result| {
            query_in_flight.set(false);
            if cancelled.get() {
                return;
            }
            let next = result.ok().map(|info| FileStamp::from_info(&info));
            if let Some(event) = apply_file_stamp_result(&poll_state, next) {
                on_event(event);
            }
        },
    );
}

fn apply_file_stamp_result(
    poll_state: &Rc<RefCell<Option<FileStamp>>>,
    next: Option<FileStamp>,
) -> Option<ExternalFileEvent> {
    let mut state = poll_state.borrow_mut();
    match (&*state, &next) {
        (Some(previous), Some(current)) if previous != current => {
            *state = next;
            Some(ExternalFileEvent::ContentPossiblyChanged)
        }
        (None, Some(_)) => {
            *state = next;
            Some(ExternalFileEvent::ContentPossiblyChanged)
        }
        (Some(_), None) => {
            *state = None;
            Some(ExternalFileEvent::Missing)
        }
        (None, None) | (Some(_), Some(_)) => None,
    }
}

#[must_use]
pub fn normalize_monitor_event(
    _other_file: Option<&gio::File>,
    event_type: gio::FileMonitorEvent,
) -> Option<ExternalFileEvent> {
    match event_type {
        // Atomic-save workflows commonly replace the file through a rename, so
        // monitor move/rename notifications still need the normal stale/reload flow.
        gio::FileMonitorEvent::Changed
        | gio::FileMonitorEvent::ChangesDoneHint
        | gio::FileMonitorEvent::Moved
        | gio::FileMonitorEvent::Renamed
        | gio::FileMonitorEvent::Created
        | gio::FileMonitorEvent::MovedIn => Some(ExternalFileEvent::ContentPossiblyChanged),
        gio::FileMonitorEvent::Deleted
        | gio::FileMonitorEvent::MovedOut
        | gio::FileMonitorEvent::PreUnmount
        | gio::FileMonitorEvent::Unmounted => Some(ExternalFileEvent::Missing),
        _ => None,
    }
}

#[must_use]
pub fn next_pending_state(
    current: &PendingExternalState,
    event: ExternalFileEvent,
) -> PendingExternalState {
    match event {
        ExternalFileEvent::ContentPossiblyChanged => PendingExternalState::ContentPossiblyChanged {
            acknowledged: false,
        },
        ExternalFileEvent::Missing => PendingExternalState::Missing {
            acknowledged: false,
        },
        ExternalFileEvent::Moved { new_file } => match current {
            PendingExternalState::Missing { .. } => PendingExternalState::Missing {
                acknowledged: false,
            },
            PendingExternalState::Idle
            | PendingExternalState::ContentPossiblyChanged { .. }
            | PendingExternalState::Moved { .. } => PendingExternalState::Moved { new_file },
        },
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{
        ExternalFileEvent, FileStamp, PendingExternalState, apply_file_stamp_result,
        next_pending_state, normalize_monitor_event, uses_document_portal,
    };
    use gtk4::gio;

    #[test]
    fn move_without_other_file_degrades_to_content_change() {
        let event = normalize_monitor_event(None, gio::FileMonitorEvent::Moved);
        assert!(matches!(
            event,
            Some(ExternalFileEvent::ContentPossiblyChanged)
        ));
    }

    #[test]
    fn rename_with_other_file_still_treated_as_content_change() {
        let event = normalize_monitor_event(
            Some(&gio::File::for_path("/tmp/replaced.txt")),
            gio::FileMonitorEvent::Renamed,
        );
        assert!(matches!(
            event,
            Some(ExternalFileEvent::ContentPossiblyChanged)
        ));
    }

    #[test]
    fn pre_unmount_is_missing() {
        let event = normalize_monitor_event(None, gio::FileMonitorEvent::PreUnmount);
        assert!(matches!(event, Some(ExternalFileEvent::Missing)));
    }

    #[test]
    fn created_is_treated_as_content_change() {
        let event = normalize_monitor_event(None, gio::FileMonitorEvent::Created);
        assert!(matches!(
            event,
            Some(ExternalFileEvent::ContentPossiblyChanged)
        ));
    }

    #[test]
    fn content_change_resets_acknowledged_state() {
        let current = PendingExternalState::ContentPossiblyChanged { acknowledged: true };
        let next = next_pending_state(&current, ExternalFileEvent::ContentPossiblyChanged);
        assert!(matches!(
            next,
            PendingExternalState::ContentPossiblyChanged {
                acknowledged: false
            }
        ));
    }

    #[test]
    fn polling_detects_file_stamp_change() {
        let state = Rc::new(RefCell::new(Some(file_stamp(1, 12))));
        let event = apply_file_stamp_result(&state, Some(file_stamp(2, 12)));
        assert!(matches!(
            event,
            Some(ExternalFileEvent::ContentPossiblyChanged)
        ));
        assert_eq!(*state.borrow(), Some(file_stamp(2, 12)));
    }

    #[test]
    fn polling_detects_missing_file() {
        let state = Rc::new(RefCell::new(Some(file_stamp(1, 12))));
        let event = apply_file_stamp_result(&state, None);
        assert!(matches!(event, Some(ExternalFileEvent::Missing)));
        assert_eq!(*state.borrow(), None);
    }

    #[test]
    fn polling_detects_recreated_file() {
        let state = Rc::new(RefCell::new(None));
        let event = apply_file_stamp_result(&state, Some(file_stamp(1, 12)));
        assert!(matches!(
            event,
            Some(ExternalFileEvent::ContentPossiblyChanged)
        ));
        assert_eq!(*state.borrow(), Some(file_stamp(1, 12)));
    }

    #[test]
    fn portal_paths_enable_polling_fallback() {
        assert!(uses_document_portal(&gio::File::for_path(
            "/run/user/1000/doc/abc/file.txt"
        )));
        assert!(uses_document_portal(&gio::File::for_path(
            "/run/flatpak/doc/abc/file.txt"
        )));
        assert!(!uses_document_portal(&gio::File::for_path("/tmp/file.txt")));
    }

    #[test]
    fn missing_takes_precedence_over_move() {
        let current = PendingExternalState::Missing {
            acknowledged: false,
        };
        let next = next_pending_state(
            &current,
            ExternalFileEvent::Moved {
                new_file: gio::File::for_path("/tmp/new.txt"),
            },
        );
        assert!(matches!(
            next,
            PendingExternalState::Missing {
                acknowledged: false
            }
        ));
    }

    fn file_stamp(modified: u64, size: i64) -> FileStamp {
        FileStamp {
            file_type: gio::FileType::Regular,
            size,
            modified,
            modified_usec: 0,
            etag: None,
        }
    }
}
