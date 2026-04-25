use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};
use libadwaita::prelude::*;

use super::Window;

impl Window {
    pub(crate) fn tab_count_for_tests(&self) -> i32 {
        self.workspace.tab_count()
    }

    pub(crate) fn selected_title_for_tests(&self) -> String {
        self.workspace.selected_title()
    }

    pub(crate) fn selected_text_for_tests(&self) -> String {
        self.workspace.selected_text()
    }

    pub(crate) fn set_selected_text_for_tests(&self, text: &str) {
        self.workspace.set_selected_text(text);
    }

    pub(crate) fn close_request_for_tests(self: &Rc<Self>) -> glib::Propagation {
        self.on_close_request()
    }

    pub(crate) fn size_for_tests(&self) -> (i32, i32) {
        self.settings.window_size()
    }

    pub(crate) fn recent_files_for_tests(&self) -> Vec<String> {
        self.workspace.recent_files()
    }

    pub(crate) fn session_files_for_tests(&self) -> Vec<String> {
        self.workspace.session_files()
    }

    pub(crate) fn selected_saved_uri_for_tests(&self) -> String {
        self.workspace.selected_saved_uri()
    }

    pub(crate) fn text_for_uri_for_tests(&self, uri: &str) -> Option<String> {
        self.workspace.text_for_uri(uri)
    }

    pub(crate) fn reorder_selected_to_first_for_tests(&self) -> bool {
        self.workspace.reorder_selected_to_first()
    }

    pub(crate) fn shortcuts_enabled_for_tests(&self) -> bool {
        self.workspace.shortcuts_enabled()
    }

    pub(crate) fn search_visible_for_tests(&self) -> bool {
        self.workspace.search_visible()
    }

    pub(crate) fn replace_visible_for_tests(&self) -> bool {
        self.workspace.replace_visible()
    }

    pub(crate) fn search_query_for_tests(&self) -> String {
        self.workspace.search_query()
    }

    pub(crate) fn search_result_for_tests(&self) -> String {
        self.workspace.search_result()
    }

    pub(crate) fn status_labels_for_tests(&self) -> (String, String, String) {
        self.workspace.status_labels()
    }

    pub(crate) fn status_format_summary_for_tests(&self) -> String {
        self.workspace.status_format_summary()
    }

    pub(crate) fn choose_selected_line_ending_from_preferences_for_tests(
        &self,
        line_ending_mode: crate::editor_format::LineEndingMode,
    ) {
        let index = match line_ending_mode {
            crate::editor_format::LineEndingMode::Lf => 0,
            crate::editor_format::LineEndingMode::CrLf => 1,
            crate::editor_format::LineEndingMode::Cr => 2,
        };
        self.shell.line_ending_row.set_selected(index);
    }

    pub(crate) fn request_selected_encoding_from_preferences_for_tests(&self) {
        libadwaita::prelude::ActionRowExt::activate(&self.shell.encoding_row);
    }

    pub(crate) fn status_zoom_percent_for_tests(&self) -> String {
        self.workspace.status_zoom_percent()
    }

    pub(crate) fn activate_status_zoom_in_for_tests(&self) {
        self.workspace.activate_status_zoom_in();
    }

    pub(crate) fn activate_status_zoom_out_for_tests(&self) {
        self.workspace.activate_status_zoom_out();
    }

    pub(crate) fn activate_status_zoom_reset_for_tests(&self) {
        self.workspace.activate_status_zoom_reset();
    }

    pub(crate) fn selected_line_numbers_visible_for_tests(&self) -> bool {
        self.workspace.selected_line_numbers_visible()
    }

    pub(crate) fn select_offsets_for_tests(&self, start: i32, end: i32) {
        self.workspace.select_offsets_in_selected(start, end);
    }

    pub(crate) fn undo_selected_for_tests(&self) {
        self.workspace.undo_selected();
    }

    pub(crate) fn replace_current_for_tests(self: &Rc<Self>) {
        self.workspace.replace_current_for_tests();
    }

