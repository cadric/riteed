use gtk4::prelude::SettingsExt;

use super::{AppSettings, SettingsBackend};

const KEY_COMPARE_VIEW_MODE: &str = "compare-view-mode";
const KEY_COMPARE_COLLAPSE_UNCHANGED: &str = "compare-collapse-unchanged";
const KEY_COMPARE_CONTEXT_LINES: &str = "compare-context-lines";
const KEY_COMPARE_IGNORE_WHITESPACE: &str = "compare-ignore-leading-trailing-whitespace";
const KEY_COMPARE_WORD_WRAP: &str = "compare-word-wrap";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompareViewMode {
    Adaptive,
    Split,
    Unified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompareReviewSettingsSnapshot {
    pub view_mode: CompareViewMode,
    pub collapse_unchanged: bool,
    pub context_lines: i32,
    pub ignore_leading_trailing_whitespace: bool,
    pub word_wrap: bool,
}

impl CompareViewMode {
    pub const ALL: [Self; 3] = [Self::Adaptive, Self::Split, Self::Unified];

    #[must_use]
    pub const fn enum_value(self) -> i32 {
        match self {
            Self::Adaptive => 0,
            Self::Split => 1,
            Self::Unified => 2,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::Split,
            2 => Self::Unified,
            _ => Self::Adaptive,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::Adaptive => "adaptive",
            Self::Split => "split",
            Self::Unified => "unified",
        }
    }

    #[must_use]
    pub fn from_nick(nick: &str) -> Option<Self> {
        match nick {
            "adaptive" => Some(Self::Adaptive),
            "split" => Some(Self::Split),
            "unified" => Some(Self::Unified),
            _ => None,
        }
    }
}

impl AppSettings {
    #[must_use]
    pub fn compare_review_settings_snapshot(&self) -> CompareReviewSettingsSnapshot {
        CompareReviewSettingsSnapshot {
            view_mode: self.compare_view_mode(),
            collapse_unchanged: self.compare_collapse_unchanged(),
            context_lines: self.compare_context_lines(),
            ignore_leading_trailing_whitespace: self.compare_ignore_leading_trailing_whitespace(),
            word_wrap: self.compare_word_wrap(),
        }
    }

    #[must_use]
    pub fn compare_view_mode(&self) -> CompareViewMode {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                CompareViewMode::from_enum_value(settings.enum_(KEY_COMPARE_VIEW_MODE))
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.compare.view_mode)
            }
        }
    }

    pub fn set_compare_view_mode(&self, mode: CompareViewMode) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_enum(KEY_COMPARE_VIEW_MODE, mode.enum_value());
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.compare.view_mode = mode;
                    super::record_memory_write(state, KEY_COMPARE_VIEW_MODE);
                });
            }
        }
    }

    #[must_use]
    pub fn compare_collapse_unchanged(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.boolean(KEY_COMPARE_COLLAPSE_UNCHANGED)
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.compare.collapse_unchanged)
            }
        }
    }

    pub fn set_compare_collapse_unchanged(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_COMPARE_COLLAPSE_UNCHANGED, enabled);
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.compare.collapse_unchanged = enabled;
                    super::record_memory_write(state, KEY_COMPARE_COLLAPSE_UNCHANGED);
                });
            }
        }
    }

    #[must_use]
    pub fn compare_context_lines(&self) -> i32 {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.int(KEY_COMPARE_CONTEXT_LINES),
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.compare.context_lines)
            }
        }
    }

    pub fn set_compare_context_lines(&self, lines: i32) {
        let lines = lines.clamp(1, 10);
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_int(KEY_COMPARE_CONTEXT_LINES, lines);
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.compare.context_lines = lines;
                    super::record_memory_write(state, KEY_COMPARE_CONTEXT_LINES);
                });
            }
        }
    }

    #[must_use]
    pub fn compare_ignore_leading_trailing_whitespace(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_COMPARE_IGNORE_WHITESPACE),
            SettingsBackend::Memory(memory) => super::with_memory(memory, |state| {
                state.compare.ignore_leading_trailing_whitespace
            }),
        }
    }

    pub fn set_compare_ignore_leading_trailing_whitespace(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_COMPARE_IGNORE_WHITESPACE, enabled);
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.compare.ignore_leading_trailing_whitespace = enabled;
                    super::record_memory_write(state, KEY_COMPARE_IGNORE_WHITESPACE);
                });
            }
        }
    }

    #[must_use]
    pub fn compare_word_wrap(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => settings.boolean(KEY_COMPARE_WORD_WRAP),
            SettingsBackend::Memory(memory) => {
                super::with_memory(memory, |state| state.compare.word_wrap)
            }
        }
    }

    pub fn set_compare_word_wrap(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_COMPARE_WORD_WRAP, enabled);
            }
            SettingsBackend::Memory(memory) => {
                super::with_memory_mut(memory, |state| {
                    state.compare.word_wrap = enabled;
                    super::record_memory_write(state, KEY_COMPARE_WORD_WRAP);
                });
            }
        }
    }

    #[must_use]
    pub(crate) fn connect_compare_view_mode_changed<F: Fn(&AppSettings) + 'static>(
        &self,
        callback: F,
    ) -> super::SettingsSubscription {
        let settings = self.clone();
        self.connect_changed(KEY_COMPARE_VIEW_MODE, move || callback(&settings))
    }

    #[must_use]
    pub(crate) fn connect_compare_collapse_changed<F: Fn(&AppSettings) + 'static>(
        &self,
        callback: F,
    ) -> super::SettingsSubscription {
        let settings = self.clone();
        self.connect_changed(KEY_COMPARE_COLLAPSE_UNCHANGED, move || callback(&settings))
    }

    #[must_use]
    pub(crate) fn connect_compare_context_lines_changed<F: Fn(&AppSettings) + 'static>(
        &self,
        callback: F,
    ) -> super::SettingsSubscription {
        let settings = self.clone();
        self.connect_changed(KEY_COMPARE_CONTEXT_LINES, move || callback(&settings))
    }

    #[must_use]
    pub(crate) fn connect_compare_ignore_whitespace_changed<F: Fn(&AppSettings) + 'static>(
        &self,
        callback: F,
    ) -> super::SettingsSubscription {
        let settings = self.clone();
        self.connect_changed(KEY_COMPARE_IGNORE_WHITESPACE, move || callback(&settings))
    }

    #[must_use]
    pub(crate) fn connect_compare_word_wrap_changed<F: Fn(&AppSettings) + 'static>(
        &self,
        callback: F,
    ) -> super::SettingsSubscription {
        let settings = self.clone();
        self.connect_changed(KEY_COMPARE_WORD_WRAP, move || callback(&settings))
    }
}
