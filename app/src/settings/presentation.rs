use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita as adw;

use super::{AppSettings, SOURCE_STYLE_SCHEME_DARK, SOURCE_STYLE_SCHEME_LIGHT, SettingsBackend};

const KEY_EDITOR_PALETTE: &str = "editor-palette";
const KEY_HIGHLIGHT_CURRENT_LINE: &str = "highlight-current-line";
const KEY_AUTOSAVE_ENABLED: &str = "autosave-enabled";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorPalette {
    FollowSystem,
    AdwaitaLight,
    AdwaitaDark,
    Classic,
    Kate,
    KateDark,
    SolarizedLight,
    SolarizedDark,
}

impl EditorPalette {
    pub const ALL: [Self; 8] = [
        Self::FollowSystem,
        Self::AdwaitaLight,
        Self::AdwaitaDark,
        Self::Classic,
        Self::Kate,
        Self::KateDark,
        Self::SolarizedLight,
        Self::SolarizedDark,
    ];

    #[must_use]
    pub const fn enum_value(self) -> i32 {
        match self {
            Self::FollowSystem => 0,
            Self::AdwaitaLight => 1,
            Self::AdwaitaDark => 2,
            Self::Classic => 3,
            Self::Kate => 4,
            Self::KateDark => 5,
            Self::SolarizedLight => 6,
            Self::SolarizedDark => 7,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::AdwaitaLight,
            2 => Self::AdwaitaDark,
            3 => Self::Classic,
            4 => Self::Kate,
            5 => Self::KateDark,
            6 => Self::SolarizedLight,
            7 => Self::SolarizedDark,
            _ => Self::FollowSystem,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::FollowSystem => "follow-system",
            Self::AdwaitaLight => "adwaita-light",
            Self::AdwaitaDark => "adwaita-dark",
            Self::Classic => "classic",
            Self::Kate => "kate",
            Self::KateDark => "kate-dark",
            Self::SolarizedLight => "solarized-light",
            Self::SolarizedDark => "solarized-dark",
        }
    }

    #[must_use]
    pub fn from_index(index: u32, available: &[Self]) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| available.get(index).copied())
            .unwrap_or(Self::FollowSystem)
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::FollowSystem => pgettext("editor palette", "Follow System"),
            Self::AdwaitaLight => pgettext("editor palette", "Adwaita Light"),
            Self::AdwaitaDark => pgettext("editor palette", "Adwaita Dark"),
            Self::Classic => pgettext("editor palette", "Classic"),
            Self::Kate => pgettext("editor palette", "Kate"),
            Self::KateDark => pgettext("editor palette", "Kate Dark"),
            Self::SolarizedLight => pgettext("editor palette", "Solarized Light"),
            Self::SolarizedDark => pgettext("editor palette", "Solarized Dark"),
        }
    }

    #[must_use]
    pub(crate) const fn scheme_id(self) -> Option<&'static str> {
        match self {
            Self::FollowSystem => None,
            Self::AdwaitaLight => Some(SOURCE_STYLE_SCHEME_LIGHT),
            Self::AdwaitaDark => Some(SOURCE_STYLE_SCHEME_DARK),
            Self::Classic => Some("classic"),
            Self::Kate => Some("kate"),
            Self::KateDark => Some("kate-dark"),
            Self::SolarizedLight => Some("solarized-light"),
            Self::SolarizedDark => Some("solarized-dark"),
        }
    }

    #[must_use]
    pub(crate) const fn is_dark(self) -> Option<bool> {
        match self {
            Self::FollowSystem => None,
            Self::AdwaitaDark | Self::KateDark | Self::SolarizedDark => Some(true),
            Self::AdwaitaLight | Self::Classic | Self::Kate | Self::SolarizedLight => Some(false),
        }
    }
}

impl AppSettings {
    #[must_use]
    pub fn editor_palette(&self) -> EditorPalette {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                EditorPalette::from_enum_value(settings.enum_(KEY_EDITOR_PALETTE))
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.presentation.editor_palette)
            }
        }
    }

    pub fn set_editor_palette(&self, palette: EditorPalette) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_enum(KEY_EDITOR_PALETTE, palette.enum_value());
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.presentation.editor_palette = palette;
                    super::record_memory_write(state, KEY_EDITOR_PALETTE);
                });
            }
        }
    }

    #[must_use]
    pub fn highlight_current_line(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_HIGHLIGHT_CURRENT_LINE),
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.presentation.highlight_current_line)
            }
        }
    }

    pub fn set_highlight_current_line(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_HIGHLIGHT_CURRENT_LINE, enabled);
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.presentation.highlight_current_line = enabled;
                    super::record_memory_write(state, KEY_HIGHLIGHT_CURRENT_LINE);
                });
            }
        }
    }

    #[must_use]
    pub fn autosave_enabled(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_AUTOSAVE_ENABLED),
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.presentation.autosave_enabled)
            }
        }
    }

    pub fn set_autosave_enabled(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_AUTOSAVE_ENABLED, enabled);
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.presentation.autosave_enabled = enabled;
                    super::record_memory_write(state, KEY_AUTOSAVE_ENABLED);
                });
            }
        }
    }

    #[must_use]
    pub(crate) fn available_editor_palettes() -> Vec<EditorPalette> {
        let manager = sourceview5::StyleSchemeManager::default();
        EditorPalette::ALL
            .into_iter()
            .filter(|palette| {
                palette
                    .scheme_id()
                    .is_none_or(|scheme_id| manager.scheme(scheme_id).is_some())
            })
            .collect()
    }

    #[must_use]
    pub(crate) fn editor_palette_index(&self, available: &[EditorPalette]) -> u32 {
        let selected = self.editor_palette();
        available
            .iter()
            .position(|palette| *palette == selected)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(0)
    }

    #[must_use]
    pub(crate) fn resolved_source_style_scheme_id(&self) -> String {
        let selected = self.editor_palette();
        if let Some(scheme_id) = selected.scheme_id()
            && sourceview5::StyleSchemeManager::default()
                .scheme(scheme_id)
                .is_some()
        {
            return String::from(scheme_id);
        }

        let dark = adw::StyleManager::default().is_dark();
        String::from(if dark {
            SOURCE_STYLE_SCHEME_DARK
        } else {
            SOURCE_STYLE_SCHEME_LIGHT
        })
    }

    #[must_use]
    pub(crate) fn editor_palette_is_dark(&self) -> bool {
        let selected = self.editor_palette();
        if let Some(scheme_id) = selected.scheme_id()
            && sourceview5::StyleSchemeManager::default()
                .scheme(scheme_id)
                .is_some()
        {
            return selected.is_dark().unwrap_or(false);
        }
        adw::StyleManager::default().is_dark()
    }
}
