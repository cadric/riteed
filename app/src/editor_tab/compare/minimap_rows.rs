use super::diff::{DiffOptions, DiffSkipReason, compute_diff_with_options};
use super::model::DiffRowKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::editor_tab) enum MinimapRowKind {
    Equal,
    Removed,
    Added,
    Modified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::editor_tab) struct MinimapRow {
    pub(in crate::editor_tab) reference_line: Option<usize>,
    pub(in crate::editor_tab) current_line: Option<usize>,
    pub(in crate::editor_tab) kind: MinimapRowKind,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::editor_tab) struct MinimapRows {
    pub(in crate::editor_tab) rows: Vec<MinimapRow>,
    pub(in crate::editor_tab) skip_reason: Option<DiffSkipReason>,
}

pub(in crate::editor_tab) fn compute(reference_text: &str, current_text: &str) -> MinimapRows {
    let computation =
        compute_diff_with_options(reference_text, current_text, DiffOptions::default());
    if let Some(skip_reason) = computation.skip_reason {
        return MinimapRows {
            rows: Vec::new(),
            skip_reason: Some(skip_reason),
        };
    }
    let rows = computation
        .model
        .rows
        .iter()
        .map(|row| MinimapRow {
            reference_line: row.reference_line,
            current_line: row.current_line,
            kind: match row.kind {
                DiffRowKind::Equal => MinimapRowKind::Equal,
                DiffRowKind::ReferenceOnly => MinimapRowKind::Removed,
                DiffRowKind::CurrentOnly => MinimapRowKind::Added,
                DiffRowKind::Modify => MinimapRowKind::Modified,
            },
        })
        .collect();
    MinimapRows {
        rows,
        skip_reason: None,
    }
}
