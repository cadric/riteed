use gettextrs::pgettext;
use gtk4::{gio, prelude::*};

use crate::APP_ID;

use super::{AppSettings, SettingsBackend};
#[cfg(test)]
use super::{record_memory_write, with_memory, with_memory_mut};

pub(crate) const KEY_LANGUAGE: &str = "language";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppLanguage {
    System,
    English,
    Danish,
}

impl AppLanguage {
    pub const ALL: [Self; 3] = [Self::System, Self::English, Self::Danish];

    #[must_use]
    pub const fn enum_value(self) -> i32 {
        match self {
            Self::System => 0,
            Self::English => 1,
            Self::Danish => 2,
        }
    }

    #[must_use]
    pub const fn from_enum_value(value: i32) -> Self {
        match value {
            1 => Self::English,
            2 => Self::Danish,
            _ => Self::System,
        }
    }

    #[must_use]
    pub const fn nick(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::English => "en",
            Self::Danish => "da",
        }
    }

    #[must_use]
    pub fn from_index(index: u32) -> Self {
        usize::try_from(index)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
            .unwrap_or(Self::System)
    }

    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::System => pgettext("language option", "System"),
            Self::English => pgettext("language option", "English"),
            Self::Danish => pgettext("language option", "Danish"),
        }
    }
}

impl AppSettings {
    #[must_use]
    pub fn language(&self) -> AppLanguage {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                AppLanguage::from_enum_value(settings.enum_(KEY_LANGUAGE))
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| state.language),
        }
    }

    pub fn set_language(&self, language: AppLanguage) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_enum(KEY_LANGUAGE, language.enum_value());
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.language = language;
                    record_memory_write(state, KEY_LANGUAGE);
                });
            }
        }
    }
}

#[must_use]
pub(crate) fn startup_language_preference() -> AppLanguage {
    let Some(source) = gio::SettingsSchemaSource::default() else {
        return AppLanguage::System;
    };
    let Some(schema) = source.lookup(APP_ID, true) else {
        return AppLanguage::System;
    };
    if !schema.has_key(KEY_LANGUAGE) {
        return AppLanguage::System;
    }
    let settings = gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None);
    AppLanguage::from_enum_value(settings.enum_(KEY_LANGUAGE))
}
