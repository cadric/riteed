use super::review_session::{ReviewFileInput, ReviewSession};
use crate::editor_tab::{
    ReviewFileId, ReviewFileSpec, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec,
};
use crate::git_status::GitFileStatus;
use crate::settings::{CompareReviewSettingsSnapshot, CompareViewMode};
use gettextrs::pgettext;

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

#[test]
fn review_path_newline_keeps_rendered_line_map_aligned() {
    let raw_path = b"a\nb.txt".to_vec();
    let session = ReviewSession::from_inputs(
        &spec(),
        vec![ReviewFileInput::skipped(
            ReviewFileId::new(ReviewKind::Staged, raw_path.clone()),
            GitFileStatus::Modified,
            "too large",
        )],
    );

    let rendered = session.render_text();
    assert_eq!(rendered.lines().count(), session.rendered_lines.len());
    assert_eq!(
        rendered.lines().next(),
        Some("a\\nb.txt (M, 0 additions, 0 removals)")
    );
    assert!(!rendered.contains("a\nb.txt"));
    assert_eq!(session.files[0].file_id.raw_path, raw_path);
}

#[test]
fn review_path_display_preserves_escape_and_fallback_contracts() {
    let control_path = "safe\\path\t\u{202e}gnp.txt".as_bytes().to_vec();
    let invalid_path = b"dir/\xff.bin".to_vec();
    let empty_path = Vec::new();
    let session = ReviewSession::from_inputs(
        &spec(),
        vec![
            ReviewFileInput::skipped(
                ReviewFileId::new(ReviewKind::Staged, control_path.clone()),
                GitFileStatus::Modified,
                "too large",
            ),
            ReviewFileInput::skipped(
                ReviewFileId::new(ReviewKind::Staged, invalid_path.clone()),
                GitFileStatus::Modified,
                "too large",
            ),
            ReviewFileInput::skipped(
                ReviewFileId::new(ReviewKind::Staged, empty_path.clone()),
                GitFileStatus::Modified,
                "too large",
            ),
        ],
    );

    let rendered = session.render_text();
    let header_paths = rendered
        .lines()
        .step_by(2)
        .map(|line| line.split_once(" (").map_or(line, |(path, _suffix)| path))
        .collect::<Vec<_>>();
    let invalid_fallback = pgettext("git path fallback", "Invalid path encoding");
    let empty_sentinel = pgettext("git review path", "Review limit");
    assert_eq!(
        header_paths,
        vec![
            "safe\\\\path\\t\\u{202e}gnp.txt",
            invalid_fallback.as_str(),
            empty_sentinel.as_str(),
        ]
    );
    let change_items = session.change_list_items();
    let change_labels = change_items
        .chunks_exact(2)
        .map(|items| items[0].label.as_str())
        .collect::<Vec<_>>();
    assert_eq!(change_labels, header_paths);
    assert_eq!(session.files[0].file_id.raw_path, control_path);
    assert_eq!(session.files[1].file_id.raw_path, invalid_path);
    assert_eq!(session.files[2].file_id.raw_path, empty_path);
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
