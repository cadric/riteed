use super::display::{CompareDisplayModel, CompareDisplayRow, DisplayContentRow};
use super::model::DiffRowKind;
use super::presentation::{DiffPresentation, PlaceholderMarker, PresentationSide, marker_text};

pub(super) fn build_presentation_from_display(display: &CompareDisplayModel) -> DiffPresentation {
    if display.rows.is_empty() {
        return DiffPresentation::empty();
    }
    let mut rows = PresentationRows::new(display.rows.len());
    let mut row_index = 0;

    while row_index < display.rows.len() {
        match &display.rows[row_index] {
            CompareDisplayRow::Content(row) => {
                row_index += rows.push_content(display, row_index, row);
            }
            CompareDisplayRow::Collapsed(row) => {
                rows.push_metadata(&row.label);
                row_index += 1;
            }
            CompareDisplayRow::FileBoundary(row) => {
                rows.push_metadata(&row.path);
                row_index += 1;
            }
            CompareDisplayRow::SkippedMarker(row) => {
                rows.push_metadata(&row.reason);
                row_index += 1;
            }
        }
    }

    rows.finish()
}

struct PresentationRows {
    reference_rows: Vec<String>,
    current_rows: Vec<String>,
    reference_numbers: Vec<Option<usize>>,
    current_numbers: Vec<Option<usize>>,
    placeholder_markers: Vec<Option<PlaceholderMarker>>,
    metadata_rows: Vec<bool>,
    placeholder_count: usize,
}

impl PresentationRows {
    fn new(capacity: usize) -> Self {
        Self {
            reference_rows: Vec::with_capacity(capacity),
            current_rows: Vec::with_capacity(capacity),
            reference_numbers: Vec::with_capacity(capacity),
            current_numbers: Vec::with_capacity(capacity),
            placeholder_markers: Vec::with_capacity(capacity),
            metadata_rows: Vec::with_capacity(capacity),
            placeholder_count: 0,
        }
    }

    fn finish(self) -> DiffPresentation {
        DiffPresentation::from_parts(
            self.reference_rows.join("\n"),
            self.current_rows.join("\n"),
            self.reference_numbers,
            self.current_numbers,
            self.placeholder_markers,
            self.metadata_rows,
            self.placeholder_count,
        )
    }

    fn push_content(
        &mut self,
        display: &CompareDisplayModel,
        row_index: usize,
        row: &DisplayContentRow,
    ) -> usize {
        match row.kind {
            DiffRowKind::Equal | DiffRowKind::Modify => {
                self.push_paired_content(row);
                1
            }
            DiffRowKind::ReferenceOnly => {
                let run_len =
                    same_display_kind_run_len(display, row_index, DiffRowKind::ReferenceOnly);
                self.push_reference_only_run(display, row_index, run_len);
                run_len
            }
            DiffRowKind::CurrentOnly => {
                let run_len =
                    same_display_kind_run_len(display, row_index, DiffRowKind::CurrentOnly);
                self.push_current_only_run(display, row_index, run_len);
                run_len
            }
        }
    }

    fn push_paired_content(&mut self, row: &DisplayContentRow) {
        push_display_line(
            &mut self.reference_rows,
            &mut self.reference_numbers,
            row.reference_text.as_deref(),
            row.reference_line,
            &mut self.placeholder_count,
        );
        push_display_line(
            &mut self.current_rows,
            &mut self.current_numbers,
            row.current_text.as_deref(),
            row.current_line,
            &mut self.placeholder_count,
        );
        self.push_row_metadata(None, false);
    }

    fn push_reference_only_run(
        &mut self,
        display: &CompareDisplayModel,
        start: usize,
        run_len: usize,
    ) {
        for offset in 0..run_len {
            let row = display_content_row(display, start + offset);
            push_display_line(
                &mut self.reference_rows,
                &mut self.reference_numbers,
                row.and_then(|row| row.reference_text.as_deref()),
                row.and_then(|row| row.reference_line),
                &mut self.placeholder_count,
            );
            let marker = (offset == 0).then_some(PlaceholderMarker {
                side: PresentationSide::Current,
                run_len,
            });
            push_placeholder(
                &mut self.current_rows,
                &mut self.current_numbers,
                &mut self.placeholder_count,
                marker.map(marker_text).as_deref(),
            );
            self.push_row_metadata(marker, false);
        }
    }

