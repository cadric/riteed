use gtk4::prelude::*;

use super::{AppSettings, SettingsBackend};

const KEY_WORD_WRAP: &str = "word-wrap";
const KEY_SHOW_LINE_NUMBERS: &str = "show-line-numbers";
const KEY_SHOW_MINIMAP: &str = "show-minimap";

impl AppSettings {
    #[must_use]
    pub fn word_wrap(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_WORD_WRAP),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.display.word_wrap)
            }
        }
    }

    pub fn set_word_wrap(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_WORD_WRAP, enabled);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.display.word_wrap = enabled;
                    super::record_memory_write(state, KEY_WORD_WRAP);
                });
            }
        }
    }

    #[must_use]
    pub fn show_line_numbers(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_SHOW_LINE_NUMBERS),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.display.show_line_numbers)
            }
        }
    }

    pub fn set_show_line_numbers(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_SHOW_LINE_NUMBERS, enabled);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.display.show_line_numbers = enabled;
                    super::record_memory_write(state, KEY_SHOW_LINE_NUMBERS);
                });
            }
        }
    }

    #[must_use]
    pub fn show_minimap(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_SHOW_MINIMAP),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.display.show_minimap)
            }
        }
    }

    pub fn set_show_minimap(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_SHOW_MINIMAP, enabled);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.display.show_minimap = enabled;
                    super::record_memory_write(state, KEY_SHOW_MINIMAP);
                });
            }
        }
    }

    pub fn apply_word_wrap<T: IsA<gtk4::TextView>>(&self, text_view: &T) {
        let wrap_mode = if self.word_wrap() {
            gtk4::WrapMode::WordChar
        } else {
            gtk4::WrapMode::None
        };
        text_view.set_wrap_mode(wrap_mode);
    }
}
