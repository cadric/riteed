use similar::TextDiff;

use super::model::{DiffRowKind, DiffRowModel};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DiffPresentation {
    pub(super) reference_text: String,
    pub(super) current_text: String,
    pub(super) reference_line_numbers: Vec<Option<usize>>,
    pub(super) current_line_numbers: Vec<Option<usize>>,
    pub(super) placeholder_count: usize,
}

impl DiffPresentation {
    #[must_use]
    pub(super) fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub(super) fn line_number(&self, side: PresentationSide, row: usize) -> Option<usize> {
        let numbers = match side {
            PresentationSide::Reference => &self.reference_line_numbers,
            PresentationSide::Current => &self.current_line_numbers,
        };
        numbers.get(row).and_then(|line| *line)
    }

    #[must_use]
    pub(super) fn hatch_side_for_row(&self, row: usize) -> Option<PresentationSide> {
        let reference = self.reference_line_numbers.get(row)?;
        let current = self.current_line_numbers.get(row)?;
        match (reference.is_none(), current.is_none()) {
            (true, false) => Some(PresentationSide::Reference),
            (false, true) => Some(PresentationSide::Current),
            _ => None,
        }
    }

    #[must_use]
    pub(super) fn max_line_number(&self, side: PresentationSide) -> usize {
        let numbers = match side {
            PresentationSide::Reference => &self.reference_line_numbers,
            PresentationSide::Current => &self.current_line_numbers,
        };
        numbers.iter().filter_map(|line| *line).max().unwrap_or(1)
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn line_count(&self) -> usize {
        self.reference_line_numbers.len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PresentationSide {
    Reference,
    Current,
}

pub(super) fn build_presentation(
    model: &DiffRowModel,
    reference_text: &str,
    current_text: &str,
) -> DiffPresentation {
    if model.too_large || model.rows.is_empty() {
        return DiffPresentation::empty();
    }
    let diff = TextDiff::from_lines(reference_text, current_text);
    let reference_lines = diff.old_slices();
    let current_lines = diff.new_slices();
    let mut reference_rows = Vec::with_capacity(model.rows.len());
    let mut current_rows = Vec::with_capacity(model.rows.len());
    let mut reference_numbers = Vec::with_capacity(model.rows.len());
    let mut current_numbers = Vec::with_capacity(model.rows.len());
    let mut placeholder_count = 0;

    for row in &model.rows {
        match row.kind {
            DiffRowKind::Equal | DiffRowKind::Modify => {
                push_line(
                    &mut reference_rows,
                    &mut reference_numbers,
                    reference_lines,
                    row.reference_line,
                    &mut placeholder_count,
                );
                push_line(
                    &mut current_rows,
                    &mut current_numbers,
                    current_lines,
                    row.current_line,
                    &mut placeholder_count,
                );
            }
            DiffRowKind::ReferenceOnly => {
                push_line(
                    &mut reference_rows,
                    &mut reference_numbers,
                    reference_lines,
                    row.reference_line,
                    &mut placeholder_count,
                );
                push_placeholder(
                    &mut current_rows,
                    &mut current_numbers,
                    &mut placeholder_count,
                );
            }
            DiffRowKind::CurrentOnly => {
                push_placeholder(
                    &mut reference_rows,
                    &mut reference_numbers,
                    &mut placeholder_count,
                );
                push_line(
                    &mut current_rows,
                    &mut current_numbers,
                    current_lines,
                    row.current_line,
                    &mut placeholder_count,
                );
            }
        }
    }

    DiffPresentation {
        reference_text: reference_rows.join("\n"),
        current_text: current_rows.join("\n"),
        reference_line_numbers: reference_numbers,
        current_line_numbers: current_numbers,
        placeholder_count,
    }
}

fn push_line(
    rows: &mut Vec<String>,
    numbers: &mut Vec<Option<usize>>,
    source: &[&str],
    line: Option<usize>,
    placeholder_count: &mut usize,
) {
    let Some(line) = line else {
        push_placeholder(rows, numbers, placeholder_count);
        return;
    };
    let text = source
        .get(line)
        .map_or("", |value| strip_line_ending(value));
    rows.push(text.to_string());
    numbers.push(Some(line + 1));
}

fn push_placeholder(
    rows: &mut Vec<String>,
    numbers: &mut Vec<Option<usize>>,
    placeholder_count: &mut usize,
) {
    rows.push(String::new());
    numbers.push(None);
    *placeholder_count += 1;
}

fn strip_line_ending(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::{PresentationSide, build_presentation};
    use crate::editor_tab::compare::diff::compute_diff_row_model;

    #[test]
    fn current_only_uses_reference_placeholders() {
        let model = compute_diff_row_model("", "a\nb\n");
        let presentation = build_presentation(&model, "", "a\nb\n");

        assert_eq!(presentation.reference_text, "\n");
        assert_eq!(presentation.current_text, "a\nb");
        assert_eq!(presentation.placeholder_count, 2);
        assert_eq!(
            presentation.line_number(PresentationSide::Reference, 0),
            None
        );
        assert_eq!(
            presentation.line_number(PresentationSide::Current, 0),
            Some(1)
        );
    }

    #[test]
    fn reference_only_uses_current_placeholders() {
        let model = compute_diff_row_model("a\nb\n", "");
        let presentation = build_presentation(&model, "a\nb\n", "");

        assert_eq!(presentation.reference_text, "a\nb");
        assert_eq!(presentation.current_text, "\n");
        assert_eq!(presentation.placeholder_count, 2);
        assert_eq!(
            presentation.line_number(PresentationSide::Reference, 1),
            Some(2)
        );
        assert_eq!(presentation.line_number(PresentationSide::Current, 1), None);
    }

    #[test]
    fn mixed_rows_preserve_original_line_numbers() {
        let left = "same\nold\nleft\n";
        let right = "same\nnew\nright\n";
        let model = compute_diff_row_model(left, right);
        let presentation = build_presentation(&model, left, right);

        assert_eq!(presentation.line_count(), model.rows.len());
        assert_eq!(
            presentation.line_number(PresentationSide::Reference, 0),
            Some(1)
        );
        assert_eq!(
            presentation.line_number(PresentationSide::Current, 0),
            Some(1)
        );
        assert_eq!(
            presentation.reference_line_numbers.len(),
            presentation.current_line_numbers.len()
        );
    }

    #[test]
    fn hatch_side_targets_only_placeholder_side() {
        let left = "same\nold\nleft only\nblank follows\n\n";
        let right = "same\nnew\nblank follows\n\nright only\n";
        let model = compute_diff_row_model(left, right);
        let presentation = build_presentation(&model, left, right);

        let hatch_sides: Vec<Option<PresentationSide>> = (0..presentation.line_count())
            .map(|row| presentation.hatch_side_for_row(row))
            .collect();

        assert!(hatch_sides.contains(&Some(PresentationSide::Reference)));
        assert!(hatch_sides.contains(&Some(PresentationSide::Current)));
        assert_eq!(presentation.hatch_side_for_row(0), None);
        for row in 0..presentation.line_count() {
            if presentation
                .line_number(PresentationSide::Reference, row)
                .is_some()
                && presentation
                    .line_number(PresentationSide::Current, row)
                    .is_some()
            {
                assert_eq!(presentation.hatch_side_for_row(row), None);
            }
        }
    }
}
