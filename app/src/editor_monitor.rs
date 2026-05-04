use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

mod stamp;
#[cfg(test)]
mod stamp_tests;

use stamp::{StampPurpose, StampTracker};

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
    stamp_tracker: Rc<StampTracker>,
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
        let stamp_tracker = StampTracker::new(file, on_event);
        stamp_tracker.queue(StampPurpose::Baseline);
        let poll_source = if use_polling {
            Some(start_file_poll(stamp_tracker.clone()))
        } else {
            None
        };
        let queued_change = Rc::new(Cell::new(false));
        monitor.connect_changed({
            let queued_change = queued_change.clone();
            let stamp_tracker = stamp_tracker.clone();
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
                        let stamp_tracker = stamp_tracker.clone();
                        glib::idle_add_local_once(move || {
                            queued_change.set(false);
                            if stamp_tracker.is_cancelled() {
                                return;
                            }
                            stamp_tracker.queue(StampPurpose::Change);
                        });
                    }
                    ExternalFileEvent::Missing => {
                        let stamp_tracker = stamp_tracker.clone();
                        let _source =
                            glib::timeout_add_local_once(FILE_MISSING_SETTLE_DELAY, move || {
                                if stamp_tracker.is_cancelled() {
                                    return;
                                }
                                stamp_tracker.queue(StampPurpose::MissingSettle);
                            });
                    }
                    moved @ ExternalFileEvent::Moved { .. } => on_event_for_monitor(moved),
                }
            }
        });

        Ok(Self {
            monitor,
            poll_source: RefCell::new(poll_source),
            stamp_tracker,
            target_uri: file.uri().to_string(),
        })
    }

    pub fn cancel(&self) {
        self.stamp_tracker.cancel();
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

fn uses_document_portal(file: &gio::File) -> bool {
    file.path().is_some_and(|path| {
        let path = path.to_string_lossy();
        path.starts_with("/run/flatpak/doc/")
            || (path.starts_with("/run/user/") && path.contains("/doc/"))
    })
}

fn start_file_poll(stamp_tracker: Rc<StampTracker>) -> glib::SourceId {
    glib::timeout_add_local(FILE_POLL_INTERVAL, move || {
        if stamp_tracker.is_cancelled() {
            return glib::ControlFlow::Break;
        }
        stamp_tracker.queue(StampPurpose::Poll);
        glib::ControlFlow::Continue
    })
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
    use super::{
        ExternalFileEvent, PendingExternalState, next_pending_state, normalize_monitor_event,
        uses_document_portal,
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
}
