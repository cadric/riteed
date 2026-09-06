use gtk4::prelude::SettingsExt;
use libadwaita as adw;

use super::{AppSettings, SettingsBackend};
#[cfg(test)]
use super::{record_memory_write, with_memory, with_memory_mut};

const KEY_THEME: &str = "theme";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemePreference {
    System,
    Light,
    Dark,
}

impl ThemePreference {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    #[must_use]
    pub const fn enum_value(self) -> i32 {
        match self {
            Self::System => 0,
            Self::Light => 1,
            Self::Dark => 2,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::Light,
            2 => Self::Dark,
            _ => Self::System,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    #[must_use]
    pub fn from_nick(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }
}

impl AppSettings {
    #[must_use]
    pub fn theme(&self) -> ThemePreference {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                ThemePreference::from_enum_value(settings.enum_(KEY_THEME))
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| state.theme),
        }
    }

    pub fn set_theme(&self, preference: ThemePreference) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_enum(KEY_THEME, preference.enum_value());
            }
            #[cfg(test)]
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
            ThemePreference::Light => adw::ColorScheme::ForceLight,
            ThemePreference::Dark => adw::ColorScheme::ForceDark,
        };
        adw::StyleManager::default().set_color_scheme(color_scheme);
    }
}
