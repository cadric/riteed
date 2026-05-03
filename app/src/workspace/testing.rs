use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use super::Workspace;

impl Workspace {
    pub(crate) fn tab_count(&self) -> i32 {
        self.tab_view.n_pages()
    }

    pub(crate) fn selected_title(&self) -> String {
        self.selected_tab()
            .map_or_else(|| pgettext("document title", "Untitled"), |tab| tab.title())
    }

    pub(crate) fn selected_text(&self) -> String {
        self.selected_tab()
            .map_or_else(String::new, |tab| tab.buffer_text())
    }

    pub(crate) fn selected_style_scheme_id(&self) -> Option<String> {
        self.selected_tab()
            .and_then(|tab| tab.text_buffer().style_scheme())
            .map(|scheme| scheme.id().to_string())
    }

    pub(crate) fn set_selected_text(&self, text: &str) {
        if let Some(tab) = self.selected_tab() {
            tab.set_text_for_tests(text);
            self.refresh_selected_state();
        }
    }

    pub(crate) fn recent_files(&self) -> Vec<String> {
        self.state.borrow().recent_files.clone()
    }

    pub(crate) fn session_files(&self) -> Vec<String> {
        self.state.borrow().stored_session_files.clone()
    }

    pub(crate) fn selected_saved_uri(&self) -> String {
        self.selected_tab()
            .and_then(|tab| tab.uri())
            .unwrap_or_default()
    }

    pub(crate) fn text_for_uri(&self, uri: &str) -> Option<String> {
        self.find_tab_by_uri(uri).map(|tab| tab.buffer_text())
    }

    pub(crate) fn reorder_selected_to_first(&self) -> bool {
        self.tab_view
            .selected_page()
            .as_ref()
            .is_some_and(|page| self.tab_view.reorder_first(page))
    }

    pub(crate) fn shortcuts_enabled(&self) -> bool {
        self.tab_view.shortcuts() == adw::TabViewShortcuts::ALL_SHORTCUTS
    }

    pub(crate) fn tab_bar_controls_tab_view(&self) -> bool {
        self.tab_bar
            .view()
            .as_ref()
            .is_some_and(|view| view == &self.tab_view)
    }

    pub(crate) fn top_bar_order_matches(&self, header_bar: &adw::HeaderBar) -> bool {
        let header_widget = header_bar.clone().upcast::<gtk4::Widget>();
        let tab_widget = self.tab_bar.clone().upcast::<gtk4::Widget>();
        let search_widget = self.search.widget().clone().upcast::<gtk4::Widget>();
        let Some(parent) = header_widget.parent() else {
            return false;
        };
        if tab_widget.parent().as_ref() != Some(&parent)
            || search_widget.parent().as_ref() != Some(&parent)
        {
            return false;
        }
        header_widget.next_sibling().as_ref() == Some(&tab_widget)
            && tab_widget.next_sibling().as_ref() == Some(&search_widget)
    }

    pub(crate) fn search_visible(&self) -> bool {
        self.search.is_search_mode_for_tests()
    }

    pub(crate) fn replace_visible(&self) -> bool {
        self.search.is_replace_visible_for_tests()
    }

    pub(crate) fn search_query(&self) -> String {
        self.search.query_text_for_tests()
    }

    pub(crate) fn search_result(&self) -> String {
        self.search.result_text_for_tests()
    }

    pub(crate) fn status_labels(&self) -> (String, String, String) {
        self.status_bar.labels_for_tests()
    }

    pub(crate) fn status_format_summary(&self) -> String {
        self.status_bar.format_summary_for_tests()
    }

    pub(crate) fn status_zoom_percent(&self) -> String {
        self.status_bar.zoom_percent_for_tests()
    }

    pub(crate) fn activate_status_zoom_in(&self) {
        self.status_bar.activate_zoom_in_for_tests();
    }

    pub(crate) fn activate_status_zoom_out(&self) {
        self.status_bar.activate_zoom_out_for_tests();
    }

    pub(crate) fn activate_status_zoom_reset(&self) {
        self.status_bar.activate_zoom_reset_for_tests();
    }

    pub(crate) fn selected_line_numbers_visible(&self) -> bool {
        self.selected_tab()
            .is_some_and(|tab| tab.shows_line_numbers_for_tests())
    }

    pub(crate) fn selected_minimap_visible(&self) -> bool {
        self.selected_tab()
            .is_some_and(|tab| tab.minimap_visible_for_tests())
    }

