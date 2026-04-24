use std::rc::Rc;
use std::sync::Mutex;

use gtk4::{gio, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::APP_ID;

const KEY_THEME: &str = "theme";
const KEY_WORD_WRAP: &str = "word-wrap";
const KEY_SHOW_LINE_NUMBERS: &str = "show-line-numbers";
const KEY_SHOW_MINIMAP: &str = "show-minimap";
const KEY_INSERT_SPACES_INSTEAD_OF_TABS: &str = "insert-spaces-instead-of-tabs";
const KEY_TAB_WIDTH: &str = "tab-width";
const KEY_INDENT_WIDTH: &str = "indent-width";
const KEY_EDITOR_FONT: &str = "editor-font";
const KEY_WINDOW_WIDTH: &str = "window-width";
const KEY_WINDOW_HEIGHT: &str = "window-height";
const KEY_RECENT_FILES: &str = "recent-files";
const KEY_SESSION_FILES: &str = "session-files";
const KEY_SESSION_SELECTED_FILE: &str = "session-selected-file";
const SOURCE_STYLE_SCHEME_LIGHT: &str = "Adwaita";
const SOURCE_STYLE_SCHEME_DARK: &str = "Adwaita-dark";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    #[must_use]
    pub fn from_stored(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    #[must_use]
    pub const fn stored(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    #[must_use]
    pub fn from_index(index: u32) -> Self {
        match index {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }
}

#[derive(Clone)]
pub struct AppSettings {
    backend: SettingsBackend,
}

#[derive(Clone)]
enum SettingsBackend {
    GSettings(gio::Settings),
    Memory(Rc<Mutex<MemorySettings>>),
}

#[derive(Clone)]
struct MemorySettings {
    theme: ThemePreference,
    display: MemoryDisplaySettings,
    indentation: MemoryIndentationSettings,
    editor_font: String,
    window_width: i32,
    window_height: i32,
    recent_files: Vec<String>,
    session_files: Vec<String>,
    session_selected_file: String,
    project_folder_uri: String,
    project_folder_display_name: String,
    project_sidebar_visible: bool,
    project_show_hidden: bool,
    #[cfg(test)]
    write_log: Vec<String>,
}

#[derive(Clone)]
struct MemoryDisplaySettings {
    word_wrap: bool,
    show_line_numbers: bool,
    show_minimap: bool,
}

#[derive(Clone)]
struct MemoryIndentationSettings {
    insert_spaces_instead_of_tabs: bool,
    tab_width: i32,
    indent_width: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl AppSettings {
    #[must_use]
    pub fn new() -> Self {
        Self {
            backend: SettingsBackend::GSettings(gio::Settings::new(APP_ID)),
        }
    }

    #[must_use]
    pub fn new_for_tests() -> Self {
        Self {
            backend: SettingsBackend::Memory(Rc::new(Mutex::new(MemorySettings {
                theme: ThemePreference::System,
                display: MemoryDisplaySettings {
                    word_wrap: false,
                    show_line_numbers: false,
                    show_minimap: false,
                },
                indentation: MemoryIndentationSettings {
                    insert_spaces_instead_of_tabs: true,
                    tab_width: 4,
                    indent_width: 4,
                },
                editor_font: String::new(),
                window_width: 840,
                window_height: 620,
                recent_files: Vec::new(),
                session_files: Vec::new(),
                session_selected_file: String::new(),
                project_folder_uri: String::new(),
                project_folder_display_name: String::new(),
                project_sidebar_visible: false,
                project_show_hidden: false,
                #[cfg(test)]
                write_log: Vec::new(),
            }))),
        }
    }

    #[must_use]
    pub fn theme(&self) -> ThemePreference {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                ThemePreference::from_stored(settings.string(KEY_THEME).as_str())
            }
            SettingsBackend::Memory(memory) => with_memory(memory, |state| state.theme),
        }
    }

    pub fn set_theme(&self, preference: ThemePreference) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_THEME, preference.stored());
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.theme = preference;
                    record_memory_write(state, KEY_THEME);
                });
            }
        }
    }

    pub fn apply_theme(&self) {
        let color_scheme = match self.theme() {
            ThemePreference::System => adw::ColorScheme::Default,
            ThemePreference::Light => adw::ColorScheme::PreferLight,
            ThemePreference::Dark => adw::ColorScheme::PreferDark,
        };
        adw::StyleManager::default().set_color_scheme(color_scheme);
    }

    pub(crate) fn apply_source_style_scheme(&self, buffer: &sourceview5::Buffer) {
        let manager = sourceview5::StyleSchemeManager::default();
        let preferred = match self.theme() {
            ThemePreference::Dark => SOURCE_STYLE_SCHEME_DARK,
            ThemePreference::System if adw::StyleManager::default().is_dark() => {
                SOURCE_STYLE_SCHEME_DARK
            }
            ThemePreference::Light | ThemePreference::System => SOURCE_STYLE_SCHEME_LIGHT,
        };
        let scheme = manager
            .scheme(preferred)
            .or_else(|| manager.scheme(SOURCE_STYLE_SCHEME_LIGHT));
        buffer.set_style_scheme(scheme.as_ref());
    }

    #[must_use]
    pub fn word_wrap(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_WORD_WRAP),
            SettingsBackend::Memory(memory) => with_memory(memory, |state| state.display.word_wrap),
        }
    }

    pub fn set_word_wrap(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_WORD_WRAP, enabled);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.display.word_wrap = enabled;
                    record_memory_write(state, KEY_WORD_WRAP);
                });
            }
        }
    }

    #[must_use]
    pub fn show_line_numbers(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_SHOW_LINE_NUMBERS),
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.display.show_line_numbers)
            }
        }
    }

    pub fn set_show_line_numbers(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_SHOW_LINE_NUMBERS, enabled);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.display.show_line_numbers = enabled;
                    record_memory_write(state, KEY_SHOW_LINE_NUMBERS);
                });
            }
        }
    }

    #[must_use]
    pub fn show_minimap(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_SHOW_MINIMAP),
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.display.show_minimap)
            }
        }
    }

    pub fn set_show_minimap(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_SHOW_MINIMAP, enabled);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.display.show_minimap = enabled;
                    record_memory_write(state, KEY_SHOW_MINIMAP);
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

    #[must_use]
    pub fn insert_spaces_instead_of_tabs(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.boolean(KEY_INSERT_SPACES_INSTEAD_OF_TABS)
            }
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
                state.indentation.insert_spaces_instead_of_tabs
            }),
        }
    }

    pub fn set_insert_spaces_instead_of_tabs(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_INSERT_SPACES_INSTEAD_OF_TABS, enabled);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.indentation.insert_spaces_instead_of_tabs = enabled;
                    record_memory_write(state, KEY_INSERT_SPACES_INSTEAD_OF_TABS);
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
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
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
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.indentation.tab_width = width;
                    record_memory_write(state, KEY_TAB_WIDTH);
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
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
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
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.indentation.indent_width = width;
                    record_memory_write(state, KEY_INDENT_WIDTH);
                });
            }
        }
    }

    #[must_use]
    pub fn editor_font(&self) -> String {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.string(KEY_EDITOR_FONT).to_string(),
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
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.editor_font = String::from(font);
                    record_memory_write(state, KEY_EDITOR_FONT);
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

    #[must_use]
    pub fn window_size(&self) -> (i32, i32) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => (
                sanitize_dimension(settings.int(KEY_WINDOW_WIDTH), 840),
                sanitize_dimension(settings.int(KEY_WINDOW_HEIGHT), 620),
            ),
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
                (
                    sanitize_dimension(state.window_width, 840),
                    sanitize_dimension(state.window_height, 620),
                )
            }),
        }
    }

    pub fn set_window_size(&self, width: i32, height: i32) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed_width = settings.set_int(KEY_WINDOW_WIDTH, width);
                let _changed_height = settings.set_int(KEY_WINDOW_HEIGHT, height);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.window_width = width;
                    state.window_height = height;
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
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.recent_files.clone())
            }
        }
    }

    pub fn set_recent_files(&self, files: &[String]) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_strv(KEY_RECENT_FILES, files);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.recent_files = files.to_vec();
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
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.session_files.clone())
            }
        }
    }

    pub fn set_session_files(&self, files: &[String]) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_strv(KEY_SESSION_FILES, files);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.session_files = files.to_vec();
                    record_memory_write(state, KEY_SESSION_FILES);
                });
            }
        }
    }

    #[must_use]
    pub fn session_selected_file(&self) -> String {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.string(KEY_SESSION_SELECTED_FILE).to_string()
            }
            SettingsBackend::Memory(memory) => {
                with_memory(memory, |state| state.session_selected_file.clone())
            }
        }
    }

    pub fn set_session_selected_file(&self, uri: &str) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_string(KEY_SESSION_SELECTED_FILE, uri);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.session_selected_file = String::from(uri);
                    record_memory_write(state, KEY_SESSION_SELECTED_FILE);
                });
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn write_log_for_tests(&self) -> Vec<String> {
        match &self.backend {
            SettingsBackend::GSettings(_) => Vec::new(),
            SettingsBackend::Memory(memory) => with_memory(memory, |state| state.write_log.clone()),
        }
    }
}

const fn sanitize_dimension(value: i32, fallback: i32) -> i32 {
    if value > 0 { value } else { fallback }
}

const fn sanitize_editor_width(value: i32, fallback: i32) -> i32 {
    if value >= 1 && value <= 16 {
        value
    } else {
        fallback
    }
}

fn with_memory<T>(memory: &Rc<Mutex<MemorySettings>>, read: impl Fn(&MemorySettings) -> T) -> T {
    match memory.lock() {
        Ok(guard) => read(&guard),
        Err(poisoned) => read(&poisoned.into_inner()),
    }
}

fn with_memory_mut(memory: &Rc<Mutex<MemorySettings>>, write: impl Fn(&mut MemorySettings)) {
    match memory.lock() {
        Ok(mut guard) => write(&mut guard),
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            write(&mut guard);
        }
    }
}

#[cfg(test)]
fn record_memory_write(state: &mut MemorySettings, key: &str) {
    state.write_log.push(String::from(key));
}

#[cfg(not(test))]
fn record_memory_write(_state: &mut MemorySettings, _key: &str) {}

#[cfg(test)]
mod tests;

mod project;
