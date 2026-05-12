use super::display::{CompareDisplayModel, CompareDisplayRow, DisplayRowId};
use super::inline::InlineRange;
use super::model::DiffSide;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UnifiedLineSide {
    Context,
    Removal,
    Addition,
    Collapsed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct UnifiedLine {
    pub(super) display_row_id: DisplayRowId,
    pub(super) logical_row: Option<usize>,
    pub(super) side: UnifiedLineSide,
    pub(super) reference_line: Option<usize>,
    pub(super) current_line: Option<usize>,
    pub(super) text: String,
    pub(super) inline_ranges: Vec<InlineRange>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct UnifiedPresentation {
    pub(super) lines: Vec<UnifiedLine>,
}

impl UnifiedPresentation {
    #[must_use]
    #[cfg(test)]
    pub(super) fn line_count(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn text(&self) -> String {
        self.lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub(super) fn build_unified_presentation(display: &CompareDisplayModel) -> UnifiedPresentation {
    let mut lines = Vec::new();
    for row in &display.rows {
        match row {
            CompareDisplayRow::FileBoundary(row) => lines.push(UnifiedLine {
                display_row_id: row.id.clone(),
                logical_row: None,
                side: UnifiedLineSide::Collapsed,
                reference_line: None,
                current_line: None,
                text: row.path.clone(),
                inline_ranges: Vec::new(),
            }),
            CompareDisplayRow::Content(row) => match row.kind {
                super::model::DiffRowKind::Equal => lines.push(UnifiedLine {
                    display_row_id: row.id.clone(),
                    logical_row: Some(row.logical_row),
                    side: UnifiedLineSide::Context,
                    reference_line: row.reference_line,
                    current_line: row.current_line,
                    text: row
                        .current_text
                        .as_ref()
                        .or(row.reference_text.as_ref())
                        .cloned()
                        .unwrap_or_default(),
                    inline_ranges: Vec::new(),
                }),
                super::model::DiffRowKind::ReferenceOnly => lines.push(UnifiedLine {
                    display_row_id: row.id.clone(),
                    logical_row: Some(row.logical_row),
                    side: UnifiedLineSide::Removal,
                    reference_line: row.reference_line,
                    current_line: None,
                    text: row.reference_text.clone().unwrap_or_default(),
                    inline_ranges: Vec::new(),
                }),
                super::model::DiffRowKind::CurrentOnly => lines.push(UnifiedLine {
                    display_row_id: row.id.clone(),
                    logical_row: Some(row.logical_row),
                    side: UnifiedLineSide::Addition,
                    reference_line: None,
                    current_line: row.current_line,
                    text: row.current_text.clone().unwrap_or_default(),
                    inline_ranges: Vec::new(),
                }),
                super::model::DiffRowKind::Modify => {
                    lines.push(UnifiedLine {
                        display_row_id: row.id.clone(),
                        logical_row: Some(row.logical_row),
                        side: UnifiedLineSide::Removal,
                        reference_line: row.reference_line,
                        current_line: None,
                        text: row.reference_text.clone().unwrap_or_default(),
                        inline_ranges: inline_ranges_for_side(
                            &row.inline_ranges,
                            DiffSide::Reference,
                        ),
                    });
                    lines.push(UnifiedLine {
                        display_row_id: row.id.clone(),
                        logical_row: Some(row.logical_row),
                        side: UnifiedLineSide::Addition,
                        reference_line: None,
                        current_line: row.current_line,
                        text: row.current_text.clone().unwrap_or_default(),
                        inline_ranges: inline_ranges_for_side(
                            &row.inline_ranges,
                            DiffSide::Current,
                        ),
                    });
                }
            },
            CompareDisplayRow::Collapsed(row) => lines.push(UnifiedLine {
                display_row_id: row.id.clone(),
                logical_row: None,
                side: UnifiedLineSide::Collapsed,
                reference_line: None,
                current_line: None,
                text: row.label.clone(),
                inline_ranges: Vec::new(),
            }),
            CompareDisplayRow::SkippedMarker(row) => lines.push(UnifiedLine {
                display_row_id: row.id.clone(),
                logical_row: None,
                side: UnifiedLineSide::Collapsed,
                reference_line: None,
                current_line: None,
                text: row.reason.clone(),
                inline_ranges: Vec::new(),
            }),
        }
    }
    UnifiedPresentation { lines }
}

fn inline_ranges_for_side(ranges: &[InlineRange], side: DiffSide) -> Vec<InlineRange> {
    ranges
        .iter()
        .filter(|range| range.side == side)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{UnifiedLineSide, build_unified_presentation};
    use crate::editor_tab::compare::diff::compute_diff;
    use crate::editor_tab::compare::display::{CompareDisplayOptions, build_display_model};

    fn unified(reference: &str, current: &str) -> super::UnifiedPresentation {
        let computation = compute_diff(reference, current);
        let reference_lines = reference.lines().collect::<Vec<_>>();
        let current_lines = current.lines().collect::<Vec<_>>();
        let display = build_display_model(
            None,
            &computation.model,
            &reference_lines,
            &current_lines,
            &CompareDisplayOptions::expanded(),
        );
        build_unified_presentation(&display)
    }

    #[test]
    fn insertions_and_deletions_use_one_flow() {
        let presentation = unified("same\nold\n", "same\nnew\nextra\n");
        let sides = presentation
            .lines
            .iter()
            .map(|line| line.side)
            .collect::<Vec<_>>();

        assert!(sides.contains(&UnifiedLineSide::Removal));
        assert!(sides.contains(&UnifiedLineSide::Addition));
        assert_eq!(presentation.text(), "same\nold\nnew\nextra");
    }

    #[test]
    fn modify_rows_preserve_both_line_numbers() {
        let presentation = unified("alpha\n", "alpine\n");

        let removal = presentation
            .lines
            .iter()
            .find(|line| line.side == UnifiedLineSide::Removal);
        let addition = presentation
            .lines
            .iter()
            .find(|line| line.side == UnifiedLineSide::Addition);

        assert_eq!(removal.and_then(|line| line.reference_line), Some(1));
        assert_eq!(removal.and_then(|line| line.current_line), None);
        assert_eq!(addition.and_then(|line| line.reference_line), None);
        assert_eq!(addition.and_then(|line| line.current_line), Some(1));
    }

    #[test]
    fn modify_rows_keep_side_specific_inline_ranges() {
        let presentation = unified("alpha beta\n", "alpha zeta\n");

        let removal_ranges = presentation
            .lines
            .iter()
            .find(|line| line.side == UnifiedLineSide::Removal)
            .map(|line| line.inline_ranges.len());
        let addition_ranges = presentation
            .lines
            .iter()
            .find(|line| line.side == UnifiedLineSide::Addition)
            .map(|line| line.inline_ranges.len());

        assert!(removal_ranges.is_some_and(|count| count > 0));
        assert!(addition_ranges.is_some_and(|count| count > 0));
    }

    #[test]
    fn empty_files_have_no_unified_rows() {
        let presentation = unified("", "");

        assert_eq!(presentation.line_count(), 0);
    }
}
