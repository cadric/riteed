#[cfg(test)]
use std::cell::Cell;

use similar::{DiffTag, DiffableStr, TextDiff};

use super::model::{DiffLineOp, DiffLineTag, DiffRowModel, build_row_model};
#[cfg(test)]
use super::presentation::{DiffPresentation, build_presentation};

const MAX_COMPARE_BYTES: usize = 1_000_000;
const MAX_COMPARE_LINES: usize = 20_000;
const MAX_COMPARE_LINE_PRODUCT: usize = 10_000_000;

#[cfg(test)]
std::thread_local! {
    static LINE_DIFF_CALLS: Cell<usize> = const { Cell::new(0) };
}

pub(super) struct DiffComputation {
    pub(super) model: DiffRowModel,
    #[cfg(test)]
    pub(super) presentation: DiffPresentation,
    pub(super) hidden_trim_whitespace_differences: bool,
    pub(super) skip_reason: Option<DiffSkipReason>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct DiffOptions {
    pub(super) ignore_leading_trailing_whitespace: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiffSkipReason {
    Bytes,
    Lines,
    Computation,
}

// Keep this as the single runtime full-line diff entry point so near-limit
// compares do not duplicate the expensive line-diff work.
// PARSER-BOUNDARY: id=diff_compute
#[cfg(test)]
pub(super) fn compute_diff(reference_text: &str, current_text: &str) -> DiffComputation {
    compute_diff_with_options(reference_text, current_text, DiffOptions::default())
}

pub(super) fn compute_diff_with_options(
    reference_text: &str,
    current_text: &str,
    options: DiffOptions,
) -> DiffComputation {
    if let Some(skip_reason) = compare_skip_reason(reference_text, current_text) {
        return DiffComputation {
            model: DiffRowModel::too_large(),
            #[cfg(test)]
            presentation: DiffPresentation::empty(),
            hidden_trim_whitespace_differences: false,
            skip_reason: Some(skip_reason),
        };
    }

    let reference_lines = line_slices(reference_text);
    let current_lines = line_slices(current_text);
    let ops = compute_line_ops(
        reference_text,
        current_text,
        &reference_lines,
        &current_lines,
        options.ignore_leading_trailing_whitespace,
    );
    let model = build_row_model(&ops, &reference_lines, &current_lines);
    #[cfg(test)]
    let presentation = build_presentation(&model, &reference_lines, &current_lines);
    let hidden_trim_whitespace_differences = options.ignore_leading_trailing_whitespace
        && model.changed_row_count() == 0
        && reference_text != current_text;

    DiffComputation {
        model,
        #[cfg(test)]
        presentation,
        hidden_trim_whitespace_differences,
        skip_reason: None,
    }
}

#[cfg(feature = "fuzzing")]
pub(super) fn fuzz_compute_diff(reference_text: &str, current_text: &str) -> (bool, usize, bool) {
    let default_computation =
        compute_diff_with_options(reference_text, current_text, DiffOptions::default());
    let whitespace_computation = compute_diff_with_options(
        reference_text,
        current_text,
        DiffOptions {
            ignore_leading_trailing_whitespace: true,
        },
    );
    let reference_lines = line_slices(reference_text).len();
    let current_lines = line_slices(current_text).len();
    let mappings_valid = if default_computation.skip_reason.is_some() {
        default_computation.model.too_large && whitespace_computation.model.too_large
    } else {
        default_computation
            .model
            .has_complete_line_identity(reference_lines, current_lines)
            && whitespace_computation
                .model
                .has_complete_line_identity(reference_lines, current_lines)
    };
    (
        default_computation.skip_reason.is_some(),
        default_computation.model.changed_row_count(),
        mappings_valid,
    )
}

#[cfg(test)]
pub(super) fn compute_diff_row_model(reference_text: &str, current_text: &str) -> DiffRowModel {
    compute_diff(reference_text, current_text).model
}

fn compute_line_ops(
    reference_text: &str,
    current_text: &str,
    reference_lines: &[&str],
    current_lines: &[&str],
    ignore_leading_trailing_whitespace: bool,
) -> Vec<DiffLineOp> {
    #[cfg(test)]
    LINE_DIFF_CALLS.with(|calls| calls.set(calls.get() + 1));
    if ignore_leading_trailing_whitespace {
        let normalized_reference: Vec<String> = reference_lines
            .iter()
            .map(|line| normalized_line(line))
            .collect();
        let normalized_current: Vec<String> = current_lines
            .iter()
            .map(|line| normalized_line(line))
            .collect();
        let old: Vec<&str> = normalized_reference.iter().map(String::as_str).collect();
        let new: Vec<&str> = normalized_current.iter().map(String::as_str).collect();
        let diff = TextDiff::configure().diff_slices(&old, &new);
        return line_ops(diff.ops());
    }
    let diff = TextDiff::from_lines(reference_text, current_text);
    line_ops(diff.ops())
}

fn line_ops(ops: &[similar::DiffOp]) -> Vec<DiffLineOp> {
    ops.iter()
        .map(|op| {
            let (tag, reference_range, current_range) = op.as_tag_tuple();
            DiffLineOp {
                tag: match tag {
                    DiffTag::Equal => DiffLineTag::Equal,
                    DiffTag::Delete => DiffLineTag::Delete,
                    DiffTag::Insert => DiffLineTag::Insert,
                    DiffTag::Replace => DiffLineTag::Replace,
                },
                reference_range,
                current_range,
            }
        })
        .collect()
}

fn compare_skip_reason(reference_text: &str, current_text: &str) -> Option<DiffSkipReason> {
    if reference_text.len().saturating_add(current_text.len()) > MAX_COMPARE_BYTES {
        return Some(DiffSkipReason::Bytes);
    }
    let reference_lines = line_count(reference_text);
    let current_lines = line_count(current_text);
    if reference_lines.saturating_add(current_lines) > MAX_COMPARE_LINES {
        return Some(DiffSkipReason::Lines);
    }
    if reference_lines.saturating_mul(current_lines) > MAX_COMPARE_LINE_PRODUCT {
        return Some(DiffSkipReason::Computation);
    }
    None
}

fn line_count(text: &str) -> usize {
    line_slices(text).len()
}

pub(super) fn line_slices(text: &str) -> Vec<&str> {
    text.tokenize_lines()
}

fn normalized_line(line: &str) -> String {
    let (content, ending) = split_line_ending(line);
    format!("{}{ending}", content.trim())
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        return (content, "\r\n");
    }
    if let Some(content) = line.strip_suffix('\n') {
        return (content, "\n");
    }
    if let Some(content) = line.strip_suffix('\r') {
        return (content, "\r");
    }
    (line, "")
}

#[cfg(test)]
mod tests {
    use super::{
        DiffOptions, DiffSkipReason, LINE_DIFF_CALLS, MAX_COMPARE_BYTES, MAX_COMPARE_LINE_PRODUCT,
        MAX_COMPARE_LINES, compute_diff, compute_diff_with_options,
    };
    use proptest::prelude::*;
    use proptest::test_runner::FileFailurePersistence;

