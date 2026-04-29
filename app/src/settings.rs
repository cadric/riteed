use std::rc::Rc;
use std::sync::Mutex;

use gtk4::gio;

use crate::APP_ID;

pub use appearance::ThemePreference;
pub use presentation::EditorPalette;
pub use source_control::SourceControlViewMode;

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
    presentation: MemoryPresentationSettings,
    editor_font: String,
    window_session: MemoryWindowSessionSettings,
    selected_document: MemorySelectedDocumentSettings,
    git_user_name: String,
    git_user_email: String,
    source_control_view_mode: SourceControlViewMode,
    project: MemoryProjectSettings,
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

#[derive(Clone)]
struct MemoryPresentationSettings {
    editor_palette: EditorPalette,
    highlight_current_line: bool,
    autosave_enabled: bool,
}

#[derive(Clone)]
struct MemoryWindowSessionSettings {
    window_width: i32,
    window_height: i32,
    recent_files: Vec<String>,
    session_files: Vec<String>,
}

#[derive(Clone)]
struct MemorySelectedDocumentSettings {
    session_selected_file: String,
}

#[derive(Clone)]
struct MemoryProjectSettings {
    folder_uri: String,
    folder_display_name: String,
    sidebar_visible: bool,
    show_hidden: bool,
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
                presentation: MemoryPresentationSettings {
                    editor_palette: EditorPalette::FollowSystem,
                    highlight_current_line: true,
                    autosave_enabled: false,
                },
                editor_font: String::new(),
                window_session: MemoryWindowSessionSettings {
                    window_width: 840,
                    window_height: 620,
                    recent_files: Vec::new(),
                    session_files: Vec::new(),
                },
                selected_document: MemorySelectedDocumentSettings {
                    session_selected_file: String::new(),
                },
                git_user_name: String::new(),
                git_user_email: String::new(),
                source_control_view_mode: SourceControlViewMode::Tree,
                project: MemoryProjectSettings {
                    folder_uri: String::new(),
                    folder_display_name: String::new(),
                    sidebar_visible: false,
                    show_hidden: false,
                },
                #[cfg(test)]
                write_log: Vec::new(),
            }))),
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

const fn sanitize_restored_dimension(value: i32, fallback: i32, min: i32, max: i32) -> i32 {
    if value <= 0 {
        fallback
    } else if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
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

mod appearance;
mod display;
mod editor;
mod git;
mod indentation;
mod presentation;
mod project;
mod selected_document;
mod source_control;
mod window_session;
