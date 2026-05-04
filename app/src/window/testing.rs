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

    pub(crate) fn selected_style_scheme_id_for_tests(&self) -> Option<String> {
        self.workspace.selected_style_scheme_id()
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

    pub(crate) fn activate_tab_move_backward_for_tests(&self) -> bool {
        gtk4::prelude::WidgetExt::activate_action(self.widget(), "win.tab-move-backward", None)
            .is_ok()
    }

    pub(crate) fn activate_tab_move_forward_for_tests(&self) -> bool {
        gtk4::prelude::WidgetExt::activate_action(self.widget(), "win.tab-move-forward", None)
            .is_ok()
    }

    pub(crate) fn activate_tab_move_to_new_window_for_tests(&self) -> bool {
        gtk4::prelude::WidgetExt::activate_action(self.widget(), "win.tab-move-to-new-window", None)
            .is_ok()
    }

    pub(crate) fn activate_close_other_tabs_for_tests(&self) -> bool {
        gtk4::prelude::WidgetExt::activate_action(self.widget(), "win.close-other-tabs", None)
            .is_ok()
    }

    pub(crate) fn shortcuts_enabled_for_tests(&self) -> bool {
        self.workspace.shortcuts_enabled()
    }

    pub(crate) fn open_button_action_for_tests(&self) -> Option<String> {
        self.shell
            .open_button
            .action_name()
            .map(|action| action.to_string())
    }

    pub(crate) fn tab_chrome_layout_for_tests(&self) -> bool {
        self.workspace.tab_bar_controls_tab_view()
            && self.workspace.top_bar_order_matches(&self.shell.header_bar)
            && self.shell.toolbar_view.top_bar_style() == libadwaita::ToolbarStyle::Flat
            && self.workspace_box_contains_only_tab_view_for_tests()
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

    pub(crate) fn set_word_wrap_for_tests(&self, enabled: bool) {
        self.shell.word_wrap_row.set_active(enabled);
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
            3 => crate::settings::EditorPalette::ClassicLight,
            4 => crate::settings::EditorPalette::ClassicDark,
            5 => crate::settings::EditorPalette::Kate,
            6 => crate::settings::EditorPalette::KateDark,
            7 => crate::settings::EditorPalette::SolarizedLight,
            8 => crate::settings::EditorPalette::SolarizedDark,
            _ => crate::settings::EditorPalette::FollowSystem,
        };
        self.appearance.set_palette_for_tests(palette);
    }

    pub(crate) fn set_app_appearance_for_tests(&self, theme: crate::settings::ThemePreference) {
        crate::window_theme::set_theme_for_tests(&self.theme_action, theme);
    }

    pub(crate) fn sync_appearance_for_tests(&self) {
        self.appearance.sync_for_tests();
    }

    pub(crate) fn present_appearance_for_tests(&self) {
        self.appearance.present_for_tests(&self.shell.window);
    }

    pub(crate) fn selected_appearance_palette_for_tests(&self) -> crate::settings::EditorPalette {
        self.appearance.selected_palette_for_tests()
    }

    pub(crate) fn chrome_css_for_tests(&self) -> String {
        crate::app_chrome::chrome_css_for_settings(&self.settings)
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

    pub(crate) fn request_selected_guarded_autosave_for_tests(
        self: &Rc<Self>,
    ) -> crate::workspace::AutosaveRequestForTests {
        self.workspace.request_selected_guarded_autosave_for_tests()
    }

    pub(crate) fn source_control_status_for_tests(&self) -> String {
        self.source_control.status_label_for_tests()
    }

    pub(crate) fn source_control_row_count_for_tests(&self) -> usize {
        self.source_control.row_count_for_tests()
    }

    pub(crate) fn source_control_activate_path_for_tests(&self, path: &str) -> bool {
        self.source_control.activate_path_for_tests(path)
    }

    pub(crate) fn source_control_row_state_for_tests(
        &self,
        path: &str,
    ) -> Option<(String, bool, bool)> {
        self.source_control.row_state_for_tests(path)
    }

    pub(crate) fn set_source_control_view_mode_for_tests(
        &self,
        mode: crate::settings::SourceControlViewMode,
    ) {
        self.source_control.set_view_mode_for_tests(mode);
    }

    pub(crate) fn source_control_recent_commit_count_for_tests(&self) -> usize {
        self.source_control.recent_commit_count_for_tests()
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

    pub(crate) fn selected_zoom_css_classes_for_tests(&self) -> Vec<String> {
        self.workspace.selected_zoom_css_classes_for_tests()
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
        self.shell.project_split_view.position() > 0
    }

    pub(crate) fn project_sidebar_position_for_tests(&self) -> i32 {
        self.shell.project_split_view.position()
    }

    pub(crate) fn set_project_sidebar_position_for_tests(&self, position: i32) {
        self.shell.project_split_view.set_position(position);
    }

    pub(crate) fn set_project_sidebar_visible_for_tests(&self, visible: bool) {
        self.project.set_sidebar_visible_for_tests(visible);
    }

    pub(crate) fn project_sidebar_left_layout_for_tests(&self) -> bool {
        let start_is_sidebar = self
            .shell
            .project_split_view
            .start_child()
            .and_then(|child| child.downcast::<libadwaita::ToolbarView>().ok())
            .is_some();
        let end_is_workspace = self
            .shell
            .project_split_view
            .end_child()
            .is_some_and(|child| {
                child == self.shell.workspace_box.clone().upcast::<gtk4::Widget>()
            });
        start_is_sidebar && end_is_workspace
    }

    fn workspace_box_contains_only_tab_view_for_tests(&self) -> bool {
        let Some(first_child) = self.shell.workspace_box.first_child() else {
            return false;
        };
        first_child == self.workspace.tab_view.clone().upcast::<gtk4::Widget>()
            && first_child.next_sibling().is_none()
    }

    pub(crate) fn source_control_icon_for_tests(&self) -> Option<String> {
        self.sidebar_host.source_control_icon_for_tests()
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

    pub(crate) fn selected_wrap_mode_for_tests(&self) -> Option<gtk4::WrapMode> {
        self.workspace
            .selected_tab()
            .map(|tab| tab.text_view().wrap_mode())
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

    pub(crate) fn project_reveal_pending_for_tests(&self) -> bool {
        self.project.reveal_pending_for_tests()
    }

    pub(crate) fn reveal_project_file_for_tests(&self, file: &gio::File) {
        self.project.reveal_file_for_tests(file);
    }

    pub(crate) fn reset_project_reveal_scan_count_for_tests(&self) {
        let _root = self.project.root_uri_for_tests();
        crate::window_project::WindowProjectController::reset_reveal_scan_count_for_tests();
    }

    pub(crate) fn project_reveal_scan_count_for_tests(&self) -> usize {
        let _root = self.project.root_uri_for_tests();
        crate::window_project::WindowProjectController::reveal_scan_count_for_tests()
    }
}
