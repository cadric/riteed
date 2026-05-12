use super::review_session::{ReviewFileInput, ReviewSession};
use crate::editor_tab::{
    ReviewFileId, ReviewFileSpec, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec,
};
use crate::git_status::GitFileStatus;
use crate::settings::{CompareReviewSettingsSnapshot, CompareViewMode};

#[test]
fn session_populates_file_display_models() {
    let spec = spec();
    let mut session = ReviewSession::from_inputs(
        &spec,
        vec![ReviewFileInput::file(
            ReviewFileId::new(ReviewKind::Staged, b"file.txt".to_vec()),
            GitFileStatus::Modified,
            Some(String::from("old\nsame\n")),
            Some(String::from("new\nsame\n")),
        )],
    );

    assert_eq!(session.file_count(), 1);
    assert!(session.files[0].diff_row_model.is_some());
    assert!(session.files[0].display_model.is_some());
    assert!(session.render_text().contains("file.txt"));

    let mut settings = spec.settings_snapshot;
    settings.collapse_unchanged = true;
    settings.context_lines = 1;
    session.rebuild_displays(settings);
    assert!(session.files[0].display_model.is_some());
}

#[test]
fn session_keeps_skip_reason_as_rendered_marker() {
    let spec = spec();
    let session = ReviewSession::from_inputs(
        &spec,
        vec![ReviewFileInput::skipped(
            ReviewFileId::new(ReviewKind::Staged, b"large.txt".to_vec()),
            GitFileStatus::Modified,
            "too large",
        )],
    );

    assert!(session.render_text().contains("too large"));
    assert_eq!(session.rendered_lines.len(), 2);
}

fn spec() -> ReviewTabSpec {
    ReviewTabSpec::new(
        ReviewKind::Staged,
        std::path::PathBuf::from("/repo"),
        7,
        ReviewSnapshotFingerprint::new("fingerprint"),
        vec![ReviewFileSpec::new(b"file.txt".to_vec())],
        CompareReviewSettingsSnapshot {
            view_mode: CompareViewMode::Unified,
            collapse_unchanged: false,
            context_lines: 3,
            ignore_leading_trailing_whitespace: false,
            word_wrap: false,
        },
    )
}
