use std::rc::Rc;
use std::sync::Mutex;

use gtk4::{gio, glib, prelude::*};

use crate::APP_ID;

pub use appearance::ThemePreference;
pub use compare::{CompareReviewSettingsSnapshot, CompareViewMode};
pub use language::AppLanguage;
pub(crate) use language::startup_language_preference;
pub(crate) use large_file::LargeFileLimitValues;
pub use presentation::{EditorPalette, WindowPalette};
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
    language: AppLanguage,
    display: MemoryDisplaySettings,
    indentation: MemoryIndentationSettings,
    presentation: MemoryPresentationSettings,
    editor_font: String,
    window_session: MemoryWindowSessionSettings,
    selected_document: MemorySelectedDocumentSettings,
    git_user_name: String,
    git_user_email: String,
    source_control_view_mode: SourceControlViewMode,
    compare: MemoryCompareSettings,
    project: MemoryProjectSettings,
    large_file: MemoryLargeFileSettings,
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
    window_palette: WindowPalette,
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
struct MemoryCompareSettings {
    view_mode: CompareViewMode,
    collapse_unchanged: bool,
    context_lines: i32,
    ignore_leading_trailing_whitespace: bool,
    word_wrap: bool,
}

#[derive(Clone)]
struct MemoryProjectSettings {
    folder_uri: String,
    folder_display_name: String,
    sidebar_visible: bool,
    show_hidden: bool,
}

#[derive(Clone)]
struct MemoryLargeFileSettings {
    full_feature: i32,
    editor: i32,
    strong_warning: i32,
    viewer_only: i32,
    always_allow_large_file_edit: bool,
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
                language: AppLanguage::System,
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
                    window_palette: WindowPalette::FollowEditor,
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
                compare: MemoryCompareSettings {
                    view_mode: CompareViewMode::Adaptive,
                    collapse_unchanged: true,
                    context_lines: 3,
                    ignore_leading_trailing_whitespace: false,
                    word_wrap: false,
                },
                project: MemoryProjectSettings {
                    folder_uri: String::new(),
                    folder_display_name: String::new(),
                    sidebar_visible: false,
                    show_hidden: false,
                },
                large_file: MemoryLargeFileSettings {
                    full_feature: crate::document_limits::DEFAULT_FULL_FEATURE_LIMIT_MIB,
                    editor: crate::document_limits::DEFAULT_EDITOR_LIMIT_MIB,
                    strong_warning: crate::document_limits::DEFAULT_STRONG_WARNING_LIMIT_MIB,
                    viewer_only: crate::document_limits::DEFAULT_VIEWER_ONLY_LIMIT_MIB,
                    always_allow_large_file_edit: false,
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

    #[must_use]
    pub(crate) fn connect_changed(
        &self,
        key: &'static str,
        callback: impl Fn() + 'static,
    ) -> SettingsSubscription {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let handler = settings.connect_changed(Some(key), move |_, _| callback());
                SettingsSubscription::new(settings, handler)
            }
            SettingsBackend::Memory(_) => SettingsSubscription::noop(),
        }
    }
}

pub(crate) struct SettingsSubscription {
    settings: Option<gio::Settings>,
    handler: Option<glib::SignalHandlerId>,
}

impl SettingsSubscription {
    #[must_use]
    fn new(settings: &gio::Settings, handler: glib::SignalHandlerId) -> Self {
        Self {
            settings: Some(settings.clone()),
            handler: Some(handler),
        }
    }

    #[must_use]
    fn noop() -> Self {
        Self {
            settings: None,
            handler: None,
        }
    }
}

impl Drop for SettingsSubscription {
    fn drop(&mut self) {
        if let (Some(settings), Some(handler)) = (self.settings.as_ref(), self.handler.take()) {
            settings.disconnect(handler);
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
mod compare;
mod display;
mod editor;
mod git;
mod indentation;
mod language;
mod large_file;
mod presentation;
mod project;
mod selected_document;
mod source_control;
mod window_session;
