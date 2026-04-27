use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk4::{gio, glib, prelude::*};

use crate::source_control::{SourceControlState, SourceStateRef};

use super::refresh::refresh_status;

const DEBOUNCE: Duration = Duration::from_millis(250);
const MIN_INTERVAL: Duration = Duration::from_secs(1);
const PORTAL_POLL: Duration = Duration::from_secs(4);

pub(crate) struct SourceControlLiveRefresh {
    repo: PathBuf,
    head_monitor: Option<gio::FileMonitor>,
    index_monitor: Option<gio::FileMonitor>,
    poll_source: Option<glib::SourceId>,
    scheduler: LiveScheduler,
}

impl SourceControlLiveRefresh {
    pub(super) fn new(repo: &Path, on_refresh: Rc<dyn Fn()>) -> Self {
        let scheduler = LiveScheduler::new(repo.to_path_buf(), on_refresh);
        let (head_monitor, index_monitor, poll_source) = if use_polling(repo) {
            (None, None, Some(start_polling(&scheduler)))
        } else {
            (
                monitor_file(&repo.join(".git/HEAD"), &scheduler),
                monitor_file(&repo.join(".git/index"), &scheduler),
                None,
            )
        };
        Self {
            repo: repo.to_path_buf(),
            head_monitor,
            index_monitor,
            poll_source,
            scheduler,
        }
    }

    pub(super) fn schedule(&self) {
        self.scheduler.schedule();
    }

    pub(super) fn index_lock_exists(&self) -> bool {
        self.repo.join(".git/index.lock").exists()
    }

    pub(super) fn cancel(&mut self) {
        if let Some(source) = self.poll_source.take() {
            source.remove();
        }
        if let Some(monitor) = self.head_monitor.take() {
            let _cancelled = monitor.cancel();
        }
        if let Some(monitor) = self.index_monitor.take() {
            let _cancelled = monitor.cancel();
        }
    }
}

pub(super) fn install(state: &SourceStateRef) {
    cancel(state);
    let Some(repo) = state.borrow().repo.clone() else {
        return;
    };
    let weak = Rc::downgrade(state);
    let live = SourceControlLiveRefresh::new(
        &repo,
        Rc::new(move || {
            if let Some(state) = weak.upgrade() {
                refresh_status(&state);
            }
        }),
    );
    state.borrow_mut().live_refresh = Some(live);
}

pub(super) fn cancel(state: &SourceStateRef) {
    if let Some(mut live) = state.borrow_mut().live_refresh.take() {
        live.cancel();
    }
}

pub(super) fn index_lock_exists(state: &SourceStateRef) -> bool {
    state
        .borrow()
        .live_refresh
        .as_ref()
        .is_some_and(SourceControlLiveRefresh::index_lock_exists)
}

pub(super) fn schedule(state: &SourceStateRef) {
    if let Some(live) = state.borrow().live_refresh.as_ref() {
        live.schedule();
    }
}

pub(super) fn saved_file_in_repo(state: &SourceControlState, file: &gio::File) -> bool {
    let Some(repo) = state.repo.as_ref() else {
        return false;
    };
    file.path().is_some_and(|path| path.starts_with(repo))
}

impl Drop for SourceControlLiveRefresh {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone)]
struct LiveScheduler {
    repo: PathBuf,
    pending: Rc<Cell<bool>>,
    last_refresh: Rc<RefCell<Option<Instant>>>,
    on_refresh: Rc<dyn Fn()>,
}

impl LiveScheduler {
    fn new(repo: PathBuf, on_refresh: Rc<dyn Fn()>) -> Self {
        Self {
            repo,
            pending: Rc::new(Cell::new(false)),
            last_refresh: Rc::new(RefCell::new(None)),
            on_refresh,
        }
    }

    fn schedule(&self) {
        if self.pending.replace(true) {
            return;
        }
        let delay = self.delay();
        let scheduler = self.clone();
        glib::timeout_add_local_once(delay, move || scheduler.fire());
    }

    fn fire(&self) {
        self.pending.set(false);
        if self.repo.join(".git/index.lock").exists() {
            self.schedule();
            return;
        }
        *self.last_refresh.borrow_mut() = Some(Instant::now());
        (self.on_refresh)();
    }

    fn delay(&self) -> Duration {
        let Some(last) = *self.last_refresh.borrow() else {
            return DEBOUNCE;
        };
        let elapsed = last.elapsed();
        if elapsed >= MIN_INTERVAL {
            DEBOUNCE
        } else {
            MIN_INTERVAL.saturating_sub(elapsed).max(DEBOUNCE)
        }
    }
}

fn monitor_file(path: &Path, scheduler: &LiveScheduler) -> Option<gio::FileMonitor> {
    let file = gio::File::for_path(path);
    let monitor = file
        .monitor_file(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
        .ok()?;
    let scheduler = scheduler.clone();
    monitor.connect_changed(move |_, _file, _other, event| {
        if refresh_event(event) {
            scheduler.schedule();
        }
    });
    Some(monitor)
}

fn start_polling(scheduler: &LiveScheduler) -> glib::SourceId {
    let scheduler = scheduler.clone();
    glib::timeout_add_local(PORTAL_POLL, move || {
        scheduler.schedule();
        glib::ControlFlow::Continue
    })
}

fn use_polling(repo: &Path) -> bool {
    let file = gio::File::for_path(repo);
    !file.is_native() || document_portal_path(repo)
}

fn document_portal_path(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.starts_with("/run/flatpak/doc/")
        || (path.starts_with("/run/user/") && path.contains("/doc/"))
}

fn refresh_event(event: gio::FileMonitorEvent) -> bool {
    matches!(
        event,
        gio::FileMonitorEvent::Changed
            | gio::FileMonitorEvent::ChangesDoneHint
            | gio::FileMonitorEvent::Created
            | gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::Moved
            | gio::FileMonitorEvent::Renamed
    )
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::Instant;

    use gtk4::gio;

    use super::{DEBOUNCE, LiveScheduler, document_portal_path, refresh_event};

    #[test]
    fn native_git_events_trigger_refresh() {
        assert!(refresh_event(gio::FileMonitorEvent::Changed));
        assert!(refresh_event(gio::FileMonitorEvent::ChangesDoneHint));
        assert!(!refresh_event(gio::FileMonitorEvent::PreUnmount));
    }

    #[test]
    fn document_portal_paths_use_polling() {
        assert!(document_portal_path(std::path::Path::new(
            "/run/flatpak/doc/abc"
        )));
        assert!(document_portal_path(std::path::Path::new(
            "/run/user/1000/doc/abc"
        )));
        assert!(!document_portal_path(std::path::Path::new("/tmp/repo")));
    }

    #[test]
    fn live_scheduler_delay_uses_debounce_and_minimum_interval() {
        let scheduler = LiveScheduler::new(std::path::PathBuf::from("/tmp/repo"), Rc::new(|| {}));
        assert_eq!(scheduler.delay(), DEBOUNCE);

        *scheduler.last_refresh.borrow_mut() = Some(Instant::now());
        assert!(scheduler.delay() >= DEBOUNCE);
    }
}
