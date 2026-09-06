use gtk4::prelude::{SettingsExt, SettingsExtManual};

use super::{AppSettings, SettingsBackend, sanitize_restored_dimension};
#[cfg(test)]
use super::{record_memory_write, with_memory, with_memory_mut};

const KEY_WINDOW_WIDTH: &str = "window-width";
const KEY_WINDOW_HEIGHT: &str = "window-height";
const KEY_RECENT_FILES: &str = "recent-files";
const KEY_SESSION_FILES: &str = "session-files";
const DEFAULT_WINDOW_WIDTH: i32 = 840;
const DEFAULT_WINDOW_HEIGHT: i32 = 620;
const MIN_WINDOW_WIDTH: i32 = 360;
const MIN_WINDOW_HEIGHT: i32 = 320;
const MAX_WINDOW_WIDTH: i32 = 8192;
const MAX_WINDOW_HEIGHT: i32 = 8192;

impl AppSettings {
    #[must_use]
    pub fn window_size(&self) -> (i32, i32) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => (
                sanitize_restored_dimension(
                    settings.int(KEY_WINDOW_WIDTH),
                    DEFAULT_WINDOW_WIDTH,
                    MIN_WINDOW_WIDTH,
                    MAX_WINDOW_WIDTH,
                ),
                sanitize_restored_dimension(
                    settings.int(KEY_WINDOW_HEIGHT),
                    DEFAULT_WINDOW_HEIGHT,
                    MIN_WINDOW_HEIGHT,
                    MAX_WINDOW_HEIGHT,
                ),
            ),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
                (
                    sanitize_restored_dimension(
                        state.window_session.window_width,
                        DEFAULT_WINDOW_WIDTH,
                        MIN_WINDOW_WIDTH,
                        MAX_WINDOW_WIDTH,
                    ),
                    sanitize_restored_dimension(
                        state.window_session.window_height,
                        DEFAULT_WINDOW_HEIGHT,
                        MIN_WINDOW_HEIGHT,
                        MAX_WINDOW_HEIGHT,
                    ),
                )
            }),
        }
    }

    pub fn set_window_size(&self, width: i32, height: i32) {
        let width = sanitize_restored_dimension(
            width,
            DEFAULT_WINDOW_WIDTH,
            MIN_WINDOW_WIDTH,
            MAX_WINDOW_WIDTH,
        );
        let height = sanitize_restored_dimension(
            height,
            DEFAULT_WINDOW_HEIGHT,
            MIN_WINDOW_HEIGHT,
            MAX_WINDOW_HEIGHT,
        );
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed_width = settings.set_int(KEY_WINDOW_WIDTH, width);
                let _changed_height = settings.set_int(KEY_WINDOW_HEIGHT, height);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.window_session.window_width = width;
                    state.window_session.window_height = height;
                    record_memory_write(state, KEY_WINDOW_WIDTH);
                    record_memory_write(state, KEY_WINDOW_HEIGHT);
                });
            }
        }
    }

    #[must_use]
    pub fn recent_files(&self) -> Vec<String> {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings
                .strv(KEY_RECENT_FILES)
                .iter()
                .map(ToString::to_string)
                .collect(),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.window_session.recent_files.clone())
            }
        }
    }

    pub fn set_recent_files(&self, files: &[String]) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_strv(KEY_RECENT_FILES, files);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.window_session.recent_files = files.to_vec();
                    record_memory_write(state, KEY_RECENT_FILES);
                });
            }
        }
    }

    #[must_use]
    pub fn session_files(&self) -> Vec<String> {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings
                .strv(KEY_SESSION_FILES)
                .iter()
                .map(ToString::to_string)
                .collect(),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.window_session.session_files.clone())
            }
        }
    }

    pub fn set_session_files(&self, files: &[String]) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_strv(KEY_SESSION_FILES, files);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.window_session.session_files = files.to_vec();
                    record_memory_write(state, KEY_SESSION_FILES);
                });
            }
        }
    }
}
