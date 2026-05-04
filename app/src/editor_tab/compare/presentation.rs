use gettextrs::{npgettext, pgettext};

use super::model::{DiffRowKind, DiffRowModel};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DiffPresentation {
    pub(super) reference_text: String,
    pub(super) current_text: String,
    pub(super) reference_line_numbers: Vec<Option<usize>>,
    pub(super) current_line_numbers: Vec<Option<usize>>,
    placeholder_markers: Vec<Option<PlaceholderMarker>>,
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
    pub(super) fn placeholder_marker(
        &self,
        side: PresentationSide,
        row: usize,
    ) -> Option<PlaceholderMarker> {
        self.placeholder_markers
            .get(row)
            .and_then(|marker| *marker)
            .filter(|marker| marker.side == side)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PlaceholderMarker {
    pub(super) side: PresentationSide,
    pub(super) run_len: usize,
}

pub(super) fn build_presentation(
    model: &DiffRowModel,
    reference_lines: &[&str],
    current_lines: &[&str],
) -> DiffPresentation {
    if model.too_large || model.rows.is_empty() {
        return DiffPresentation::empty();
    }
    let mut reference_rows = Vec::with_capacity(model.rows.len());
    let mut current_rows = Vec::with_capacity(model.rows.len());
    let mut reference_numbers = Vec::with_capacity(model.rows.len());
    let mut current_numbers = Vec::with_capacity(model.rows.len());
    let mut placeholder_markers = Vec::with_capacity(model.rows.len());
    let mut placeholder_count = 0;
    let mut row_index = 0;

    while row_index < model.rows.len() {
        let row = &model.rows[row_index];
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
                placeholder_markers.push(None);
                row_index += 1;
            }
            DiffRowKind::ReferenceOnly => {
                let run_len = same_kind_run_len(model, row_index, DiffRowKind::ReferenceOnly);
                for offset in 0..run_len {
                    let row = &model.rows[row_index + offset];
                    push_line(
                        &mut reference_rows,
                        &mut reference_numbers,
                        reference_lines,
                        row.reference_line,
                        &mut placeholder_count,
                    );
                    let marker = (offset == 0).then_some(PlaceholderMarker {
                        side: PresentationSide::Current,
                        run_len,
                    });
                    push_placeholder(
                        &mut current_rows,
                        &mut current_numbers,
                        &mut placeholder_count,
                        marker.map(marker_text).as_deref(),
                    );
                    placeholder_markers.push(marker);
                }
                row_index += run_len;
            }
            DiffRowKind::CurrentOnly => {
                let run_len = same_kind_run_len(model, row_index, DiffRowKind::CurrentOnly);
                for offset in 0..run_len {
                    let row = &model.rows[row_index + offset];
                    let marker = (offset == 0).then_some(PlaceholderMarker {
                        side: PresentationSide::Reference,
                        run_len,
                    });
                    push_placeholder(
                        &mut reference_rows,
                        &mut reference_numbers,
                        &mut placeholder_count,
                        marker.map(marker_text).as_deref(),
                    );
                    placeholder_markers.push(marker);
                    push_line(
                        &mut current_rows,
                        &mut current_numbers,
                        current_lines,
                        row.current_line,
                        &mut placeholder_count,
                    );
                }
                row_index += run_len;
            }
        }
    }

    DiffPresentation {
        reference_text: reference_rows.join("\n"),
        current_text: current_rows.join("\n"),
        reference_line_numbers: reference_numbers,
        current_line_numbers: current_numbers,
        placeholder_markers,
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
        push_placeholder(rows, numbers, placeholder_count, None);
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
    text: Option<&str>,
) {
    rows.push(text.unwrap_or_default().to_string());
    numbers.push(None);
    *placeholder_count += 1;
}

fn same_kind_run_len(model: &DiffRowModel, start: usize, kind: DiffRowKind) -> usize {
    model.rows[start..]
        .iter()
        .take_while(|row| row.kind == kind)
        .count()
}

fn marker_text(marker: PlaceholderMarker) -> String {
    match (marker.side, marker.run_len) {
        (PresentationSide::Reference, 1) => pgettext("compare placeholder", "Only in current"),
        (PresentationSide::Reference, len) => {
            // TRANSLATORS: Shown once on the empty reference side for a run
            // of lines that exists only in the current file.
            npgettext(
                "compare placeholder",
                "%d line only in current",
                "%d lines only in current",
                plural_count(len),
            )
            .replace("%d", &len.to_string())
        }
        (PresentationSide::Current, 1) => pgettext("compare placeholder", "Only in reference"),
        (PresentationSide::Current, len) => {
            // TRANSLATORS: Shown once on the empty current side for a run
            // of lines that exists only in the reference file.
            npgettext(
                "compare placeholder",
                "%d line only in reference",
                "%d lines only in reference",
                plural_count(len),
            )
            .replace("%d", &len.to_string())
        }
    }
}

fn plural_count(value: usize) -> u32 {
    u32::try_from(value).map_or(u32::MAX, |value| value)
}

fn strip_line_ending(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::PresentationSide;
    use crate::editor_tab::compare::diff::compute_diff;

    #[test]
    fn current_only_uses_reference_placeholders() {
        let presentation = compute_diff("", "a\nb\n").presentation;

        assert_eq!(presentation.reference_text, "2 lines only in current\n");
        assert_eq!(presentation.current_text, "a\nb");
        assert_eq!(presentation.placeholder_count, 2);
        assert_eq!(
            presentation
                .placeholder_marker(PresentationSide::Reference, 0)
                .map(|marker| marker.run_len),
            Some(2)
        );
        assert_eq!(
            presentation.placeholder_marker(PresentationSide::Reference, 1),
            None
        );
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
        let presentation = compute_diff("a\nb\n", "").presentation;

        assert_eq!(presentation.reference_text, "a\nb");
        assert_eq!(presentation.current_text, "2 lines only in reference\n");
        assert_eq!(presentation.placeholder_count, 2);
        assert_eq!(
            presentation
                .placeholder_marker(PresentationSide::Current, 0)
                .map(|marker| marker.run_len),
            Some(2)
        );
        assert_eq!(
            presentation.placeholder_marker(PresentationSide::Current, 1),
            None
        );
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
        let computation = compute_diff(left, right);
        let model = computation.model;
        let presentation = computation.presentation;

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
        let presentation = compute_diff(left, right).presentation;

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

    #[test]
    fn marker_metadata_does_not_mark_real_blank_lines() {
        let left = "a\n\n";
        let right = "a\n\nb\n";
        let presentation = compute_diff(left, right).presentation;

        let real_blank = (0..presentation.line_count()).find(|row| {
            presentation.line_number(PresentationSide::Reference, *row) == Some(2)
                && presentation.line_number(PresentationSide::Current, *row) == Some(2)
        });
        let marker_count = (0..presentation.line_count())
            .filter(|row| {
                presentation
                    .placeholder_marker(PresentationSide::Reference, *row)
                    .is_some()
                    || presentation
                        .placeholder_marker(PresentationSide::Current, *row)
                        .is_some()
            })
            .count();

        assert!(real_blank.is_some());
        assert_eq!(marker_count, 1);
    }
}