    fn push_current_only_run(
        &mut self,
        display: &CompareDisplayModel,
        start: usize,
        run_len: usize,
    ) {
        for offset in 0..run_len {
            let row = display_content_row(display, start + offset);
            let marker = (offset == 0).then_some(PlaceholderMarker {
                side: PresentationSide::Reference,
                run_len,
            });
            push_placeholder(
                &mut self.reference_rows,
                &mut self.reference_numbers,
                &mut self.placeholder_count,
                marker.map(marker_text).as_deref(),
            );
            push_display_line(
                &mut self.current_rows,
                &mut self.current_numbers,
                row.and_then(|row| row.current_text.as_deref()),
                row.and_then(|row| row.current_line),
                &mut self.placeholder_count,
            );
            self.push_row_metadata(marker, false);
        }
    }

    fn push_metadata(&mut self, label: &str) {
        self.reference_rows.push(label.to_string());
        self.current_rows.push(label.to_string());
        self.reference_numbers.push(None);
        self.current_numbers.push(None);
        self.push_row_metadata(None, true);
    }

    fn push_row_metadata(&mut self, marker: Option<PlaceholderMarker>, metadata: bool) {
        self.placeholder_markers.push(marker);
        self.metadata_rows.push(metadata);
    }
}

fn push_display_line(
    rows: &mut Vec<String>,
    numbers: &mut Vec<Option<usize>>,
    text: Option<&str>,
    line: Option<usize>,
    placeholder_count: &mut usize,
) {
    let Some(line) = line else {
        push_placeholder(rows, numbers, placeholder_count, None);
        return;
    };
    rows.push(text.unwrap_or_default().to_string());
    numbers.push(Some(line));
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

fn same_display_kind_run_len(
    display: &CompareDisplayModel,
    start: usize,
    kind: DiffRowKind,
) -> usize {
    display.rows[start..]
        .iter()
        .take_while(|row| match row {
            CompareDisplayRow::Content(row) => row.kind == kind,
            CompareDisplayRow::FileBoundary(_)
            | CompareDisplayRow::Collapsed(_)
            | CompareDisplayRow::SkippedMarker(_) => false,
        })
        .count()
}

fn display_content_row(display: &CompareDisplayModel, row: usize) -> Option<&DisplayContentRow> {
    match display.rows.get(row) {
        Some(CompareDisplayRow::Content(row)) => Some(row),
        Some(
            CompareDisplayRow::FileBoundary(_)
            | CompareDisplayRow::Collapsed(_)
            | CompareDisplayRow::SkippedMarker(_),
        )
        | None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::build_presentation_from_display;
    use crate::editor_tab::compare::diff::compute_diff;
    use crate::editor_tab::compare::display::{CompareDisplayOptions, build_display_model};
    use crate::editor_tab::compare::presentation::PresentationSide;

    #[test]
    fn collapsed_display_rows_become_split_metadata_rows() {
        let reference = "0\n1\n2\n3\n4\n5\n6\n";
        let current = "x\n1\n2\n3\n4\n5\ny\n";
        let computation = compute_diff(reference, current);
        let reference_lines = reference.lines().collect::<Vec<_>>();
        let current_lines = current.lines().collect::<Vec<_>>();
        let display = build_display_model(
            None,
            &computation.model,
            &reference_lines,
            &current_lines,
            &CompareDisplayOptions::collapsed(1),
        );
        let presentation = build_presentation_from_display(&display);

        let metadata_row = (0..presentation.line_count()).find(|row| {
            presentation.is_metadata_row(*row)
                && presentation
                    .reference_text
                    .lines()
                    .nth(*row)
                    .is_some_and(|line| line.contains("unchanged lines hidden"))
        });

        assert!(metadata_row.is_some());
        assert!(metadata_row.is_some_and(|row| {
            presentation
                .line_number(PresentationSide::Reference, row)
                .is_none()
                && presentation
                    .line_number(PresentationSide::Current, row)
                    .is_none()
        }));
        assert!(presentation.line_count() < computation.model.rows.len());
    }
}
