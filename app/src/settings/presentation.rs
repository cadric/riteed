use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use super::{AppSettings, SettingsBackend};

pub(crate) const KEY_EDITOR_PALETTE: &str = "editor-palette";
pub(crate) const KEY_WINDOW_PALETTE: &str = "window-palette";
const KEY_HIGHLIGHT_CURRENT_LINE: &str = "highlight-current-line";
const KEY_AUTOSAVE_ENABLED: &str = "autosave-enabled";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditorPalette {
    FollowSystem,
    ClassicLight,
    ClassicDark,
    AdwaitaLight,
    AdwaitaDark,
    Kate,
    KateDark,
    SolarizedLight,
    SolarizedDark,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowPalette {
    FollowEditor,
    Adwaita,
    Classic,
    Kate,
    Solarized,
}

impl WindowPalette {
    pub const ALL: [Self; 5] = [
        Self::FollowEditor,
        Self::Adwaita,
        Self::Classic,
        Self::Kate,
        Self::Solarized,
    ];

    #[must_use]
    pub const fn enum_value(self) -> i32 {
        match self {
            Self::FollowEditor => 0,
            Self::Adwaita => 1,
            Self::Classic => 2,
            Self::Kate => 3,
            Self::Solarized => 4,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::Adwaita,
            2 => Self::Classic,
            3 => Self::Kate,
            4 => Self::Solarized,
            _ => Self::FollowEditor,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::FollowEditor => "follow-editor",
            Self::Adwaita => "adwaita",
            Self::Classic => "classic",
            Self::Kate => "kate",
            Self::Solarized => "solarized",
        }
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::FollowEditor => String::from("Compatibility"),
            Self::Adwaita => pgettext("window palette", "Adwaita"),
            Self::Classic => pgettext("window palette", "Classic"),
            Self::Kate => pgettext("window palette", "Kate"),
            Self::Solarized => pgettext("window palette", "Solarized"),
        }
    }
}

impl EditorPalette {
    pub const ALL: [Self; 9] = [
        Self::FollowSystem,
        Self::ClassicLight,
        Self::ClassicDark,
        Self::AdwaitaLight,
        Self::AdwaitaDark,
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
            Self::ClassicLight => 3,
            Self::Kate => 4,
            Self::KateDark => 5,
            Self::SolarizedLight => 6,
            Self::SolarizedDark => 7,
            Self::ClassicDark => 8,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::AdwaitaLight,
            2 => Self::AdwaitaDark,
            3 => Self::ClassicLight,
            4 => Self::Kate,
            5 => Self::KateDark,
            6 => Self::SolarizedLight,
            7 => Self::SolarizedDark,
            8 => Self::ClassicDark,
            _ => Self::FollowSystem,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::FollowSystem => "follow-system",
            Self::AdwaitaLight => "adwaita-light",
            Self::AdwaitaDark => "adwaita-dark",
            Self::ClassicLight => "classic",
            Self::ClassicDark => "classic-dark",
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
            Self::FollowSystem => String::from("Compatibility"),
            Self::AdwaitaLight => pgettext("editor palette", "Adwaita Light"),
            Self::AdwaitaDark => pgettext("editor palette", "Adwaita Dark"),
            Self::ClassicLight => pgettext("editor palette", "Classic Light"),
            Self::ClassicDark => pgettext("editor palette", "Classic Dark"),
            Self::Kate => pgettext("editor palette", "Kate Light"),
            Self::KateDark => pgettext("editor palette", "Kate Dark"),
            Self::SolarizedLight => pgettext("editor palette", "Solarized Light"),
            Self::SolarizedDark => pgettext("editor palette", "Solarized Dark"),
        }
    }

    #[must_use]
    pub(crate) const fn scheme_id(self) -> Option<&'static str> {
        match self {
            Self::FollowSystem => None,
            Self::AdwaitaLight => Some(crate::palette_engine::ADWAITA_LIGHT_SCHEME),
            Self::AdwaitaDark => Some(crate::palette_engine::ADWAITA_DARK_SCHEME),
            Self::ClassicLight => Some("classic"),
            Self::ClassicDark => Some("classic-dark"),
            Self::Kate => Some("kate"),
            Self::KateDark => Some("kate-dark"),
            Self::SolarizedLight => Some("solarized-light"),
            Self::SolarizedDark => Some("solarized-dark"),
        }
    }
}

impl AppSettings {
    pub(crate) fn apply_source_style_scheme(&self, buffer: &sourceview5::Buffer) {
        let manager = sourceview5::StyleSchemeManager::default();
        let preferred = self.resolved_source_style_scheme_id();
        let scheme = manager
            .scheme(&preferred)
            .or_else(|| manager.scheme(crate::palette_engine::ADWAITA_LIGHT_SCHEME));
        buffer.set_style_scheme(scheme.as_ref());
    }

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
    pub fn window_palette(&self) -> WindowPalette {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                WindowPalette::from_enum_value(settings.enum_(KEY_WINDOW_PALETTE))
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.presentation.window_palette)
            }
        }
    }

    pub fn set_window_palette(&self, palette: WindowPalette) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_enum(KEY_WINDOW_PALETTE, palette.enum_value());
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.presentation.window_palette = palette;
                    super::record_memory_write(state, KEY_WINDOW_PALETTE);
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
    pub(crate) fn resolved_source_style_scheme_id(&self) -> String {
        crate::palette_engine::editor_scheme_id(
            self.editor_palette(),
            adw::StyleManager::default().is_dark(),
        )
    }

    #[must_use]
    pub(crate) fn editor_palette_is_dark(&self) -> bool {
        crate::palette_engine::editor_palette_is_dark(
            self.editor_palette(),
            adw::StyleManager::default().is_dark(),
        )
    }

    #[must_use]
    pub(crate) fn connect_editor_palette_changed(
        &self,
        callback: impl Fn() + 'static,
    ) -> super::SettingsSubscription {
        self.connect_changed(KEY_EDITOR_PALETTE, callback)
    }
}