    fn reset_line_diff_calls() {
        LINE_DIFF_CALLS.with(|calls| calls.set(0));
    }

    fn line_diff_calls() -> usize {
        LINE_DIFF_CALLS.with(std::cell::Cell::get)
    }

    fn bounded_proptest_config() -> ProptestConfig {
        ProptestConfig {
            cases: 64,
            failure_persistence: Some(Box::new(FileFailurePersistence::SourceParallel(
                ".proptest-regressions",
            ))),
            ..ProptestConfig::default()
        }
    }

    fn repeated_lines(count: usize, line: &str) -> String {
        line.repeat(count)
    }

    #[test]
    fn performance_guard_skips_large_inputs() {
        reset_line_diff_calls();

        let large = "x".repeat(MAX_COMPARE_BYTES + 1);
        let computation = compute_diff(&large, "");
        let model = computation.model;

        assert!(model.too_large);
        assert_eq!(computation.skip_reason, Some(DiffSkipReason::Bytes));
        assert!(model.hunks.is_empty());
        assert_eq!(line_diff_calls(), 0);
    }

    #[test]
    fn bytes_at_minus_one_returns_ok() {
        let reference = "x".repeat(MAX_COMPARE_BYTES - 1);
        let computation = compute_diff(&reference, "");

        assert_eq!(computation.skip_reason, None);
    }

