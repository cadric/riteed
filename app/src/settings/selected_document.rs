use gtk4::prelude::SettingsExt;

use super::{AppSettings, SettingsBackend};
#[cfg(test)]
use super::{record_memory_write, with_memory, with_memory_mut};

const KEY_SESSION_SELECTED_FILE: &str = "session-selected-file";

impl AppSettings {
    #[must_use]
    pub fn session_selected_file(&self) -> String {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.string(KEY_SESSION_SELECTED_FILE).to_string()
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
                state.selected_document.session_selected_file.clone()
            }),
        }
    }

    pub fn set_session_selected_file(&self, uri: &str) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_SESSION_SELECTED_FILE, uri);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.selected_document.session_selected_file = String::from(uri);
                    record_memory_write(state, KEY_SESSION_SELECTED_FILE);
                });
            }
        }
    }
}
