use gtk4::prelude::*;
use sourceview5::prelude::*;

use super::{AppSettings, SettingsBackend, sanitize_editor_width};

const KEY_INSERT_SPACES_INSTEAD_OF_TABS: &str = "insert-spaces-instead-of-tabs";
const KEY_TAB_WIDTH: &str = "tab-width";
const KEY_INDENT_WIDTH: &str = "indent-width";

impl AppSettings {
    #[must_use]
    pub fn insert_spaces_instead_of_tabs(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.boolean(KEY_INSERT_SPACES_INSTEAD_OF_TABS)
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => super::with_memory(memory, |state| {
                state.indentation.insert_spaces_instead_of_tabs
            }),
        }
    }

    pub fn set_insert_spaces_instead_of_tabs(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_INSERT_SPACES_INSTEAD_OF_TABS, enabled);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.indentation.insert_spaces_instead_of_tabs = enabled;
                    super::record_memory_write(state, KEY_INSERT_SPACES_INSTEAD_OF_TABS);
                });
            }
        }
    }

    #[must_use]
    pub fn tab_width(&self) -> u32 {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                sanitize_editor_width(settings.int(KEY_TAB_WIDTH), 4).cast_unsigned()
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => super::with_memory(memory, |state| {
                sanitize_editor_width(state.indentation.tab_width, 4).cast_unsigned()
            }),
        }
    }

    pub fn set_tab_width(&self, width: i32) {
        let width = sanitize_editor_width(width, 4);
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_int(KEY_TAB_WIDTH, width);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.indentation.tab_width = width;
                    super::record_memory_write(state, KEY_TAB_WIDTH);
                });
            }
        }
    }

    #[must_use]
    pub fn indent_width(&self) -> i32 {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                sanitize_editor_width(settings.int(KEY_INDENT_WIDTH), 4)
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => super::with_memory(memory, |state| {
                sanitize_editor_width(state.indentation.indent_width, 4)
            }),
        }
    }

    pub fn set_indent_width(&self, width: i32) {
        let width = sanitize_editor_width(width, 4);
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_int(KEY_INDENT_WIDTH, width);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.indentation.indent_width = width;
                    super::record_memory_write(state, KEY_INDENT_WIDTH);
                });
            }
        }
    }

    pub fn apply_indentation<T: IsA<sourceview5::View>>(&self, view: &T) {
        view.set_auto_indent(true);
        view.set_smart_backspace(true);
        view.set_indent_on_tab(true);
        view.set_insert_spaces_instead_of_tabs(self.insert_spaces_instead_of_tabs());
        view.set_tab_width(self.tab_width());
        view.set_indent_width(self.indent_width());
    }
}