    #[test]
    fn bytes_at_exact_returns_ok() {
        let reference = "x".repeat(MAX_COMPARE_BYTES);
        let computation = compute_diff(&reference, "");

        assert_eq!(computation.skip_reason, None);
    }

    #[test]
    fn bytes_at_plus_one_returns_too_large() {
        reset_line_diff_calls();

        let reference = "x".repeat(MAX_COMPARE_BYTES + 1);
        let computation = compute_diff(&reference, "");

        assert_eq!(computation.skip_reason, Some(DiffSkipReason::Bytes));
        assert_eq!(line_diff_calls(), 0);
    }

    #[test]
    fn lines_at_minus_one_returns_ok() {
        let reference = repeated_lines(MAX_COMPARE_LINES - 1, "x\n");
        let computation = compute_diff(&reference, "");

        assert_eq!(computation.skip_reason, None);
    }

    #[test]
    fn lines_at_exact_returns_ok() {
        let reference = repeated_lines(MAX_COMPARE_LINES, "x\n");
        let computation = compute_diff(&reference, "");

        assert_eq!(computation.skip_reason, None);
    }

    #[test]
    fn lines_at_plus_one_returns_too_large() {
        reset_line_diff_calls();

        let reference = repeated_lines(MAX_COMPARE_LINES + 1, "x\n");
        let computation = compute_diff(&reference, "");

        assert_eq!(computation.skip_reason, Some(DiffSkipReason::Lines));
        assert_eq!(line_diff_calls(), 0);
    }

    #[test]
    fn line_product_at_minus_one_returns_ok() {
        let reference = repeated_lines(9_999, "x\n");
        let current = repeated_lines(1_000, "x\n");
        let computation = compute_diff(&reference, &current);

        assert_eq!(computation.skip_reason, None);
    }

    #[test]
    fn line_product_at_exact_returns_ok() {
        let reference = repeated_lines(10_000, "x\n");
        let current = repeated_lines(1_000, "x\n");
        let computation = compute_diff(&reference, &current);

        assert_eq!(computation.skip_reason, None);
    }

    #[test]
    fn line_product_at_plus_one_returns_too_large() {
        reset_line_diff_calls();

        let reference = repeated_lines(10_001, "x\n");
        let current = repeated_lines(1_000, "x\n");
        let computation = compute_diff(&reference, &current);

        assert_eq!(computation.skip_reason, Some(DiffSkipReason::Computation));
        assert_eq!(line_diff_calls(), 0);
    }

    #[test]
    fn line_limit_reports_skip_reason() {
        reset_line_diff_calls();

        let many = "x\n".repeat(super::MAX_COMPARE_LINES + 1);
        let computation = compute_diff(&many, "");

        assert!(computation.model.too_large);
        assert_eq!(computation.skip_reason, Some(DiffSkipReason::Lines));
        assert_eq!(line_diff_calls(), 0);
    }

    #[test]
    fn computation_limit_reports_skip_reason() {
        reset_line_diff_calls();

        let line_count = integer_sqrt(MAX_COMPARE_LINE_PRODUCT).saturating_add(1);
        let reference = "x\n".repeat(line_count);
        let current = "y\n".repeat(line_count);
        let computation = compute_diff(&reference, &current);

        assert!(computation.model.too_large);
        assert_eq!(computation.skip_reason, Some(DiffSkipReason::Computation));
        assert_eq!(line_diff_calls(), 0);
    }