    pub(crate) fn replace_all_for_tests(self: &Rc<Self>) {
        self.workspace.replace_all_for_tests();
    }

    pub(crate) fn set_replace_text_for_tests(&self, text: &str) {
        self.workspace.set_replace_text_for_tests(text);
    }

    pub(crate) fn set_line_numbers_for_tests(&self, enabled: bool) {
        self.shell.line_numbers_row.set_active(enabled);
    }

    pub(crate) fn set_minimap_for_tests(&self, enabled: bool) {
        self.shell.minimap_row.set_active(enabled);
    }

    pub(crate) fn set_current_line_highlight_for_tests(&self, enabled: bool) {
        self.shell.highlight_current_line_row.set_active(enabled);
    }

    pub(crate) fn set_autosave_for_tests(&self, enabled: bool) {
        self.shell.autosave_row.set_active(enabled);
    }

    pub(crate) fn select_editor_palette_for_tests(&self, index: u32) {
        let palette = match index {
            1 => crate::settings::EditorPalette::AdwaitaLight,
            2 => crate::settings::EditorPalette::AdwaitaDark,
            3 => crate::settings::EditorPalette::Kate,
            4 => crate::settings::EditorPalette::KateDark,
            5 => crate::settings::EditorPalette::SolarizedLight,
            6 => crate::settings::EditorPalette::SolarizedDark,
            7 => crate::settings::EditorPalette::Classic,
            _ => crate::settings::EditorPalette::FollowSystem,
        };
        self.appearance.set_palette_for_tests(palette);
    }

    pub(crate) fn set_app_appearance_for_tests(&self, theme: crate::settings::ThemePreference) {
        self.appearance.set_theme_for_tests(theme);
    }

    pub(crate) fn sync_appearance_for_tests(&self) {
        self.appearance.sync_for_tests();
    }

    pub(crate) fn present_appearance_for_tests(&self) {
        self.appearance
            .present_for_tests(&self.shell.appearance_button);
    }

    pub(crate) fn selected_appearance_palette_for_tests(&self) -> crate::settings::EditorPalette {
        self.appearance.selected_palette_for_tests()
    }

    pub(crate) fn set_fullscreen_for_tests(&self, fullscreen: bool) {
        self.set_fullscreen(fullscreen);
    }

    pub(crate) fn persist_window_size_for_tests(&self) {
        self.persist_window_size();
    }

    pub(crate) fn selected_minimap_visible_for_tests(&self) -> bool {
        self.workspace.selected_minimap_visible()
    }

    pub(crate) fn selected_language_id_for_tests(&self) -> Option<String> {
        self.workspace.selected_language_id()
    }

    pub(crate) fn selected_banner_visible_for_tests(&self) -> bool {
        self.workspace.selected_banner_visible()
    }

    pub(crate) fn selected_writability_for_tests(&self) -> Option<crate::editor_tab::Writability> {
        self.workspace.selected_writability()
    }

    pub(crate) fn selected_autosave_eligible_for_tests(&self) -> bool {
        self.workspace.selected_autosave_eligible()
    }

    pub(crate) fn sync_selected_banner_for_tests(&self, window_active: bool) {
        self.workspace.sync_selected_banner_for_tests(window_active);
    }

    pub(crate) fn resolve_selected_external_for_tests(&self) {
        self.workspace.resolve_selected_external_for_tests();
    }

    pub(crate) fn trigger_selected_external_action_for_tests(&self) {
        self.workspace.trigger_selected_external_action_for_tests();
    }

    pub(crate) fn request_selected_autosave_for_tests(&self) {
        self.workspace.request_selected_autosave_for_tests();
    }

    pub(crate) fn inject_external_event_for_tests(
        self: &Rc<Self>,
        uri: &str,
        event: crate::editor_monitor::ExternalFileEvent,
    ) {
        self.workspace.inject_external_event_for_tests(uri, event);
    }

    pub(crate) fn set_insert_spaces_for_tests(&self, enabled: bool) {
        self.shell.insert_spaces_row.set_active(enabled);
    }

