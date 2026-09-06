use gtk4::prelude::SettingsExt;

use crate::settings::{AppSettings, SettingsBackend};
#[cfg(test)]
use crate::settings::{record_memory_write, with_memory, with_memory_mut};

const KEY_SOURCE_CONTROL_VIEW_MODE: &str = "source-control-view-mode";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceControlViewMode {
    Tree,
    List,
}

impl SourceControlViewMode {
    pub const ALL: [Self; 2] = [Self::Tree, Self::List];

    #[must_use]
    pub const fn enum_value(self) -> i32 {
        match self {
            Self::Tree => 0,
            Self::List => 1,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::List,
            _ => Self::Tree,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::List => "list",
        }
    }
}

impl AppSettings {
    #[must_use]
    pub(crate) fn source_control_view_mode(&self) -> SourceControlViewMode {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                SourceControlViewMode::from_enum_value(settings.enum_(KEY_SOURCE_CONTROL_VIEW_MODE))
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.source_control_view_mode)
            }
        }
    }

    pub(crate) fn set_source_control_view_mode(&self, mode: SourceControlViewMode) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_enum(KEY_SOURCE_CONTROL_VIEW_MODE, mode.enum_value());
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.source_control_view_mode = mode;
                    record_memory_write(state, KEY_SOURCE_CONTROL_VIEW_MODE);
                });
            }
        }
    }
}
