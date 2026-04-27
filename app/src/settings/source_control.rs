use gtk4::prelude::SettingsExt;

use crate::settings::{
    AppSettings, KEY_SOURCE_CONTROL_VIEW_MODE, SettingsBackend, record_memory_write, with_memory,
    with_memory_mut,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceControlViewMode {
    Tree,
    List,
}

impl SourceControlViewMode {
    #[must_use]
    pub fn from_stored(value: &str) -> Self {
        match value {
            "list" => Self::List,
            _ => Self::Tree,
        }
    }

    #[must_use]
    pub const fn stored(self) -> &'static str {
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
            SettingsBackend::GSettings(settings) => SourceControlViewMode::from_stored(
                settings.string(KEY_SOURCE_CONTROL_VIEW_MODE).as_str(),
            ),
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.source_control_view_mode)
            }
        }
    }

    pub(crate) fn set_source_control_view_mode(&self, mode: SourceControlViewMode) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_SOURCE_CONTROL_VIEW_MODE, mode.stored());
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.source_control_view_mode = mode;
                    record_memory_write(state, KEY_SOURCE_CONTROL_VIEW_MODE);
                });
            }
        }
    }
}