    pub(crate) fn set_tab_width_for_tests(&self, width: i32) {
        self.settings.set_tab_width(width);
        self.workspace.apply_indentation_to_tabs();
    }

    pub(crate) fn set_indent_width_for_tests(&self, width: i32) {
        self.settings.set_indent_width(width);
        self.workspace.apply_indentation_to_tabs();
    }

    pub(crate) fn selected_indentation_for_tests(&self) -> Option<(bool, u32, i32)> {
        self.workspace.selected_indentation_for_tests()
    }

    pub(crate) fn indent_selected_lines_for_tests(&self) {
        self.workspace.indent_selected_lines_for_tests();
    }

    pub(crate) fn unindent_selected_lines_for_tests(&self) {
        self.workspace.unindent_selected_lines_for_tests();
    }

    pub(crate) fn selected_visual_column_at_offset_for_tests(&self, offset: i32) -> Option<u32> {
        self.workspace
            .selected_visual_column_at_offset_for_tests(offset)
    }

    pub(crate) fn zoom_percent_for_tests(&self) -> i32 {
        self.zoom.zoom_percent()
    }

    pub(crate) fn selected_minimap_font_for_tests(&self) -> Option<gtk4::pango::FontDescription> {
        self.workspace.selected_minimap_font_for_tests()
    }

    pub(crate) fn selected_zoom_class_for_tests(&self) -> bool {
        self.workspace.selected_zoom_class_for_tests()
    }

    pub(crate) fn selected_scroll_past_end_padding_for_tests(&self) -> Option<(i32, i32)> {
        self.workspace.selected_scroll_past_end_padding_for_tests()
    }

    pub(crate) fn preferences_write_log_for_tests(&self) -> Vec<String> {
        self.settings.write_log_for_tests()
    }

    pub(crate) fn indentation_control_state_for_tests(&self) -> ((bool, f64), (bool, f64)) {
        (
            (
                self.shell.tab_width_row.is_editable(),
                self.shell.tab_width_row.adjustment().step_increment(),
            ),
            (
                self.shell.indent_width_row.is_editable(),
                self.shell.indent_width_row.adjustment().step_increment(),
            ),
        )
    }

    pub(crate) fn project_root_uri_for_tests(&self) -> Option<String> {
        self.project.root_uri_for_tests()
    }

    pub(crate) fn project_sidebar_visible_for_tests(&self) -> bool {
        self.shell.project_split_view.shows_sidebar()
    }

    pub(crate) fn project_action_states_for_tests(&self) -> (bool, bool, bool, bool) {
        self.project.action_states_for_tests()
    }

    pub(crate) fn project_tree_entry_names_for_tests(&self) -> Vec<String> {
        self.project.tree_entry_names_for_tests()
    }

    pub(crate) fn close_project_for_tests(&self) {
        self.project.close_for_tests();
    }

    pub(crate) fn refresh_project_for_tests(&self) {
        self.project.refresh_for_tests();
    }

    pub(crate) fn set_project_show_hidden_for_tests(&self, show_hidden: bool) {
        self.project.set_show_hidden_for_tests(show_hidden);
    }

    pub(crate) fn resolve_project_symlink_for_tests(&self, file: &gio::File) {
        self.project.resolve_symlink_for_tests(file);
    }

    pub(crate) fn compare_action_states_for_tests(&self) -> (bool, bool, bool, bool, bool) {
        self.compare.action_states_for_tests()
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

    pub(crate) fn project_monitor_count_for_tests(&self) -> usize {
        self.project.project_monitor_count_for_tests()
    }

    pub(crate) fn trigger_project_auto_refresh_for_tests(&self) {
        self.project.trigger_project_auto_refresh_for_tests();
    }

    pub(crate) fn expand_project_tree_entry_for_tests(&self, name: &str) -> bool {
        self.project.expand_tree_entry_for_tests(name)
    }

    pub(crate) fn selected_project_tree_uri_for_tests(&self) -> Option<String> {
        self.project.selected_tree_uri_for_tests()
    }
}
