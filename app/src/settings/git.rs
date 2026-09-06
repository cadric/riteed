use crate::settings::{AppSettings, SettingsBackend};
#[cfg(test)]
use crate::settings::{record_memory_write, with_memory, with_memory_mut};
use gtk4::prelude::SettingsExt;

const KEY_GIT_USER_NAME: &str = "git-user-name";
const KEY_GIT_USER_EMAIL: &str = "git-user-email";

impl AppSettings {
    #[must_use]
    pub(crate) fn git_identity(&self) -> (String, String) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => (
                settings.string(KEY_GIT_USER_NAME).to_string(),
                settings.string(KEY_GIT_USER_EMAIL).to_string(),
            ),
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
                (state.git_user_name.clone(), state.git_user_email.clone())
            }),
        }
    }

    pub(crate) fn set_git_identity(&self, name: &str, email: &str) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed_name = settings.set_string(KEY_GIT_USER_NAME, name);
                let _changed_email = settings.set_string(KEY_GIT_USER_EMAIL, email);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.git_user_name = String::from(name);
                    state.git_user_email = String::from(email);
                    record_memory_write(state, KEY_GIT_USER_NAME);
                    record_memory_write(state, KEY_GIT_USER_EMAIL);
                });
            }
        }
    }
}
