use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

pub(crate) const MIB: u64 = 1024 * 1024;
pub(crate) const SMALL_FILE_LIMIT_BYTES: u64 = 5 * MIB;
pub(crate) const MEDIUM_FILE_LIMIT_BYTES: u64 = 25 * MIB;
pub(crate) const LARGE_FILE_LIMIT_BYTES: u64 = 75 * MIB;
#[cfg(test)]
pub(crate) const EDITOR_POLICY_CEILING_BYTES: u64 = 500 * MIB;
pub(crate) const EDITOR_HARD_LIMIT_BYTES: u64 = MEDIUM_FILE_LIMIT_BYTES;
pub(crate) const OPEN_FILE_LIMIT_BYTES: u64 = EDITOR_HARD_LIMIT_BYTES;
pub(crate) const SAVE_SNAPSHOT_LIMIT_BYTES: u64 = MEDIUM_FILE_LIMIT_BYTES;
pub(crate) const SEARCH_CHAR_LIMIT: i32 = 5_000_000;
pub(crate) const VIEWER_PAGE_BYTES: usize = 256 * 1024;
pub(crate) const VIEWER_SEARCH_MATCH_LIMIT: usize = 10_000;
const SIZE_QUERY_ATTRIBUTES: &str = "standard::type,standard::size";
const SIZE_ATTRIBUTE: &str = "standard::size";
pub(crate) const DEFAULT_FULL_FEATURE_LIMIT_MIB: i32 = 5;
pub(crate) const DEFAULT_EDITOR_LIMIT_MIB: i32 = 25;
pub(crate) const DEFAULT_STRONG_WARNING_LIMIT_MIB: i32 = 75;
pub(crate) const DEFAULT_VIEWER_ONLY_LIMIT_MIB: i32 = 500;
pub(crate) const MAX_VIEWER_ONLY_LIMIT_MIB: i32 = 500;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FileTier {
    Small,
    Medium,
    Large,
    VeryLarge,
    ViewerOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenDecision {
    Editor { tier: FileTier },
    Viewer { tier: FileTier, edit_allowed: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OpenFilePlan {
    pub(crate) size: u64,
    pub(crate) decision: OpenDecision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OpenPlanQueryResult {
    KnownSize(u64),
    NonRegular,
    SizeUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OpenFileSupport {
    pub(crate) supports_open: bool,
    pub(crate) size: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileThresholds {
    pub(crate) full_feature: u64,
    pub(crate) editor: u64,
    pub(crate) strong_warning: u64,
    pub(crate) viewer_only: u64,
}

impl FileThresholds {
    #[must_use]
    pub(crate) fn from_mib(
        full_feature: i32,
        editor: i32,
        strong_warning: i32,
        viewer_only: i32,
    ) -> Self {
        let full_feature = full_feature.clamp(1, DEFAULT_EDITOR_LIMIT_MIB.saturating_sub(1));
        let editor = editor.clamp(full_feature.saturating_add(1), DEFAULT_EDITOR_LIMIT_MIB);
        let strong_warning =
            strong_warning.clamp(editor.saturating_add(1), MAX_VIEWER_ONLY_LIMIT_MIB);
        let viewer_only = viewer_only.clamp(strong_warning, MAX_VIEWER_ONLY_LIMIT_MIB);
        Self {
            full_feature: mib_to_bytes(full_feature),
            editor: mib_to_bytes(editor),
            strong_warning: mib_to_bytes(strong_warning),
            viewer_only: mib_to_bytes(viewer_only),
        }
    }
}

impl Default for FileThresholds {
    fn default() -> Self {
        Self::from_mib(
            DEFAULT_FULL_FEATURE_LIMIT_MIB,
            DEFAULT_EDITOR_LIMIT_MIB,
            DEFAULT_STRONG_WARNING_LIMIT_MIB,
            DEFAULT_VIEWER_ONLY_LIMIT_MIB,
        )
    }
}

#[cfg(test)]
#[must_use]
pub(crate) const fn tier_for_size(size: u64) -> FileTier {
    if size < SMALL_FILE_LIMIT_BYTES {
        FileTier::Small
    } else if size < MEDIUM_FILE_LIMIT_BYTES {
        FileTier::Medium
    } else if size < LARGE_FILE_LIMIT_BYTES {
        FileTier::Large
    } else if size <= EDITOR_POLICY_CEILING_BYTES {
        FileTier::VeryLarge
    } else {
        FileTier::ViewerOnly
    }
}

#[cfg(test)]
#[must_use]
pub(crate) const fn open_decision_for_size(size: u64) -> OpenDecision {
    let tier = tier_for_size(size);
    decision_for_tier_and_size(tier, size)
}

#[must_use]
pub(crate) const fn open_decision_for_size_with_thresholds(
    size: u64,
    thresholds: &FileThresholds,
) -> OpenDecision {
    let tier = tier_for_size_with_thresholds(size, thresholds);
    decision_for_tier_and_size(tier, size)
}

#[must_use]
const fn decision_for_tier_and_size(tier: FileTier, size: u64) -> OpenDecision {
    match tier {
        FileTier::Small | FileTier::Medium if size < EDITOR_HARD_LIMIT_BYTES => {
            OpenDecision::Editor { tier }
        }
        FileTier::Small | FileTier::Medium | FileTier::Large | FileTier::VeryLarge => {
            OpenDecision::Viewer {
                tier,
                edit_allowed: size <= EDITOR_HARD_LIMIT_BYTES,
            }
        }
        FileTier::ViewerOnly => OpenDecision::Viewer {
            tier,
            edit_allowed: false,
        },
    }
}

#[must_use]
pub(crate) const fn tier_for_size_with_thresholds(
    size: u64,
    thresholds: &FileThresholds,
) -> FileTier {
    if size < thresholds.full_feature {
        FileTier::Small
    } else if size < thresholds.editor {
        FileTier::Medium
    } else if size < thresholds.strong_warning {
        FileTier::Large
    } else if size <= thresholds.viewer_only {
        FileTier::VeryLarge
    } else {
        FileTier::ViewerOnly
    }
}

#[must_use]
pub(crate) const fn markdown_preview_enabled(size: u64) -> bool {
    size < SMALL_FILE_LIMIT_BYTES
}

#[must_use]
pub(crate) fn buffer_supports_search(buffer: &sourceview5::Buffer) -> bool {
    char_count_supports_search(buffer.char_count())
}

#[must_use]
pub(crate) fn autosave_supports_current_size(char_count: i32) -> bool {
    buffer_char_count_supports_save_snapshot(char_count)
}

#[must_use]
pub(crate) fn buffer_char_count_supports_save_snapshot(char_count: i32) -> bool {
    u64::try_from(char_count).is_ok_and(size_supports_save_snapshot)
}

#[must_use]
pub(crate) fn text_len_supports_save_snapshot(len: usize) -> bool {
    u64::try_from(len).is_ok_and(size_supports_save_snapshot)
}

#[must_use]
pub(crate) fn loaded_text_supports_editor_hard_cap(len: usize) -> bool {
    u64::try_from(len).is_ok_and(file_size_supports_open)
}

pub(crate) fn query_file_supports_open(
    file: &gio::File,
    cancellable: Option<&gio::Cancellable>,
    callback: Rc<dyn Fn(Result<OpenFileSupport, glib::Error>)>,
) {
    file.query_info_async(
        SIZE_QUERY_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        cancellable,
        move |result| match result {
            Ok(info) => callback(Ok(file_info_support(&info))),
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => callback(Err(error)),
            Err(_error) => callback(Ok(OpenFileSupport {
                supports_open: true,
                size: None,
            })),
        },
    );
}

pub(crate) fn query_file_open_plan(
    file: &gio::File,
    cancellable: Option<&gio::Cancellable>,
    callback: Rc<dyn Fn(Result<OpenPlanQueryResult, glib::Error>)>,
) {
    file.query_info_async(
        SIZE_QUERY_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        cancellable,
        move |result| match result {
            Ok(info) => callback(Ok(file_info_open_plan(&info))),
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => callback(Err(error)),
            Err(_error) => callback(Ok(OpenPlanQueryResult::SizeUnavailable)),
        },
    );
}

#[must_use]
pub(crate) fn file_supports_session_restore(file: &gio::File) -> bool {
    file.path().is_some()
}

#[must_use]
pub(crate) fn uri_supports_session_restore(uri: &str) -> bool {
    file_supports_session_restore(&gio::File::for_uri(uri))
}

#[must_use]
fn char_count_supports_search(char_count: i32) -> bool {
    char_count <= SEARCH_CHAR_LIMIT
}

fn file_info_support(info: &gio::FileInfo) -> OpenFileSupport {
    if info.file_type() != gio::FileType::Regular {
        return OpenFileSupport {
            supports_open: false,
            size: None,
        };
    }
    let size = u64::try_from(info.size()).ok();
    OpenFileSupport {
        supports_open: size.is_some_and(file_size_supports_open),
        size,
    }
}

fn file_info_open_plan(info: &gio::FileInfo) -> OpenPlanQueryResult {
    if info.file_type() != gio::FileType::Regular {
        return OpenPlanQueryResult::NonRegular;
    }
    let Some(size) = file_info_known_size(info) else {
        return OpenPlanQueryResult::SizeUnavailable;
    };
    OpenPlanQueryResult::KnownSize(size)
}

fn file_info_known_size(info: &gio::FileInfo) -> Option<u64> {
    if !info.has_attribute(SIZE_ATTRIBUTE) {
        return None;
    }
    u64::try_from(info.size()).ok()
}

fn file_size_supports_open(size: u64) -> bool {
    size <= OPEN_FILE_LIMIT_BYTES
}

fn size_supports_save_snapshot(size: u64) -> bool {
    size <= SAVE_SNAPSHOT_LIMIT_BYTES
}

fn mib_to_bytes(value: i32) -> u64 {
    let value = if value < 0 {
        0
    } else {
        u64::from(value.cast_unsigned())
    };
    value * MIB
}

#[cfg(test)]
mod tests {
    use super::{
        EDITOR_HARD_LIMIT_BYTES, EDITOR_POLICY_CEILING_BYTES, FileThresholds, FileTier,
        MEDIUM_FILE_LIMIT_BYTES, MIB, OPEN_FILE_LIMIT_BYTES, OpenDecision, OpenPlanQueryResult,
        SAVE_SNAPSHOT_LIMIT_BYTES, SEARCH_CHAR_LIMIT, SMALL_FILE_LIMIT_BYTES,
        autosave_supports_current_size, buffer_char_count_supports_save_snapshot,
        char_count_supports_search, file_info_open_plan, file_size_supports_open,
        loaded_text_supports_editor_hard_cap, markdown_preview_enabled, mib_to_bytes,
        open_decision_for_size, open_decision_for_size_with_thresholds,
        text_len_supports_save_snapshot, tier_for_size, tier_for_size_with_thresholds,
    };
    use gtk4::gio;

    #[test]
    fn search_at_minus_one_returns_ok() {
        assert!(char_count_supports_search(SEARCH_CHAR_LIMIT - 1));
    }

    #[test]
    fn search_at_exact_returns_ok() {
        assert!(char_count_supports_search(SEARCH_CHAR_LIMIT));
    }

    #[test]
    fn search_at_plus_one_returns_too_large() {
        assert!(!char_count_supports_search(SEARCH_CHAR_LIMIT + 1));
    }

    #[test]
    fn open_at_minus_one_returns_ok() {
        assert_eq!(
            open_decision_for_size(EDITOR_HARD_LIMIT_BYTES - 1),
            OpenDecision::Editor {
                tier: FileTier::Medium,
            }
        );
    }

    #[test]
    fn open_at_exact_returns_ok() {
        assert_eq!(
            open_decision_for_size(EDITOR_HARD_LIMIT_BYTES),
            OpenDecision::Viewer {
                tier: FileTier::Large,
                edit_allowed: true,
            }
        );
    }

    #[test]
    fn open_at_plus_one_returns_too_large() {
        assert_eq!(
            open_decision_for_size(EDITOR_HARD_LIMIT_BYTES + 1),
            OpenDecision::Viewer {
                tier: FileTier::Large,
                edit_allowed: false,
            }
        );
    }

    #[test]
    fn tier_boundaries_match_v15_policy() {
        assert_eq!(tier_for_size(SMALL_FILE_LIMIT_BYTES - 1), FileTier::Small);
        assert_eq!(tier_for_size(SMALL_FILE_LIMIT_BYTES), FileTier::Medium);
        assert_eq!(tier_for_size(MEDIUM_FILE_LIMIT_BYTES), FileTier::Large);
        assert_eq!(
            tier_for_size(EDITOR_POLICY_CEILING_BYTES),
            FileTier::VeryLarge
        );
        assert_eq!(
            tier_for_size(EDITOR_POLICY_CEILING_BYTES + 1),
            FileTier::ViewerOnly
        );
    }

    #[test]
    fn custom_thresholds_are_ordered_inside_policy_bounds() {
        let thresholds = FileThresholds::from_mib(0, 1_000, 1, 10_000);

        assert_eq!(thresholds.full_feature, MIB);
        assert_eq!(thresholds.editor, 25 * MIB);
        assert_eq!(thresholds.strong_warning, 26 * MIB);
        assert_eq!(thresholds.viewer_only, 500 * MIB);
    }

    #[test]
    fn custom_thresholds_drive_soft_open_tiers_only() {
        let thresholds = FileThresholds::from_mib(2, 6, 12, 16);

        assert_eq!(
            tier_for_size_with_thresholds(2 * MIB - 1, &thresholds),
            FileTier::Small
        );
        assert_eq!(
            tier_for_size_with_thresholds(2 * MIB, &thresholds),
            FileTier::Medium
        );
        assert_eq!(
            tier_for_size_with_thresholds(6 * MIB, &thresholds),
            FileTier::Large
        );
        assert_eq!(
            open_decision_for_size_with_thresholds(EDITOR_HARD_LIMIT_BYTES + 1, &thresholds),
            OpenDecision::Viewer {
                tier: FileTier::ViewerOnly,
                edit_allowed: false,
            }
        );
    }

    #[test]
    fn custom_thresholds_cannot_raise_measured_edit_cap() {
        let thresholds = FileThresholds::from_mib(5, 75, 76, 500);

        assert_eq!(
            open_decision_for_size_with_thresholds(
                EDITOR_HARD_LIMIT_BYTES.saturating_add(1),
                &thresholds,
            ),
            OpenDecision::Viewer {
                tier: FileTier::Large,
                edit_allowed: false,
            }
        );
    }

    #[test]
    fn open_plan_query_distinguishes_non_regular_files() {
        let info = gio::FileInfo::new();
        info.set_file_type(gio::FileType::Directory);
        info.set_size(12);

        assert_eq!(file_info_open_plan(&info), OpenPlanQueryResult::NonRegular);
    }

    #[test]
    fn open_plan_query_requires_size_attribute_for_regular_files() {
        let info = gio::FileInfo::new();
        info.set_file_type(gio::FileType::Regular);

        assert_eq!(
            file_info_open_plan(&info),
            OpenPlanQueryResult::SizeUnavailable
        );
    }

    #[test]
    fn open_plan_query_rejects_negative_regular_file_size() {
        let info = gio::FileInfo::new();
        info.set_file_type(gio::FileType::Regular);
        info.set_size(-1);

        assert_eq!(
            file_info_open_plan(&info),
            OpenPlanQueryResult::SizeUnavailable
        );
    }

    #[test]
    fn open_plan_query_returns_known_size_for_regular_file_size() {
        let info = gio::FileInfo::new();
        info.set_file_type(gio::FileType::Regular);
        info.set_size(42);

        assert_eq!(
            file_info_open_plan(&info),
            OpenPlanQueryResult::KnownSize(42)
        );
    }

    #[test]
    fn editor_and_autosave_caps_remain_code_owned() {
        assert!(file_size_supports_open(EDITOR_HARD_LIMIT_BYTES));
        assert!(!file_size_supports_open(EDITOR_HARD_LIMIT_BYTES + 1));
        assert!(i32::try_from(MEDIUM_FILE_LIMIT_BYTES).is_ok_and(autosave_supports_current_size));
        assert!(
            match i32::try_from(MEDIUM_FILE_LIMIT_BYTES.saturating_add(1)) {
                Ok(size) => !autosave_supports_current_size(size),
                Err(_error) => true,
            }
        );
    }

    #[test]
    fn mib_conversion_never_wraps_negative_preferences() {
        assert_eq!(mib_to_bytes(-1), 0);
        assert_eq!(mib_to_bytes(2), 2 * MIB);
    }

    #[test]
    fn feature_gates_follow_small_tier() {
        assert!(markdown_preview_enabled(SMALL_FILE_LIMIT_BYTES - 1));
        assert!(!markdown_preview_enabled(SMALL_FILE_LIMIT_BYTES));
    }

    #[test]
    fn save_snapshot_char_count_uses_open_limit() {
        assert!(
            i32::try_from(SAVE_SNAPSHOT_LIMIT_BYTES)
                .is_ok_and(buffer_char_count_supports_save_snapshot)
        );
        assert!(match i32::try_from(SAVE_SNAPSHOT_LIMIT_BYTES + 1) {
            Ok(value) => !buffer_char_count_supports_save_snapshot(value),
            Err(_) => true,
        });
    }

    #[test]
    fn save_snapshot_text_len_uses_open_limit() {
        assert!(
            usize::try_from(SAVE_SNAPSHOT_LIMIT_BYTES).is_ok_and(text_len_supports_save_snapshot)
        );
        assert!(match usize::try_from(SAVE_SNAPSHOT_LIMIT_BYTES + 1) {
            Ok(value) => !text_len_supports_save_snapshot(value),
            Err(_) => true,
        });
    }

    #[test]
    fn loaded_text_cap_uses_editor_hard_limit() {
        assert!(
            usize::try_from(OPEN_FILE_LIMIT_BYTES).is_ok_and(loaded_text_supports_editor_hard_cap)
        );
        assert!(match usize::try_from(OPEN_FILE_LIMIT_BYTES + 1) {
            Ok(value) => !loaded_text_supports_editor_hard_cap(value),
            Err(_) => true,
        });
    }
}
