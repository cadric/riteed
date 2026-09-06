use super::diff::{DiffComputation, DiffOptions, compute_diff_with_options, line_slices};
use super::model::DiffSide;
use proptest::prelude::*;

const IGNORE_WHITESPACE: DiffOptions = DiffOptions {
    ignore_leading_trailing_whitespace: true,
};

fn assert_side_identity(computation: &DiffComputation, side: DiffSide, line_count: usize) {
    let model = &computation.model;
    for line in 0..line_count {
        let row = model.row_for_line(side, line);
        assert!(row.is_some(), "{side:?} line {line} has no row");
        let row = row.unwrap_or(model.rows.len());
        assert!(row < model.rows.len(), "{side:?} row {row} is out of range");
        assert_eq!(model.line_for_row(side, row), Some(line));
        let occurrences = match side {
            DiffSide::Reference => &model.row_to_reference_line,
            DiffSide::Current => &model.row_to_current_line,
        }
        .iter()
        .filter(|mapped| **mapped == Some(line))
        .count();
        assert_eq!(occurrences, 1, "{side:?} line {line} must map once");
    }
}

fn assert_identity(reference: &str, current: &str, options: DiffOptions) -> DiffComputation {
    let computation = compute_diff_with_options(reference, current, options);
    assert_eq!(computation.skip_reason, None);
    assert_side_identity(
        &computation,
        DiffSide::Reference,
        line_slices(reference).len(),
    );
    assert_side_identity(&computation, DiffSide::Current, line_slices(current).len());
    computation
}

#[test]
fn ignored_whitespace_tail_keeps_original_line_identity() {
    let computation = assert_identity("a\n   ", "a\nb", IGNORE_WHITESPACE);

    assert_eq!(computation.presentation.reference_text, "a\n   ");
    assert_eq!(computation.presentation.current_text, "a\nb");
}

#[test]
fn ignored_whitespace_tail_keeps_identity_when_reversed() {
    let computation = assert_identity("a\nb", "a\n   ", IGNORE_WHITESPACE);

    assert_eq!(computation.presentation.reference_text, "a\nb");
    assert_eq!(computation.presentation.current_text, "a\n   ");
}

#[test]
fn ignored_whitespace_only_text_keeps_its_token_against_empty() {
    let computation = assert_identity("   ", "", IGNORE_WHITESPACE);

    assert_eq!(computation.presentation.reference_text, "   ");
}

#[test]
fn ignored_whitespace_tail_keeps_token_against_trailing_newline() {
    let computation = assert_identity("a\n   ", "a\n", IGNORE_WHITESPACE);

    assert_eq!(computation.presentation.reference_text, "a\n   ");
}

#[test]
fn normalized_lines_preserve_lf_crlf_and_lone_cr_identity() {
    for (reference, current) in [
        ("a\n   ", "a\nb"),
        ("a\r\n   ", "a\r\nb"),
        ("a\r   ", "a\rb"),
        ("a\n\n   ", "a\n\nb"),
        ("a\n\u{2003}", "a\nb"),
        ("", ""),
        ("a\n", "a"),
    ] {
        assert_identity(reference, current, IGNORE_WHITESPACE);
    }
}

#[test]
fn lone_cr_presentation_uses_one_display_row_per_original_token() {
    let computation = assert_identity("a\r   ", "a\rb", IGNORE_WHITESPACE);

    assert_eq!(computation.presentation.reference_text, "a\n   ");
    assert_eq!(computation.presentation.current_text, "a\nb");
}

#[test]
fn disabled_whitespace_option_preserves_whitespace_changes_and_identity() {
    let computation = assert_identity(" a\n\n", "a\n", DiffOptions::default());

    assert!(computation.model.changed_row_count() > 0);
    assert!(!computation.hidden_trim_whitespace_differences);
}

proptest! {
    #[test]
    fn all_original_tokens_map_once_and_roundtrip(
        reference in prop::collection::vec(
            prop_oneof![
                Just('a'),
                Just(' '),
                Just('\t'),
                Just('\n'),
                Just('\r'),
                Just('\u{2003}'),
            ],
            0..128,
        ).prop_map(|chars| chars.into_iter().collect::<String>()),
        current in prop::collection::vec(
            prop_oneof![
                Just('b'),
                Just(' '),
                Just('\t'),
                Just('\n'),
                Just('\r'),
                Just('\u{2003}'),
            ],
            0..128,
        ).prop_map(|chars| chars.into_iter().collect::<String>()),
        ignore_leading_trailing_whitespace in any::<bool>(),
    ) {
        let options = DiffOptions {
            ignore_leading_trailing_whitespace,
        };

        assert_identity(&reference, &current, options);
    }
}
