use std::cell::Cell;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

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

#[derive(Clone)]
pub struct MonitorBinding {
    monitor: gio::FileMonitor,
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
        let queued_change = Rc::new(Cell::new(false));
        monitor.connect_changed({
            let queued_change = queued_change.clone();
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
                        let on_event = on_event.clone();
                        glib::idle_add_local_once(move || {
                            queued_change.set(false);
                            on_event(ExternalFileEvent::ContentPossiblyChanged);
                        });
                    }
                    other => on_event(other),
                }
            }
        });

        Ok(Self {
            monitor,
            target_uri: file.uri().to_string(),
        })
    }

    pub fn cancel(&self) {
        let _cancelled = self.monitor.cancel();
    }

    #[must_use]
    pub fn target_uri(&self) -> &str {
        &self.target_uri
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
    use super::{
        ExternalFileEvent, PendingExternalState, next_pending_state, normalize_monitor_event,
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
