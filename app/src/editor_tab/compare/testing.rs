use gtk4::prelude::*;
use sourceview5::prelude::BufferExt;

use super::interaction;
use super::navigation;
use super::render::{self, CompareTags};
use crate::editor_tab::EditorTab;

type CompareViewportPositionForTests = Option<(usize, f64)>;
type CompareViewportPositionsForTests = (
    CompareViewportPositionForTests,
    CompareViewportPositionForTests,
);

impl EditorTab {
    pub(crate) fn compare_diff_count_for_tests(&self) -> usize {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.row_model.borrow().hunks.len())
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_status_for_tests(&self) -> String {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.status_label.text().to_string())
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_current_hunk_for_tests(&self) -> Option<usize> {
        self.state.try_borrow().ok().and_then(|state| {
            state
                .compare
                .active
                .as_ref()
                .and_then(|compare| compare.current_hunk)
        })
    }

    pub(crate) fn compare_row_count_for_tests(&self) -> usize {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.row_model.borrow().rows.len())
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_placeholder_count_for_tests(&self) -> usize {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.presentation.borrow().placeholder_count)
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_inline_range_count_for_tests(&self) -> usize {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    compare
                        .row_model
                        .borrow()
                        .rows
                        .iter()
                        .map(|row| row.inline_ranges.len())
                        .sum()
                })
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_line_numbers_for_tests(
        &self,
        row: usize,
    ) -> (Option<usize>, Option<usize>) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    let presentation = compare.presentation.borrow();
                    (
                        presentation
                            .reference_line_numbers
                            .get(row)
                            .and_then(|line| *line),
                        presentation
                            .current_line_numbers
                            .get(row)
                            .and_then(|line| *line),
                    )
                })
            })
            .unwrap_or((None, None))
    }

    pub(crate) fn compare_views_editable_for_tests(&self) -> (bool, bool) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    (
                        compare.left_view.is_editable(),
                        compare.right_view.is_editable(),
                    )
                })
            })
            .unwrap_or((self.text_view.is_editable(), self.text_view.is_editable()))
    }

    pub(crate) fn compare_semantic_colors_for_tests(&self) -> bool {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    CompareTags::semantic_colors_available()
                        && !compare.left_buffer.is_highlight_syntax()
                        && !compare.right_buffer.is_highlight_syntax()
                })
            })
            .unwrap_or(false)
    }

    pub(crate) fn compare_line_counts_for_tests(&self) -> (i32, i32) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    (
                        compare.left_buffer.line_count(),
                        compare.right_buffer.line_count(),
                    )
                })
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_wrap_modes_for_tests(&self) -> (gtk4::WrapMode, gtk4::WrapMode) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    (
                        compare.left_view.wrap_mode(),
                        compare.right_view.wrap_mode(),
                    )
                })
            })
            .unwrap_or((self.text_view.wrap_mode(), self.text_view.wrap_mode()))
    }

    pub(crate) fn compare_editable_highlight_count_for_tests(&self) -> usize {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| render::highlight_count(&compare.left_buffer))
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_top_visible_rows_for_tests(&self) -> (usize, usize) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    let row_count = compare.row_model.borrow().rows.len();
                    (
                        navigation::top_visible_row(&compare.left_view, row_count),
                        navigation::top_visible_row(&compare.right_view, row_count),
                    )
                })
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_top_visible_positions_for_tests(
        &self,
    ) -> CompareViewportPositionsForTests {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    let row_count = compare.row_model.borrow().rows.len();
                    let (left, right) = compare.scroll_sync.viewport_positions_for_tests(row_count);
                    (
                        left.map(|position| (position.row, position.offset)),
                        right.map(|position| (position.row, position.offset)),
                    )
                })
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_gutter_widths_for_tests(&self) -> (i32, i32) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.gutters.width_requests())
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_scroll_to_row_for_tests(&self, row: usize) {
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        if let Some(compare) = state.compare.active.as_ref() {
            let _scrolled = compare.scroll_to_row(row);
        }
    }

    pub(crate) fn compare_scroll_left_to_row_offset_for_tests(
        &self,
        row: usize,
        offset: f64,
    ) -> bool {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    compare
                        .scroll_sync
                        .scroll_left_to_row_offset_for_tests(row, offset)
                })
            })
            .unwrap_or(false)
    }

    pub(crate) fn compare_scroll_right_to_row_offset_for_tests(
        &self,
        row: usize,
        offset: f64,
    ) -> bool {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    compare
                        .scroll_sync
                        .scroll_right_to_row_offset_for_tests(row, offset)
                })
            })
            .unwrap_or(false)
    }

    pub(crate) fn compare_set_left_scroll_value_for_tests(&self, value: f64) {
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            compare.scroll_sync.set_left_value_for_tests(value);
        }
    }

    pub(crate) fn compare_left_scroll_value_for_tests(&self) -> f64 {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.scroll_sync.left_value_for_tests())
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_scroll_event_counts_for_tests(&self) -> (usize, usize) {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state
                    .compare
                    .active
                    .as_ref()
                    .map(|compare| compare.scroll_sync.event_counts_for_tests())
            })
            .unwrap_or_default()
    }

    pub(crate) fn compare_reset_scroll_event_counts_for_tests(&self) {
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            compare.scroll_sync.reset_event_counts_for_tests();
        }
    }

    pub(crate) fn compare_select_left_for_tests(&self, start: i32, end: i32) {
        self.select_compare_range_for_tests(true, start, end);
    }

    pub(crate) fn compare_select_right_for_tests(&self, start: i32, end: i32) {
        self.select_compare_range_for_tests(false, start, end);
    }

    pub(crate) fn compare_copy_left_for_tests(&self) -> bool {
        self.copy_compare_selection_for_tests(true)
    }

    pub(crate) fn compare_copy_right_for_tests(&self) -> bool {
        self.copy_compare_selection_for_tests(false)
    }

    fn select_compare_range_for_tests(&self, left: bool, start: i32, end: i32) {
        let Ok(state) = self.state.try_borrow() else {
            return;
        };
        let Some(compare) = state.compare.active.as_ref() else {
            return;
        };
        let buffer = if left {
            &compare.left_buffer
        } else {
            &compare.right_buffer
        };
        let start = buffer.iter_at_offset(start);
        let end = buffer.iter_at_offset(end);
        buffer.select_range(&start, &end);
    }

    fn copy_compare_selection_for_tests(&self, left: bool) -> bool {
        self.state
            .try_borrow()
            .ok()
            .and_then(|state| {
                state.compare.active.as_ref().map(|compare| {
                    if left {
                        interaction::copy_selection_for_tests(
                            &compare.left_buffer,
                            &compare.left_view,
                        )
                    } else {
                        interaction::copy_selection_for_tests(
                            &compare.right_buffer,
                            &compare.right_view,
                        )
                    }
                })
            })
            .unwrap_or(false)
    }
}

pub(crate) fn row_count_for_texts_for_tests(current_text: &str, reference_text: &str) -> usize {
    super::diff::compute_diff_row_model(reference_text, current_text)
        .rows
        .len()
}