    #[test]
    fn changed_compare_builds_model_and_presentation_from_one_line_diff() {
        reset_line_diff_calls();

        let computation = compute_diff("same\nold\n", "same\nnew\ncurrent\n");

        assert_eq!(line_diff_calls(), 1);
        assert_eq!(
            computation.model.rows.len(),
            computation.presentation.line_count()
        );
        assert_eq!(computation.presentation.placeholder_count, 1);
    }

    #[test]
    fn equal_compare_uses_one_line_diff_without_placeholders() {
        reset_line_diff_calls();

        let computation = compute_diff("same\ntext\n", "same\ntext\n");

        assert_eq!(line_diff_calls(), 1);
        assert_eq!(computation.model.changed_row_count(), 0);
        assert_eq!(computation.presentation.placeholder_count, 0);
        assert_eq!(
            computation.model.rows.len(),
            computation.presentation.line_count()
        );
    }

    #[test]
    fn empty_compare_uses_at_most_one_line_diff() {
        reset_line_diff_calls();

        let computation = compute_diff("", "");

        assert!(line_diff_calls() <= 1);
        assert!(computation.model.rows.is_empty());
        assert_eq!(computation.presentation, super::DiffPresentation::empty());
    }

    #[test]
    fn leading_trailing_whitespace_option_hides_trim_only_changes() {
        let computation = compute_diff_with_options(
            "alpha\n beta \n",
            "alpha\nbeta\n",
            DiffOptions {
                ignore_leading_trailing_whitespace: true,
            },
        );

        assert_eq!(computation.model.changed_row_count(), 0);
        assert!(computation.hidden_trim_whitespace_differences);
    }

    #[test]
    fn leading_trailing_whitespace_option_keeps_original_render_text() {
        let computation = compute_diff_with_options(
            "alpha\n beta \n",
            "alpha\nzeta\n",
            DiffOptions {
                ignore_leading_trailing_whitespace: true,
            },
        );

        assert_eq!(computation.model.changed_row_count(), 1);
        assert!(computation.presentation.reference_text.contains(" beta "));
    }

    #[test]
    fn whitespace_option_does_not_hide_internal_whitespace_changes() {
        let computation = compute_diff_with_options(
            "alpha beta\n",
            "alphabeta\n",
            DiffOptions {
                ignore_leading_trailing_whitespace: true,
            },
        );

        assert_eq!(computation.model.changed_row_count(), 1);
        assert!(!computation.hidden_trim_whitespace_differences);
    }

    proptest! {
        #![proptest_config(bounded_proptest_config())]

        #[test]
        fn proptest_compute_diff_respects_caps(
            reference in prop::collection::vec(prop_oneof![Just('x'), Just('\n'), Just('\r'), Just('\0')], 0..8192)
                .prop_map(|chars| chars.into_iter().collect::<String>()),
            current in prop::collection::vec(prop_oneof![Just('y'), Just('\n'), Just('\r'), Just('\0')], 0..8192)
                .prop_map(|chars| chars.into_iter().collect::<String>()),
        ) {
            let computation = compute_diff_with_options(
                &reference,
                &current,
                DiffOptions::default(),
            );
            let expected_skip = super::compare_skip_reason(&reference, &current);

            prop_assert_eq!(computation.skip_reason, expected_skip);
            prop_assert_eq!(computation.model.too_large, expected_skip.is_some());
        }
    }

    #[test]
    fn fuzz_regression_lone_cr_line_splitting_matches_diff_ops() {
        let reference = String::from_utf8_lossy(b"al-pha}\xe1\na");
        let current = "\r\0\0\0\0\0\0\0\n";

        let computation = compute_diff(&reference, current);

        assert_eq!(computation.skip_reason, None);
        assert!(!computation.model.too_large);
    }

    const fn integer_sqrt(value: usize) -> usize {
        let mut candidate: usize = 0;
        while candidate.saturating_mul(candidate) <= value {
            candidate += 1;
        }
        candidate.saturating_sub(1)
    }
}
