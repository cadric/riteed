use std::rc::Rc;

use gtk4::gio;

use super::Window;

type CompareViewportPositionForTests = Option<(usize, f64)>;
type CompareViewportPositionsForTests = (
    CompareViewportPositionForTests,
    CompareViewportPositionForTests,
);

impl Window {
    pub(crate) fn compare_action_states_for_tests(&self) -> (bool, bool, bool, bool, bool) {
        self.compare.action_states_for_tests()
    }

    pub(crate) fn tab_compare_action_states_for_tests(&self) -> (bool, bool, bool) {
        self.compare.tab_compare_action_states_for_tests()
    }

    pub(crate) fn compare_with_disk_for_tests(self: &Rc<Self>) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.start_compare_with_disk(Rc::new(|_result| {}));
            self.workspace.refresh_selected_state();
        }
    }

    pub(crate) fn compare_with_file_for_tests(&self, file: &gio::File) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.start_compare_with_file(file, Rc::new(|_result| {}));
            self.workspace.refresh_selected_state();
        }
    }

    pub(crate) fn compare_two_files_for_tests(
        self: &Rc<Self>,
        left: &gio::File,
        right: &gio::File,
    ) {
        self.compare.compare_two_files_for_tests(left, right);
    }

    pub(crate) fn refresh_compare_reference_for_tests(self: &Rc<Self>) {
        self.compare.refresh_reference();
    }

    pub(crate) fn exit_compare_for_tests(&self) {
        self.compare.exit_compare();
    }

    pub(crate) fn next_diff_for_tests(&self) {
        self.compare.next_diff();
    }

    pub(crate) fn previous_diff_for_tests(&self) {
        self.compare.previous_diff();
    }

    pub(crate) fn selected_compare_active_for_tests(&self) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.is_compare_active())
    }

    pub(crate) fn selected_compare_diff_count_for_tests(&self) -> usize {
        self.workspace
            .selected_tab()
            .map_or(0, |tab| tab.compare_diff_count_for_tests())
    }

    pub(crate) fn selected_compare_status_for_tests(&self) -> String {
        self.workspace
            .selected_tab()
            .map_or_else(String::new, |tab| tab.compare_status_for_tests())
    }

    pub(crate) fn selected_compare_current_hunk_for_tests(&self) -> Option<usize> {
        self.workspace
            .selected_tab()
            .and_then(|tab| tab.compare_current_hunk_for_tests())
    }

    pub(crate) fn selected_compare_highlight_count_for_tests(&self) -> usize {
        self.workspace
            .selected_tab()
            .map_or(0, |tab| tab.compare_editable_highlight_count_for_tests())
    }

    pub(crate) fn selected_compare_row_count_for_tests(&self) -> usize {
        self.workspace
            .selected_tab()
            .map_or(0, |tab| tab.compare_row_count_for_tests())
    }

    pub(crate) fn selected_compare_placeholder_count_for_tests(&self) -> usize {
        self.workspace
            .selected_tab()
            .map_or(0, |tab| tab.compare_placeholder_count_for_tests())
    }

    pub(crate) fn selected_compare_inline_range_count_for_tests(&self) -> usize {
        self.workspace
            .selected_tab()
            .map_or(0, |tab| tab.compare_inline_range_count_for_tests())
    }

    pub(crate) fn selected_compare_line_numbers_for_tests(
        &self,
        row: usize,
    ) -> (Option<usize>, Option<usize>) {
        self.workspace
            .selected_tab()
            .map_or((None, None), |tab| tab.compare_line_numbers_for_tests(row))
    }

    pub(crate) fn selected_compare_views_editable_for_tests(&self) -> (bool, bool) {
        self.workspace
            .selected_tab()
            .map_or((true, true), |tab| tab.compare_views_editable_for_tests())
    }

    pub(crate) fn selected_compare_semantic_colors_for_tests(&self) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.compare_semantic_colors_for_tests())
    }

    pub(crate) fn selected_compare_line_counts_for_tests(&self) -> (i32, i32) {
        self.workspace
            .selected_tab()
            .map_or((0, 0), |tab| tab.compare_line_counts_for_tests())
    }

    pub(crate) fn selected_compare_wrap_modes_for_tests(
        &self,
    ) -> Option<(gtk4::WrapMode, gtk4::WrapMode)> {
        self.workspace
            .selected_tab()
            .map(|tab| tab.compare_wrap_modes_for_tests())
    }

    pub(crate) fn selected_compare_top_visible_rows_for_tests(&self) -> (usize, usize) {
        self.workspace
            .selected_tab()
            .map_or((0, 0), |tab| tab.compare_top_visible_rows_for_tests())
    }

    pub(crate) fn selected_compare_top_visible_positions_for_tests(
        &self,
    ) -> CompareViewportPositionsForTests {
        self.workspace.selected_tab().map_or((None, None), |tab| {
            tab.compare_top_visible_positions_for_tests()
        })
    }

    pub(crate) fn selected_compare_gutter_widths_for_tests(&self) -> (i32, i32) {
        self.workspace
            .selected_tab()
            .map_or((0, 0), |tab| tab.compare_gutter_widths_for_tests())
    }

    pub(crate) fn scroll_selected_compare_to_row_for_tests(&self, row: usize) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_scroll_to_row_for_tests(row);
        }
    }

    pub(crate) fn scroll_left_compare_to_row_offset_for_tests(
        &self,
        row: usize,
        offset: f64,
    ) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.compare_scroll_left_to_row_offset_for_tests(row, offset))
    }

    pub(crate) fn scroll_right_compare_to_row_offset_for_tests(
        &self,
        row: usize,
        offset: f64,
    ) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.compare_scroll_right_to_row_offset_for_tests(row, offset))
    }

    pub(crate) fn set_left_compare_scroll_value_for_tests(&self, value: f64) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_set_left_scroll_value_for_tests(value);
        }
    }

    pub(crate) fn left_compare_scroll_value_for_tests(&self) -> f64 {
        self.workspace
            .selected_tab()
            .map_or(0.0, |tab| tab.compare_left_scroll_value_for_tests())
    }

    pub(crate) fn compare_scroll_event_counts_for_tests(&self) -> (usize, usize) {
        self.workspace
            .selected_tab()
            .map_or((0, 0), |tab| tab.compare_scroll_event_counts_for_tests())
    }

    pub(crate) fn reset_compare_scroll_event_counts_for_tests(&self) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_reset_scroll_event_counts_for_tests();
        }
    }

    pub(crate) fn select_left_compare_range_for_tests(&self, start: i32, end: i32) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_select_left_for_tests(start, end);
        }
    }

    pub(crate) fn select_right_compare_range_for_tests(&self, start: i32, end: i32) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.compare_select_right_for_tests(start, end);
        }
    }

    pub(crate) fn copy_left_compare_selection_for_tests(&self) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.compare_copy_left_for_tests())
    }

    pub(crate) fn copy_right_compare_selection_for_tests(&self) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.compare_copy_right_for_tests())
    }
}
