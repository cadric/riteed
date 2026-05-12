#[cfg(test)]
use std::cell::Cell;

use similar::{DiffTag, TextDiff};

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
    let normalized_reference;
    let normalized_current;
    let (diff_reference, diff_current) = if options.ignore_leading_trailing_whitespace {
        normalized_reference = trim_line_sides_text(reference_text);
        normalized_current = trim_line_sides_text(current_text);
        (normalized_reference.as_str(), normalized_current.as_str())
    } else {
        (reference_text, current_text)
    };
    let diff = compute_line_diff(diff_reference, diff_current);
    let ops = line_ops(diff.ops());
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

#[cfg(test)]
pub(super) fn compute_diff_row_model(reference_text: &str, current_text: &str) -> DiffRowModel {
    compute_diff(reference_text, current_text).model
}

fn compute_line_diff<'text>(
    reference_text: &'text str,
    current_text: &'text str,
) -> TextDiff<'text, 'text, 'text, str> {
    #[cfg(test)]
    LINE_DIFF_CALLS.with(|calls| calls.set(calls.get() + 1));
    TextDiff::from_lines(reference_text, current_text)
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
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

fn line_slices(text: &str) -> Vec<&str> {
    if text.is_empty() {
        Vec::new()
    } else {
        text.split_inclusive('\n').collect()
    }
}

fn trim_line_sides_text(text: &str) -> String {
    let mut normalized = String::new();
    for line in line_slices(text) {
        let (content, ending) = split_line_ending(line);
        normalized.push_str(content.trim());
        normalized.push_str(ending);
    }
    normalized
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(content) = line.strip_suffix("\r\n") {
        return (content, "\r\n");
    }
    if let Some(content) = line.strip_suffix('\n') {
        return (content, "\n");
    }
    (line, "")
}

#[cfg(test)]
mod tests {
    use super::{
        DiffOptions, DiffSkipReason, LINE_DIFF_CALLS, MAX_COMPARE_BYTES, MAX_COMPARE_LINE_PRODUCT,
        compute_diff, compute_diff_with_options,
    };

    fn reset_line_diff_calls() {
        LINE_DIFF_CALLS.with(|calls| calls.set(0));
    }

    fn line_diff_calls() -> usize {
        LINE_DIFF_CALLS.with(std::cell::Cell::get)
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

    const fn integer_sqrt(value: usize) -> usize {
        let mut candidate: usize = 0;
        while candidate.saturating_mul(candidate) <= value {
            candidate += 1;
        }
        candidate.saturating_sub(1)
    }
}
