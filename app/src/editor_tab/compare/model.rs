use std::ops::Range;

use super::inline::{InlineBudget, InlineRange, ranges_for_modify};

const HUNK_CONTEXT_ROWS: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffSide {
    Reference,
    Current,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffRowKind {
    Equal,
    ReferenceOnly,
    CurrentOnly,
    Modify,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DiffRow {
    pub(super) reference_line: Option<usize>,
    pub(super) current_line: Option<usize>,
    pub(super) kind: DiffRowKind,
    pub(super) inline_ranges: Vec<InlineRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DiffHunk {
    pub(super) first_row: usize,
    pub(super) rows: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DiffRowModel {
    pub(super) too_large: bool,
    pub(super) rows: Vec<DiffRow>,
    pub(super) row_to_reference_line: Vec<Option<usize>>,
    pub(super) row_to_current_line: Vec<Option<usize>>,
    pub(super) reference_line_to_row: Vec<Option<usize>>,
    pub(super) current_line_to_row: Vec<Option<usize>>,
    pub(super) hunks: Vec<DiffHunk>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DiffLineTag {
    Equal,
    Delete,
    Insert,
    Replace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DiffLineOp {
    pub(super) tag: DiffLineTag,
    pub(super) reference_range: Range<usize>,
    pub(super) current_range: Range<usize>,
}

impl DiffRow {
    #[must_use]
    pub(super) fn new(
        reference_line: Option<usize>,
        current_line: Option<usize>,
        kind: DiffRowKind,
    ) -> Self {
        Self {
            reference_line,
            current_line,
            kind,
            inline_ranges: Vec::new(),
        }
    }
}

impl DiffRowModel {
    #[must_use]
    pub(super) fn empty() -> Self {
        Self {
            too_large: false,
            rows: Vec::new(),
            row_to_reference_line: Vec::new(),
            row_to_current_line: Vec::new(),
            reference_line_to_row: Vec::new(),
            current_line_to_row: Vec::new(),
            hunks: Vec::new(),
        }
    }

    #[must_use]
    pub(super) fn too_large() -> Self {
        Self {
            too_large: true,
            ..Self::empty()
        }
    }

    #[must_use]
    pub(super) fn changed_row_count(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.kind != DiffRowKind::Equal)
            .count()
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn row_for_line(&self, side: DiffSide, line: usize) -> Option<usize> {
        let map = match side {
            DiffSide::Reference => &self.reference_line_to_row,
            DiffSide::Current => &self.current_line_to_row,
        };
        map.get(line).and_then(|row| *row)
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn line_for_row(&self, side: DiffSide, row: usize) -> Option<usize> {
        let map = match side {
            DiffSide::Reference => &self.row_to_reference_line,
            DiffSide::Current => &self.row_to_current_line,
        };
        map.get(row).and_then(|line| *line)
    }
}

pub(super) fn build_row_model(
    ops: &[DiffLineOp],
    reference: &[&str],
    current: &[&str],
) -> DiffRowModel {
    let mut rows = Vec::new();
    let inline_budget = InlineBudget::new();
    for op in ops {
        match op.tag {
            DiffLineTag::Equal => {
                for (reference_line, current_line) in
                    op.reference_range.clone().zip(op.current_range.clone())
                {
                    rows.push(DiffRow::new(
                        Some(reference_line),
                        Some(current_line),
                        DiffRowKind::Equal,
                    ));
                }
            }
            DiffLineTag::Delete => {
                for reference_line in op.reference_range.clone() {
                    rows.push(DiffRow::new(
                        Some(reference_line),
                        None,
                        DiffRowKind::ReferenceOnly,
                    ));
                }
            }
            DiffLineTag::Insert => {
                for current_line in op.current_range.clone() {
                    rows.push(DiffRow::new(
                        None,
                        Some(current_line),
                        DiffRowKind::CurrentOnly,
                    ));
                }
            }
            DiffLineTag::Replace => push_replace_rows(
                &mut rows,
                &op.reference_range.clone().collect::<Vec<_>>(),
                &op.current_range.clone().collect::<Vec<_>>(),
                reference,
                current,
                &inline_budget,
            ),
        }
    }
    finish_model(rows, reference.len(), current.len())
}

fn push_replace_rows(
    rows: &mut Vec<DiffRow>,
    reference_lines: &[usize],
    current_lines: &[usize],
    reference: &[&str],
    current: &[&str],
    inline_budget: &InlineBudget,
) {
    let paired = reference_lines.len().min(current_lines.len());
    for index in 0..paired {
        let reference_line = reference_lines[index];
        let current_line = current_lines[index];
        let mut row = DiffRow::new(
            Some(reference_line),
            Some(current_line),
            DiffRowKind::Modify,
        );
        row.inline_ranges = ranges_for_modify(
            reference[reference_line],
            current[current_line],
            inline_budget,
        );
        rows.push(row);
    }
    for reference_line in reference_lines.iter().skip(paired) {
        rows.push(DiffRow::new(
            Some(*reference_line),
            None,
            DiffRowKind::ReferenceOnly,
        ));
    }
    for current_line in current_lines.iter().skip(paired) {
        rows.push(DiffRow::new(
            None,
            Some(*current_line),
            DiffRowKind::CurrentOnly,
        ));
    }
}

fn finish_model(rows: Vec<DiffRow>, reference_lines: usize, current_lines: usize) -> DiffRowModel {
    let mut model = DiffRowModel {
        row_to_reference_line: rows.iter().map(|row| row.reference_line).collect(),
        row_to_current_line: rows.iter().map(|row| row.current_line).collect(),
        reference_line_to_row: vec![None; reference_lines],
        current_line_to_row: vec![None; current_lines],
        hunks: hunks_for_rows(&rows),
        rows,
        too_large: false,
    };
    for (row_index, line) in model.row_to_reference_line.iter().enumerate() {
        if let Some(line) = line
            && let Some(slot) = model.reference_line_to_row.get_mut(*line)
        {
            *slot = Some(row_index);
        }
    }
    for (row_index, line) in model.row_to_current_line.iter().enumerate() {
        if let Some(line) = line
            && let Some(slot) = model.current_line_to_row.get_mut(*line)
        {
            *slot = Some(row_index);
        }
    }
    model
}

fn hunks_for_rows(rows: &[DiffRow]) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current = Vec::new();
    let mut equal_gap = 0;
    for (index, row) in rows.iter().enumerate() {
        if row.kind == DiffRowKind::Equal {
            if current.is_empty() {
                continue;
            }
            equal_gap += 1;
            if equal_gap > HUNK_CONTEXT_ROWS {
                push_hunk(&mut hunks, &current);
                current.clear();
                equal_gap = 0;
            }
            continue;
        }
        equal_gap = 0;
        current.push(index);
    }
    push_hunk(&mut hunks, &current);
    hunks
}

fn push_hunk(hunks: &mut Vec<DiffHunk>, rows: &[usize]) {
    let Some(first_row) = rows.first() else {
        return;
    };
    hunks.push(DiffHunk {
        first_row: *first_row,
        rows: rows.to_vec(),
    });
}

#[cfg(test)]
mod tests {
    use super::{DiffRowKind, DiffSide};
    use crate::editor_tab::compare::diff::compute_diff_row_model;

    #[test]
    fn insertion_only_builds_current_only_rows() {
        let model = compute_diff_row_model("", "a\nb\n");
        assert_eq!(model.rows.len(), 2);
        assert!(
            model
                .rows
                .iter()
                .all(|row| row.kind == DiffRowKind::CurrentOnly)
        );
        assert_eq!(model.row_to_reference_line, vec![None, None]);
        assert_eq!(model.row_to_current_line, vec![Some(0), Some(1)]);
    }

    #[test]
    fn deletion_only_builds_reference_only_rows() {
        let model = compute_diff_row_model("a\nb\n", "");
        assert_eq!(model.rows.len(), 2);
        assert!(
            model
                .rows
                .iter()
                .all(|row| row.kind == DiffRowKind::ReferenceOnly)
        );
        assert_eq!(model.row_to_reference_line, vec![Some(0), Some(1)]);
        assert_eq!(model.row_to_current_line, vec![None, None]);
    }

    #[test]
    fn replacement_pairs_modify_and_surplus_rows() {
        let model = compute_diff_row_model("a\nb\nc\n", "x\ny\nz\nq\n");
        assert_eq!(model.rows[0].kind, DiffRowKind::Modify);
        assert_eq!(model.rows[1].kind, DiffRowKind::Modify);
        assert_eq!(model.rows[2].kind, DiffRowKind::Modify);
        assert_eq!(model.rows[3].kind, DiffRowKind::CurrentOnly);
    }

    #[test]
    fn dense_line_maps_roundtrip() {
        let model = compute_diff_row_model("a\nb\nc\n", "a\nx\nc\n");
        assert_eq!(model.row_for_line(DiffSide::Reference, 1), Some(1));
        assert_eq!(model.row_for_line(DiffSide::Current, 1), Some(1));
        assert_eq!(model.line_for_row(DiffSide::Reference, 1), Some(1));
        assert_eq!(model.line_for_row(DiffSide::Current, 1), Some(1));
    }

    #[test]
    fn nearby_changes_merge_with_three_context_rows() {
        let model = compute_diff_row_model("0\n1\n2\n3\n4\n5\n", "x\n1\n2\n3\ny\n5\n");
        assert_eq!(model.hunks.len(), 1);
        assert_eq!(model.hunks[0].first_row, 0);
        assert_eq!(model.hunks[0].rows, vec![0, 4]);
    }

    #[test]
    fn eof_insert_preserves_prefix_maps() {
        let model = compute_diff_row_model("a\nb\n", "a\nb\nc\n");
        assert_eq!(model.rows[2].kind, DiffRowKind::CurrentOnly);
        assert_eq!(model.row_to_reference_line, vec![Some(0), Some(1), None]);
        assert_eq!(model.row_to_current_line, vec![Some(0), Some(1), Some(2)]);
        assert_eq!(model.row_for_line(DiffSide::Current, 2), Some(2));
        assert_eq!(model.line_for_row(DiffSide::Reference, 2), None);
    }

    #[test]
    fn eof_delete_preserves_prefix_maps() {
        let model = compute_diff_row_model("a\nb\nc\n", "a\nb\n");
        assert_eq!(model.rows[2].kind, DiffRowKind::ReferenceOnly);
        assert_eq!(model.row_to_reference_line, vec![Some(0), Some(1), Some(2)]);
        assert_eq!(model.row_to_current_line, vec![Some(0), Some(1), None]);
        assert_eq!(model.row_for_line(DiffSide::Reference, 2), Some(2));
        assert_eq!(model.line_for_row(DiffSide::Current, 2), None);
    }

    #[test]
    fn eof_replace_preserves_prefix_maps() {
        let model = compute_diff_row_model("a\nb\n", "a\nc\n");
        assert_eq!(model.rows[1].kind, DiffRowKind::Modify);
        assert_eq!(model.row_to_reference_line, vec![Some(0), Some(1)]);
        assert_eq!(model.row_to_current_line, vec![Some(0), Some(1)]);
        assert_eq!(model.row_for_line(DiffSide::Reference, 1), Some(1));
        assert_eq!(model.line_for_row(DiffSide::Current, 1), Some(1));
    }

    #[test]
    fn trailing_newline_boundaries_keep_last_line_maps() {
        let missing_newline = compute_diff_row_model("a\nb\n", "a\nb");
        assert_eq!(missing_newline.rows.len(), 2);
        assert_eq!(missing_newline.rows[1].kind, DiffRowKind::Modify);
        assert_eq!(
            missing_newline.row_to_reference_line,
            vec![Some(0), Some(1)]
        );
        assert_eq!(missing_newline.row_to_current_line, vec![Some(0), Some(1)]);

        let added_newline = compute_diff_row_model("a\nb", "a\nb\n");
        assert_eq!(added_newline.rows.len(), 2);
        assert_eq!(added_newline.rows[1].kind, DiffRowKind::Modify);
        assert_eq!(added_newline.row_to_reference_line, vec![Some(0), Some(1)]);
        assert_eq!(added_newline.row_to_current_line, vec![Some(0), Some(1)]);
    }
}
