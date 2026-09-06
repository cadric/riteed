use gtk4::prelude::SettingsExt;

use crate::document_limits::{
    DEFAULT_EDITOR_LIMIT_MIB, DEFAULT_FULL_FEATURE_LIMIT_MIB, DEFAULT_STRONG_WARNING_LIMIT_MIB,
    DEFAULT_VIEWER_ONLY_LIMIT_MIB, FileThresholds,
};

use super::{AppSettings, SettingsBackend};
#[cfg(test)]
use super::{record_memory_write, with_memory, with_memory_mut};

const KEY_FULL_FEATURE_LIMIT_MIB: &str = "large-file-full-feature-limit-mib";
const KEY_EDITOR_LIMIT_MIB: &str = "large-file-editor-limit-mib";
const KEY_STRONG_WARNING_LIMIT_MIB: &str = "large-file-strong-warning-limit-mib";
const KEY_VIEWER_ONLY_LIMIT_MIB: &str = "large-file-viewer-only-limit-mib";
const KEY_ALWAYS_ALLOW_LARGE_FILE_EDIT: &str = "always-allow-large-file-edit";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LargeFileLimitValues {
    pub(crate) full_feature: i32,
    pub(crate) editor: i32,
    pub(crate) strong_warning: i32,
    pub(crate) viewer_only: i32,
}

impl Default for LargeFileLimitValues {
    fn default() -> Self {
        Self {
            full_feature: DEFAULT_FULL_FEATURE_LIMIT_MIB,
            editor: DEFAULT_EDITOR_LIMIT_MIB,
            strong_warning: DEFAULT_STRONG_WARNING_LIMIT_MIB,
            viewer_only: DEFAULT_VIEWER_ONLY_LIMIT_MIB,
        }
    }
}

impl AppSettings {
    #[must_use]
    pub(crate) fn large_file_limit_values(&self) -> LargeFileLimitValues {
        let raw = match &self.backend {
            SettingsBackend::GSettings(settings) => LargeFileLimitValues {
                full_feature: settings.int(KEY_FULL_FEATURE_LIMIT_MIB),
                editor: settings.int(KEY_EDITOR_LIMIT_MIB),
                strong_warning: settings.int(KEY_STRONG_WARNING_LIMIT_MIB),
                viewer_only: settings.int(KEY_VIEWER_ONLY_LIMIT_MIB),
            },
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| LargeFileLimitValues {
                full_feature: state.large_file.full_feature,
                editor: state.large_file.editor,
                strong_warning: state.large_file.strong_warning,
                viewer_only: state.large_file.viewer_only,
            }),
        };
        sanitize_limit_values(raw)
    }

    #[must_use]
    pub(crate) fn large_file_thresholds(&self) -> FileThresholds {
        let limits = self.large_file_limit_values();
        FileThresholds::from_mib(
            limits.full_feature,
            limits.editor,
            limits.strong_warning,
            limits.viewer_only,
        )
    }

    pub(crate) fn set_large_file_limit_values(&self, limits: LargeFileLimitValues) {
        let limits = sanitize_limit_values(limits);
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_int(KEY_FULL_FEATURE_LIMIT_MIB, limits.full_feature);
                let _changed = settings.set_int(KEY_EDITOR_LIMIT_MIB, limits.editor);
                let _changed =
                    settings.set_int(KEY_STRONG_WARNING_LIMIT_MIB, limits.strong_warning);
                let _changed = settings.set_int(KEY_VIEWER_ONLY_LIMIT_MIB, limits.viewer_only);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.large_file.full_feature = limits.full_feature;
                    state.large_file.editor = limits.editor;
                    state.large_file.strong_warning = limits.strong_warning;
                    state.large_file.viewer_only = limits.viewer_only;
                    record_memory_write(state, KEY_FULL_FEATURE_LIMIT_MIB);
                    record_memory_write(state, KEY_EDITOR_LIMIT_MIB);
                    record_memory_write(state, KEY_STRONG_WARNING_LIMIT_MIB);
                    record_memory_write(state, KEY_VIEWER_ONLY_LIMIT_MIB);
                });
            }
        }
    }

    #[must_use]
    pub(crate) fn always_allow_large_file_edit(&self) -> bool {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                settings.boolean(KEY_ALWAYS_ALLOW_LARGE_FILE_EDIT)
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => with_memory(memory, |state| {
                state.large_file.always_allow_large_file_edit
            }),
        }
    }

    pub(crate) fn set_always_allow_large_file_edit(&self, enabled: bool) {
        match &self.backend {
            SettingsBackend::GSettings(settings) => {
                let _changed = settings.set_boolean(KEY_ALWAYS_ALLOW_LARGE_FILE_EDIT, enabled);
            }
            #[cfg(test)]
            SettingsBackend::Memory(memory) => {
                with_memory_mut(memory, |state| {
                    state.large_file.always_allow_large_file_edit = enabled;
                    record_memory_write(state, KEY_ALWAYS_ALLOW_LARGE_FILE_EDIT);
                });
            }
        }
    }
}

fn sanitize_limit_values(limits: LargeFileLimitValues) -> LargeFileLimitValues {
    let thresholds = FileThresholds::from_mib(
        limits.full_feature,
        limits.editor,
        limits.strong_warning,
        limits.viewer_only,
    );
    LargeFileLimitValues {
        full_feature: bytes_to_mib(thresholds.full_feature),
        editor: bytes_to_mib(thresholds.editor),
        strong_warning: bytes_to_mib(thresholds.strong_warning),
        viewer_only: bytes_to_mib(thresholds.viewer_only),
    }
}

fn bytes_to_mib(bytes: u64) -> i32 {
    let mib = bytes / crate::document_limits::MIB;
    i32::try_from(mib).map_or(DEFAULT_VIEWER_ONLY_LIMIT_MIB, |value| value)
}