    pub(crate) fn selected_indentation_for_tests(&self) -> Option<(bool, u32, i32)> {
        self.selected_tab().map(|tab| tab.indentation_for_tests())
    }

    pub(crate) fn indent_selected_lines_for_tests(&self) {
        if let Some(tab) = self.selected_tab() {
            tab.indent_selected_lines_for_tests();
            self.refresh_selected_state();
        }
    }

    pub(crate) fn unindent_selected_lines_for_tests(&self) {
        if let Some(tab) = self.selected_tab() {
            tab.unindent_selected_lines_for_tests();
            self.refresh_selected_state();
        }
    }

    pub(crate) fn selected_visual_column_at_offset_for_tests(&self, offset: i32) -> Option<u32> {
        self.selected_tab()
            .map(|tab| tab.visual_column_at_offset_for_tests(offset))
    }

    pub(crate) fn selected_minimap_font_for_tests(&self) -> Option<gtk4::pango::FontDescription> {
        self.selected_tab()
            .and_then(|tab| tab.minimap_font_desc_for_tests())
    }

    pub(crate) fn selected_zoom_class_for_tests(&self) -> bool {
        self.selected_tab()
            .is_some_and(|tab| tab.view_has_zoom_class_for_tests())
    }

    pub(crate) fn selected_zoom_css_classes_for_tests(&self) -> Vec<String> {
        self.selected_tab()
            .map_or_else(Vec::new, |tab| tab.zoom_css_classes_for_tests())
    }

    pub(crate) fn selected_scroll_past_end_padding_for_tests(&self) -> Option<(i32, i32)> {
        self.selected_tab()
            .map(|tab| tab.scroll_past_end_padding_for_tests())
    }

    pub(crate) fn selected_language_id(&self) -> Option<String> {
        self.selected_tab().and_then(|tab| tab.language_id())
    }

    pub(crate) fn selected_banner_visible(&self) -> bool {
        self.selected_tab()
            .is_some_and(|tab| tab.banner_visible_for_tests())
    }

    pub(crate) fn selected_writability(&self) -> Option<crate::editor_tab::Writability> {
        self.selected_tab().map(|tab| tab.writability())
    }

    pub(crate) fn sync_selected_banner_for_tests(&self, window_active: bool) {
        if let Some(tab) = self.selected_tab() {
            tab.sync_banner_for_tests(true, window_active);
        }
    }

    pub(crate) fn resolve_selected_external_for_tests(&self) {
        if let Some(tab) = self.selected_tab() {
            tab.resolve_pending_external();
        }
    }

    pub(crate) fn trigger_selected_external_action_for_tests(&self) {
        if let Some(tab) = self.selected_tab() {
            tab.trigger_external_action_for_tests();
        }
    }

    pub(crate) fn request_selected_autosave_for_tests(self: &Rc<Self>) {
        if let Some(tab) = self.selected_tab() {
            self.request_save_tab_kind(
                &tab,
                false,
                crate::editor_tab::SaveKind::Autosave,
                Rc::new(|_| {}),
            );
        }
    }

    pub(crate) fn request_selected_guarded_autosave_for_tests(
        self: &Rc<Self>,
    ) -> super::autosave::AutosaveRequestForTests {
        if let Some(tab) = self.selected_tab() {
            return super::autosave::request_tab_autosave_for_tests(self, &tab);
        }
        super::autosave::AutosaveRequestForTests {
            requested: false,
            result: Rc::new(std::cell::RefCell::new(None)),
        }
    }

    pub(crate) fn select_offsets_in_selected(&self, start: i32, end: i32) {
        if let Some(tab) = self.selected_tab() {
            tab.select_offsets_for_tests(start, end);
            self.refresh_selected_state();
        }
    }

    pub(crate) fn undo_selected(&self) {
        if let Some(tab) = self.selected_tab() {
            tab.undo_for_tests();
            self.refresh_selected_state();
        }
    }

    pub(crate) fn replace_current_for_tests(self: &Rc<Self>) {
        self.search.replace_current();
    }

    pub(crate) fn replace_all_for_tests(self: &Rc<Self>) {
        self.search.replace_all();
    }

    pub(crate) fn set_replace_text_for_tests(&self, text: &str) {
        self.search.set_replace_text_for_tests(text);
    }

    pub(crate) fn inject_external_event_for_tests(
        self: &Rc<Self>,
        uri: &str,
        event: crate::editor_monitor::ExternalFileEvent,
    ) {
        if let Some(tab) = self.find_tab_by_uri(uri) {
            tab.inject_external_event_for_tests(event);
        }
    }
}
