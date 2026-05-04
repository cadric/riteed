#[cfg(test)]
use std::cell::Cell;

use similar::{DiffTag, TextDiff};

use super::model::{DiffLineOp, DiffLineTag, DiffRowModel, build_row_model};
use super::presentation::{DiffPresentation, build_presentation};

const MAX_COMPARE_BYTES: usize = 1_000_000;
const MAX_COMPARE_LINES: usize = 20_000;

#[cfg(test)]
std::thread_local! {
    static LINE_DIFF_CALLS: Cell<usize> = const { Cell::new(0) };
}

pub(super) struct DiffComputation {
    pub(super) model: DiffRowModel,
    pub(super) presentation: DiffPresentation,
}

// Keep this as the single runtime full-line diff entry point. Building the row
// model and presentation from one TextDiff avoids duplicate near-limit work.
pub(super) fn compute_diff(reference_text: &str, current_text: &str) -> DiffComputation {
    if compare_too_large(reference_text, current_text) {
        return DiffComputation {
            model: DiffRowModel::too_large(),
            presentation: DiffPresentation::empty(),
        };
    }

    let (model, presentation) = {
        let diff = compute_line_diff(reference_text, current_text);
        let ops = line_ops(diff.ops());
        let model = build_row_model(&ops, diff.old_slices(), diff.new_slices());
        let presentation = build_presentation(&model, diff.old_slices(), diff.new_slices());
        (model, presentation)
    };

    DiffComputation {
        model,
        presentation,
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

fn compare_too_large(reference_text: &str, current_text: &str) -> bool {
    reference_text.len().saturating_add(current_text.len()) > MAX_COMPARE_BYTES
        || line_count(reference_text).saturating_add(line_count(current_text)) > MAX_COMPARE_LINES
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::{LINE_DIFF_CALLS, MAX_COMPARE_BYTES, compute_diff, compute_diff_row_model};

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
        let model = compute_diff_row_model(&large, "");

        assert!(model.too_large);
        assert!(model.hunks.is_empty());
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
}
