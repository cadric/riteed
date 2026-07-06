use std::collections::BTreeSet;

use gettextrs::ngettext;

use super::inline::InlineRange;
use super::model::{DiffRowKind, DiffRowModel};
use crate::editor_tab::ReviewFileId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompareDisplayOptions {
    pub(super) collapse_unchanged: bool,
    pub(super) context_lines: usize,
    pub(super) revealed_rows: BTreeSet<usize>,
}

impl CompareDisplayOptions {
    #[must_use]
    #[cfg(test)]
    pub(super) fn expanded() -> Self {
        Self {
            collapse_unchanged: false,
            context_lines: 3,
            revealed_rows: BTreeSet::new(),
        }
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn collapsed(context_lines: usize) -> Self {
        Self {
            collapse_unchanged: true,
            context_lines: context_lines.clamp(1, 10),
            revealed_rows: BTreeSet::new(),
        }
    }

    #[cfg(test)]
    pub(super) fn reveal_rows(&mut self, rows: impl IntoIterator<Item = usize>) {
        self.revealed_rows.extend(rows);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct DisplayRowId {
    pub(super) file_id: Option<ReviewFileId>,
    pub(super) kind: DisplayRowIdKind,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) enum DisplayRowIdKind {
    FileBoundary,
    Logical {
        row: usize,
    },
    Collapsed {
        hidden_start: usize,
        hidden_end: usize,
    },
    SkippedMarker,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompareDisplayModel {
    pub(super) rows: Vec<CompareDisplayRow>,
    logical_to_display: Vec<Option<usize>>,
}

impl CompareDisplayModel {
    #[must_use]
    pub(super) fn empty() -> Self {
        Self {
            rows: Vec::new(),
            logical_to_display: Vec::new(),
        }
    }

    #[must_use]
    pub(super) fn visible_row_count(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub(super) fn row_for_logical(&self, row: usize) -> Option<usize> {
        self.logical_to_display.get(row).and_then(|row| *row)
    }

    #[must_use]
    pub(super) fn logical_row_for_display(&self, row: usize) -> Option<usize> {
        self.rows.get(row).and_then(|row| match row {
            CompareDisplayRow::Content(row) => Some(row.logical_row),
            CompareDisplayRow::Collapsed(row) => Some(row.hidden_start),
            CompareDisplayRow::FileBoundary(_) | CompareDisplayRow::SkippedMarker(_) => None,
        })
    }

    #[must_use]
    #[cfg(test)]
    pub(super) fn current_text_projection(&self) -> String {
        self.rows
            .iter()
            .filter_map(|row| match row {
                CompareDisplayRow::Content(row) => row.current_text.as_deref(),
                CompareDisplayRow::FileBoundary(_)
                | CompareDisplayRow::Collapsed(_)
                | CompareDisplayRow::SkippedMarker(_) => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CompareDisplayRow {
    FileBoundary(DisplayFileBoundaryRow),
    Content(DisplayContentRow),
    Collapsed(DisplayCollapsedRow),
    SkippedMarker(DisplaySkippedMarkerRow),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DisplayFileBoundaryRow {
    pub(super) id: DisplayRowId,
    pub(super) path: String,
    pub(super) status_badge: &'static str,
    pub(super) additions: usize,
    pub(super) removals: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DisplayContentRow {
    pub(super) id: DisplayRowId,
    pub(super) logical_row: usize,
    pub(super) kind: DiffRowKind,
    pub(super) reference_line: Option<usize>,
    pub(super) current_line: Option<usize>,
    pub(super) reference_text: Option<String>,
    pub(super) current_text: Option<String>,
    pub(super) inline_ranges: Vec<InlineRange>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DisplayCollapsedRow {
    pub(super) id: DisplayRowId,
    pub(super) hidden_start: usize,
    pub(super) hidden_end: usize,
    pub(super) hidden_count: usize,
    pub(super) label: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DisplaySkippedMarkerRow {
    pub(super) id: DisplayRowId,
    pub(super) reason: String,
}

pub(super) fn build_display_model(
    file_id: Option<&ReviewFileId>,
    model: &DiffRowModel,
    reference_lines: &[&str],
    current_lines: &[&str],
    options: &CompareDisplayOptions,
) -> CompareDisplayModel {
    if model.too_large {
        return CompareDisplayModel {
            rows: Vec::new(),
            logical_to_display: Vec::new(),
        };
    }
    let visible = visible_rows(model, options);
    let mut rows = Vec::new();
    let mut logical_to_display = vec![None; model.rows.len()];
    let mut row_index = 0;
    while row_index < model.rows.len() {
        if visible[row_index] {
            let display_index = rows.len();
            logical_to_display[row_index] = Some(display_index);
            rows.push(CompareDisplayRow::Content(content_row(
                file_id,
                model,
                reference_lines,
                current_lines,
                row_index,
            )));
            row_index += 1;
            continue;
        }
        let hidden_start = row_index;
        while row_index < model.rows.len() && !visible[row_index] {
            row_index += 1;
        }
        rows.push(CompareDisplayRow::Collapsed(collapsed_row(
            file_id,
            hidden_start,
            row_index,
        )));
    }
    CompareDisplayModel {
        rows,
        logical_to_display,
    }
}

fn content_row(
    file_id: Option<&ReviewFileId>,
    model: &DiffRowModel,
    reference_lines: &[&str],
    current_lines: &[&str],
    logical_row: usize,
) -> DisplayContentRow {
    let row = &model.rows[logical_row];
    DisplayContentRow {
        id: DisplayRowId {
            file_id: file_id.cloned(),
            kind: DisplayRowIdKind::Logical { row: logical_row },
        },
        logical_row,
        kind: row.kind,
        reference_line: row.reference_line.map(|line| line + 1),
        current_line: row.current_line.map(|line| line + 1),
        reference_text: line_text(reference_lines, row.reference_line),
        current_text: line_text(current_lines, row.current_line),
        inline_ranges: row.inline_ranges.clone(),
    }
}

fn collapsed_row(
    file_id: Option<&ReviewFileId>,
    hidden_start: usize,
    hidden_end: usize,
) -> DisplayCollapsedRow {
    let hidden_count = hidden_end.saturating_sub(hidden_start);
    DisplayCollapsedRow {
        id: DisplayRowId {
            file_id: file_id.cloned(),
            kind: DisplayRowIdKind::Collapsed {
                hidden_start,
                hidden_end,
            },
        },
        hidden_start,
        hidden_end,
        hidden_count,
        label: hidden_label(hidden_count),
    }
}

fn visible_rows(model: &DiffRowModel, options: &CompareDisplayOptions) -> Vec<bool> {
    if !options.collapse_unchanged || model.hunks.is_empty() {
        return vec![true; model.rows.len()];
    }
    let mut visible = vec![false; model.rows.len()];
    for (row_index, row) in model.rows.iter().enumerate() {
        if row.kind == DiffRowKind::Equal {
            continue;
        }
        let start = row_index.saturating_sub(options.context_lines);
        let end = (row_index + options.context_lines + 1).min(model.rows.len());
        for slot in &mut visible[start..end] {
            *slot = true;
        }
    }
    for row in &options.revealed_rows {
        if let Some(slot) = visible.get_mut(*row) {
            *slot = true;
        }
    }
    visible
}

fn line_text(lines: &[&str], line: Option<usize>) -> Option<String> {
    let line = line?;
    let text = lines.get(line).map_or("", |value| strip_line_ending(value));
    Some(sanitize_display_line(text))
}

fn hidden_label(hidden_count: usize) -> String {
    let count = u32::try_from(hidden_count).map_or(u32::MAX, |value| value);
    ngettext(
        "%d unchanged line hidden",
        "%d unchanged lines hidden",
        count,
    )
    .replace("%d", &hidden_count.to_string())
}

fn strip_line_ending(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .or_else(|| line.strip_suffix('\r'))
        .unwrap_or(line)
}

// Presentation buffers must keep one display row per buffer line. GtkTextView
// treats \r and U+2029 as paragraph breaks, so stray separators inside a row
// are replaced with same-byte-width placeholders to keep inline byte ranges
// and row indices aligned.
fn sanitize_display_line(text: &str) -> String {
    if !text.contains(['\r', '\u{2028}', '\u{2029}']) {
        return text.to_string();
    }
    text.chars()
        .map(|ch| match ch {
            '\r' => ' ',
            '\u{2028}' | '\u{2029}' => '\u{FFFD}',
            _ => ch,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{CompareDisplayOptions, CompareDisplayRow, build_display_model, hidden_label};
    use crate::editor_tab::compare::diff::compute_diff;
    use crate::editor_tab::compare::model::DiffRowKind;
    use similar::DiffableStr;

    fn display(
        reference: &str,
        current: &str,
        options: &CompareDisplayOptions,
    ) -> super::CompareDisplayModel {
        let computation = compute_diff(reference, current);
        let reference_lines = reference.lines().collect::<Vec<_>>();
        let current_lines = current.lines().collect::<Vec<_>>();
        build_display_model(
            None,
            &computation.model,
            &reference_lines,
            &current_lines,
            options,
        )
    }

    #[test]
    fn tokenize_lines_matches_split_inclusive_for_newline_only_text() {
        for text in [
            "",
            "a",
            "a\n",
            "alpha\nbravo\ncharlie\n",
            "alpha\nbravo",
            "\n\n",
        ] {
            assert_eq!(
                text.split_inclusive('\n').collect::<Vec<_>>(),
                text.tokenize_lines(),
                "line slicing must be byte-for-byte identical for {text:?}"
            );
        }
    }

    #[test]
    fn lone_carriage_return_lines_render_the_changed_text() {
        let reference = "a\rb\nX\n";
        let current = "a\rb\nY\n";
        let computation = compute_diff(reference, current);
        let reference_lines = reference.tokenize_lines();
        let current_lines = current.tokenize_lines();
        let model = build_display_model(
            None,
            &computation.model,
            &reference_lines,
            &current_lines,
            &CompareDisplayOptions::expanded(),
        );
        let modify = model.rows.iter().find_map(|row| match row {
            CompareDisplayRow::Content(row) if row.kind == DiffRowKind::Modify => Some(row),
            CompareDisplayRow::Content(_)
            | CompareDisplayRow::FileBoundary(_)
            | CompareDisplayRow::Collapsed(_)
            | CompareDisplayRow::SkippedMarker(_) => None,
        });
        assert_eq!(
            modify.and_then(|row| row.reference_text.as_deref()),
            Some("X")
        );
        assert_eq!(
            modify.and_then(|row| row.current_text.as_deref()),
            Some("Y")
        );
    }

    #[test]
    fn display_row_texts_contain_no_paragraph_separator_characters() {
        let reference = "a\rmid\u{2029}tail\nX\n";
        let current = "a\rmid\u{2029}tail\nY\n";
        let computation = compute_diff(reference, current);
        let reference_lines = reference.tokenize_lines();
        let current_lines = current.tokenize_lines();
        let model = build_display_model(
            None,
            &computation.model,
            &reference_lines,
            &current_lines,
            &CompareDisplayOptions::expanded(),
        );
        for row in &model.rows {
            if let CompareDisplayRow::Content(row) = row {
                for text in [&row.reference_text, &row.current_text]
                    .into_iter()
                    .flatten()
                {
                    assert!(
                        !text.contains(['\r', '\u{2028}', '\u{2029}']),
                        "display text must stay one buffer line: {text:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn compare_display_slicing_shares_the_model_tokenizer() {
        let controller_src = include_str!("controller.rs");
        let review_src = include_str!("review_session.rs");
        assert!(
            !controller_src.contains("split_inclusive('\\n')"),
            "controller.rs must use compare::diff::line_slices"
        );
        assert!(
            !review_src.contains("split_inclusive('\\n')"),
            "review_session.rs must use compare::diff::line_slices"
        );
    }

    #[test]
    fn expanded_display_preserves_logical_rows_and_line_numbers() {
        let model = display("a\nold\n", "a\nnew\n", &CompareDisplayOptions::expanded());

        assert_eq!(model.visible_row_count(), 2);
        assert_eq!(model.row_for_logical(0), Some(0));
        assert_eq!(model.row_for_logical(1), Some(1));
        let second = model.rows.get(1).and_then(|row| match row {
            CompareDisplayRow::Content(row) => Some(row),
            CompareDisplayRow::FileBoundary(_)
            | CompareDisplayRow::Collapsed(_)
            | CompareDisplayRow::SkippedMarker(_) => None,
        });
        assert_eq!(second.and_then(|row| row.reference_line), Some(2));
        assert_eq!(second.and_then(|row| row.current_line), Some(2));
        assert_eq!(
            second.and_then(|row| row.reference_text.as_deref()),
            Some("old")
        );
        assert_eq!(
            second.and_then(|row| row.current_text.as_deref()),
            Some("new")
        );
    }

    #[test]
    fn collapse_hides_middle_equal_region_with_context() {
        let reference = "0\n1\n2\n3\n4\n5\n6\n";
        let current = "x\n1\n2\n3\n4\n5\ny\n";
        let model = display(reference, current, &CompareDisplayOptions::collapsed(1));

        assert!(
            model
                .rows
                .iter()
                .any(|row| matches!(row, CompareDisplayRow::Collapsed(_)))
        );
        assert_eq!(model.row_for_logical(0), Some(0));
        assert_eq!(model.row_for_logical(6), Some(4));
    }

    #[test]
    fn reveal_rows_keeps_requested_unchanged_rows_visible() {
        let mut options = CompareDisplayOptions::collapsed(1);
        options.reveal_rows([3]);

        let model = display("0\n1\n2\n3\n4\n5\n6\n", "x\n1\n2\n3\n4\n5\ny\n", &options);

        assert!(model.row_for_logical(3).is_some());
    }

    #[test]
    fn clipboard_projection_uses_current_content_only() {
        let model = display(
            "a\nold\nremove\n",
            "a\nnew\n",
            &CompareDisplayOptions::expanded(),
        );

        assert_eq!(model.current_text_projection(), "a\nnew");
    }

    #[test]
    fn hidden_label_uses_plural_forms() {
        assert_eq!(hidden_label(1), "1 unchanged line hidden");
        assert_eq!(hidden_label(2), "2 unchanged lines hidden");
    }
}
