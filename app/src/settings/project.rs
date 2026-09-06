use super::{AppSettings, SettingsBackend};
use gtk4::prelude::SettingsExt;

const KEY_PROJECT_FOLDER_URI: &str = "project-folder-uri";
const KEY_PROJECT_FOLDER_DISPLAY_NAME: &str = "project-folder-display-name";
const KEY_PROJECT_SIDEBAR_VISIBLE: &str = "project-sidebar-visible";
const KEY_PROJECT_SHOW_HIDDEN: &str = "project-show-hidden";

impl AppSettings {
    #[must_use]
    pub fn project_folder_uri(&self) -> String {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.string(KEY_PROJECT_FOLDER_URI).to_string()
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.project.folder_uri.clone())
            }
        }
    }

    pub fn set_project_folder_uri(&self, uri: &str) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_PROJECT_FOLDER_URI, uri);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.project.folder_uri = String::from(uri);
                    super::record_memory_write(state, KEY_PROJECT_FOLDER_URI);
                });
            }
        }
    }

    #[must_use]
    pub fn project_folder_display_name(&self) -> String {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.string(KEY_PROJECT_FOLDER_DISPLAY_NAME).to_string()
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.project.folder_display_name.clone())
            }
        }
    }

    pub fn set_project_folder_display_name(&self, name: &str) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_PROJECT_FOLDER_DISPLAY_NAME, name);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.project.folder_display_name = String::from(name);
                    super::record_memory_write(state, KEY_PROJECT_FOLDER_DISPLAY_NAME);
                });
            }
        }
    }

    #[must_use]
    pub fn project_sidebar_visible(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_PROJECT_SIDEBAR_VISIBLE),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.project.sidebar_visible)
            }
        }
    }

    pub fn set_project_sidebar_visible(&self, visible: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_PROJECT_SIDEBAR_VISIBLE, visible);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.project.sidebar_visible = visible;
                    super::record_memory_write(state, KEY_PROJECT_SIDEBAR_VISIBLE);
                });
            }
        }
    }

    #[must_use]
    pub fn project_show_hidden(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_PROJECT_SHOW_HIDDEN),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.project.show_hidden)
            }
        }
    }

    pub fn set_project_show_hidden(&self, show_hidden: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_PROJECT_SHOW_HIDDEN, show_hidden);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.project.show_hidden = show_hidden;
                    super::record_memory_write(state, KEY_PROJECT_SHOW_HIDDEN);
                });
            }
        }
    }
}
