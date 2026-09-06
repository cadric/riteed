use gtk4::prelude::SettingsExt;

use super::{AppSettings, SettingsBackend};
#[cfg(test)]
use super::{record_memory_write, with_memory, with_memory_mut};

const KEY_EDITOR_FONT: &str = "editor-font";

impl AppSettings {
    #[must_use]
    pub fn editor_font(&self) -> String {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.string(KEY_EDITOR_FONT).to_string(),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.editor_font.clone())
            }
        }
    }

    pub fn set_editor_font(&self, font: &str) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_EDITOR_FONT, font);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.editor_font = String::from(font);
                    record_memory_write(state, KEY_EDITOR_FONT);
                });
            }
        }
    }
}
