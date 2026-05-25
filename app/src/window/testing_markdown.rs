use super::Window;

impl Window {
    pub(crate) fn selected_markdown_preview_zoom_css_classes_for_tests(&self) -> Vec<String> {
        self.workspace.selected_tab().map_or_else(Vec::new, |tab| {
            tab.markdown_preview_zoom_css_classes_for_tests()
        })
    }

    pub(crate) fn selected_markdown_preview_base_css_class_for_tests(&self) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.markdown_preview_has_base_css_class_for_tests())
    }

    pub(crate) fn select_markdown_preview_offsets_for_tests(&self, start: i32, end: i32) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.select_markdown_preview_offsets_for_tests(start, end);
        }
    }

    pub(crate) fn selected_markdown_preview_scroll_value_for_tests(&self) -> f64 {
        self.workspace
            .selected_tab()
            .map_or(0.0, |tab| tab.markdown_preview_scroll_value_for_tests())
    }

    pub(crate) fn set_selected_markdown_preview_scroll_value_for_tests(&self, value: f64) {
        if let Some(tab) = self.workspace.selected_tab() {
            tab.set_markdown_preview_scroll_value_for_tests(value);
        }
    }

    pub(crate) fn copy_markdown_preview_selection_for_tests(&self) -> bool {
        self.workspace
            .selected_tab()
            .is_some_and(|tab| tab.copy_markdown_preview_selection_for_tests())
    }
}
