use std::rc::Rc;
use std::sync::Mutex;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::APP_ID;

const KEY_THEME: &str = "theme";
const KEY_WORD_WRAP: &str = "word-wrap";
const KEY_WINDOW_WIDTH: &str = "window-width";
const KEY_WINDOW_HEIGHT: &str = "window-height";

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
    word_wrap: bool,
    window_width: i32,
    window_height: i32,
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
                word_wrap: false,
                window_width: 840,
                window_height: 620,
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
                with_memory_mut(memory, |state| state.theme = preference);
            }
        }
    }

    pub fn apply_theme(&self) {
        let style_manager = adw::StyleManager::default();
        let color_scheme = match self.theme() {
            ThemePreference::System => adw::ColorScheme::Default,
            ThemePreference::Light => adw::ColorScheme::PreferLight,
            ThemePreference::Dark => adw::ColorScheme::PreferDark,
        };
        style_manager.set_color_scheme(color_scheme);
    }

    #[must_use]
    pub fn word_wrap(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_WORD_WRAP),
            SettingsBackend::Memory(memory) => with_memory(memory, |state| state.word_wrap),
        }
    }

    pub fn set_word_wrap(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_WORD_WRAP, enabled);
            }
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| state.word_wrap = enabled);
            }
        }
    }

    pub fn apply_word_wrap(&self, text_view: &gtk4::TextView) {
        let wrap_mode = if self.word_wrap() {
            gtk4::WrapMode::WordChar
        } else {
            gtk4::WrapMode::None
        };
        text_view.set_wrap_mode(wrap_mode);
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
                });
            }
        }
    }
}

const fn sanitize_dimension(value: i32, fallback: i32) -> i32 {
    if value > 0 { value } else { fallback }
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
mod tests {
    use super::{AppSettings, ThemePreference, sanitize_dimension};

    #[test]
    fn theme_preference_roundtrips_indices() {
        assert_eq!(ThemePreference::from_index(0), ThemePreference::System);
        assert_eq!(ThemePreference::from_index(1), ThemePreference::Light);
        assert_eq!(ThemePreference::from_index(2), ThemePreference::Dark);
        assert_eq!(ThemePreference::Dark.index(), 2);
    }

    #[test]
    fn theme_preference_serializes_stored_values() {
        assert_eq!(ThemePreference::System.stored(), "system");
        assert_eq!(ThemePreference::Light.stored(), "light");
        assert_eq!(ThemePreference::Dark.stored(), "dark");
        assert_eq!(ThemePreference::Light.index(), 1);
    }

    #[test]
    fn theme_preference_parses_stored_values() {
        assert_eq!(
            ThemePreference::from_stored("system"),
            ThemePreference::System
        );
        assert_eq!(
            ThemePreference::from_stored("light"),
            ThemePreference::Light
        );
        assert_eq!(ThemePreference::from_stored("dark"), ThemePreference::Dark);
        assert_eq!(
            ThemePreference::from_stored("other"),
            ThemePreference::System
        );
    }

    #[test]
    fn invalid_dimensions_fall_back() {
        assert_eq!(sanitize_dimension(900, 840), 900);
        assert_eq!(sanitize_dimension(0, 840), 840);
        assert_eq!(sanitize_dimension(-2, 620), 620);
    }

    #[test]
    fn memory_backend_roundtrips_values() {
        let settings = AppSettings::new_for_tests();
        settings.set_theme(ThemePreference::Dark);
        settings.set_word_wrap(true);
        settings.set_window_size(900, 700);
        assert_eq!(settings.theme(), ThemePreference::Dark);
        assert!(settings.word_wrap());
        assert_eq!(settings.window_size(), (900, 700));
    }
}
