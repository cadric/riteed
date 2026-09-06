use gtk4::{pango, prelude::*};
use sourceview5::prelude::*;

use super::EditorTab;
use crate::editor_zoom::{
    clear_zoom_css_classes, effective_scroll_past_end_padding, resolve_scroll_past_end_padding,
    restore_zoom_css_class,
};

#[cfg(test)]
use crate::editor_zoom::{EDITOR_VIEW_CSS_CLASS, EDITOR_ZOOM_CSS_CLASS_PREFIX};

impl EditorTab {
    pub fn grab_focus(&self) {
        self.text_view.grab_focus();
    }

    pub fn apply_word_wrap(&self) {
        if self.kind() == super::TabKind::GitReview {
            let wrap_mode = if self.settings.compare_word_wrap() {
                gtk4::WrapMode::WordChar
            } else {
                gtk4::WrapMode::None
            };
            self.text_view.set_wrap_mode(wrap_mode);
            return;
        }
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            self.text_view.set_wrap_mode(gtk4::WrapMode::None);
            compare.apply_wrap_override();
            return;
        }
        self.settings.apply_word_wrap(&self.text_view);
    }

    pub fn apply_line_numbers(&self) {
        self.text_view
            .set_show_line_numbers(self.settings.show_line_numbers());
    }

    pub fn apply_minimap_visibility(&self) {
        let show_minimap = self.settings.show_minimap();
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            compare.apply_minimap_visibility(show_minimap);
        }
        let show_minimap = show_minimap
            && self.editor_heavy_features_enabled()
            && !self.is_compare_active()
            && !self.is_markdown_preview_active()
            && self.state.borrow().io.pending_apply.is_none();
        self.minimap_holder.set_visible(show_minimap);
        self.scrolled
            .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
    }

    pub fn apply_indentation(&self) {
        self.settings.apply_indentation(&self.text_view);
    }

    pub(crate) fn apply_source_style_scheme(&self) {
        self.settings.apply_source_style_scheme(&self.text_buffer);
        self.apply_compare_style();
        self.refresh_source_control_minimap_colors();
    }

    pub fn apply_current_line_highlight(&self) {
        self.text_view
            .set_highlight_current_line(self.settings.highlight_current_line());
    }

    pub fn apply_minimap_font_desc(&self, font_desc: Option<&pango::FontDescription>) {
        self.minimap.set_font_desc(font_desc);
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            compare.apply_minimap_font_desc(font_desc);
        }
    }

    // GtkSourceMap mirrors the view's bottom margin through its own scaled
    // property binding; setting the unscaled value on the map directly breaks
    // its drag-position math near the end of large documents.
    pub fn apply_scroll_past_end_padding(&self, bottom_margin: i32) {
        self.state.borrow_mut().ui.scroll_past_end_floor = bottom_margin;
        self.refresh_scroll_past_end_padding();
        if let Ok(mut state) = self.state.try_borrow_mut()
            && let Some(compare) = state.compare.active.as_mut()
        {
            compare.apply_scroll_past_end_padding(bottom_margin);
        }
    }

    // The effective margin is the larger of the zoom-scaled font floor and
    // 75 % of the visible viewport, re-resolved whenever the viewport's
    // page-size changes.
    pub(crate) fn refresh_scroll_past_end_padding(&self) {
        let padding = effective_scroll_past_end_padding(
            self.scroll_past_end_floor(),
            self.scrolled.vadjustment().page_size(),
        );
        self.text_view.set_bottom_margin(padding);
    }

    pub(crate) fn scroll_past_end_floor(&self) -> i32 {
        let floor = self.state.borrow().ui.scroll_past_end_floor;
        if floor > 0 {
            floor
        } else {
            resolve_scroll_past_end_padding(&self.settings.editor_font())
        }
    }

    pub fn clear_zoom_style(&self) {
        clear_zoom_css_classes(&self.text_view);
        self.clear_markdown_preview_zoom_style();
        self.clear_compare_zoom_style();
        self.minimap
            .set_font_desc(Option::<&pango::FontDescription>::None);
    }

    pub fn restore_zoom_style(&self, css_class: &str) {
        restore_zoom_css_class(&self.text_view, css_class);
        self.restore_markdown_preview_zoom_style(css_class);
        self.restore_compare_zoom_style(css_class);
    }

    #[must_use]
    pub fn text_buffer(&self) -> sourceview5::Buffer {
        self.text_buffer.clone()
    }

    #[must_use]
    pub fn text_view(&self) -> sourceview5::View {
        self.text_view.clone()
    }

    pub(crate) fn select_offsets(&self, start: i32, end: i32) {
        let start_iter = self.text_buffer.iter_at_offset(start);
        let end_iter = self.text_buffer.iter_at_offset(end);
        self.text_buffer.select_range(&start_iter, &end_iter);
        let mut scroll_iter = start_iter;
        self.text_view
            .scroll_to_iter(&mut scroll_iter, 0.2, false, 0.0, 0.0);
    }

    #[cfg(test)]
    pub(crate) fn set_text_for_tests(&self, text: &str) {
        self.text_buffer.set_text(text);
        self.sync_presentation();
    }

    #[cfg(test)]
    pub(crate) fn reset_presentation_sync_count_for_tests(&self) {
        self.state.borrow_mut().ui.presentation_sync_count = 0;
    }

    #[cfg(test)]
    pub(crate) fn presentation_sync_count_for_tests(&self) -> usize {
        self.state.borrow().ui.presentation_sync_count
    }

    #[cfg(test)]
    pub(crate) fn dirty_indicator_visible_for_tests(&self) -> Option<bool> {
        self.page().map(|page| page.indicator_icon().is_some())
    }

    #[cfg(test)]
    pub(crate) fn select_offsets_for_tests(&self, start: i32, end: i32) {
        self.select_offsets(start, end);
    }

    #[cfg(test)]
    pub(crate) fn undo_for_tests(&self) {
        self.text_buffer.undo();
    }

    #[cfg(test)]
    pub(crate) fn shows_line_numbers_for_tests(&self) -> bool {
        self.text_view.shows_line_numbers()
    }

    #[cfg(test)]
    pub(crate) fn minimap_visible_for_tests(&self) -> bool {
        self.minimap_holder.property::<bool>("visible")
    }

    #[cfg(test)]
    pub(crate) fn minimap_scrollbar_policy_for_tests(&self) -> gtk4::PolicyType {
        self.scrolled.vscrollbar_policy()
    }

    #[cfg(test)]
    pub(crate) fn indentation_for_tests(&self) -> (bool, u32, i32) {
        (
            self.text_view.is_insert_spaces_instead_of_tabs(),
            self.text_view.tab_width(),
            self.text_view.indent_width(),
        )
    }

    #[cfg(test)]
    pub(crate) fn indent_selected_lines_for_tests(&self) {
        let (mut start, mut end) = if let Some(bounds) = self.text_buffer.selection_bounds() {
            bounds
        } else {
            let cursor = self
                .text_buffer
                .iter_at_mark(&self.text_buffer.get_insert());
            (cursor, cursor)
        };
        self.text_view.indent_lines(&mut start, &mut end);
        self.sync_presentation();
    }

    #[cfg(test)]
    pub(crate) fn unindent_selected_lines_for_tests(&self) {
        let (mut start, mut end) = if let Some(bounds) = self.text_buffer.selection_bounds() {
            bounds
        } else {
            let cursor = self
                .text_buffer
                .iter_at_mark(&self.text_buffer.get_insert());
            (cursor, cursor)
        };
        self.text_view.unindent_lines(&mut start, &mut end);
        self.sync_presentation();
    }

    #[cfg(test)]
    pub(crate) fn visual_column_at_offset_for_tests(&self, offset: i32) -> u32 {
        let iter = self.text_buffer.iter_at_offset(offset);
        self.text_view.visual_column(&iter)
    }

    #[cfg(test)]
    pub(crate) fn minimap_font_desc_for_tests(&self) -> Option<pango::FontDescription> {
        self.minimap.font_desc()
    }

    #[cfg(test)]
    pub(crate) fn scroll_past_end_padding_for_tests(&self) -> (i32, i32) {
        (self.text_view.bottom_margin(), self.minimap.bottom_margin())
    }

    #[cfg(test)]
    pub(crate) fn scroll_past_end_floor_for_tests(&self) -> i32 {
        self.scroll_past_end_floor()
    }

    #[cfg(test)]
    pub(crate) fn set_viewport_page_size_for_tests(&self, page_size: f64) {
        self.scrolled.vadjustment().set_page_size(page_size);
    }

    #[cfg(test)]
    pub(crate) fn view_has_zoom_class_for_tests(&self) -> bool {
        self.text_view.has_css_class(EDITOR_VIEW_CSS_CLASS)
    }

    #[cfg(test)]
    pub(crate) fn zoom_css_classes_for_tests(&self) -> Vec<String> {
        self.text_view
            .css_classes()
            .into_iter()
            .filter(|css_class| css_class.as_str().starts_with(EDITOR_ZOOM_CSS_CLASS_PREFIX))
            .map(|css_class| css_class.to_string())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn inject_external_event_for_tests(
        self: &std::rc::Rc<Self>,
        event: crate::editor_monitor::ExternalFileEvent,
    ) {
        self.handle_external_event(event);
    }
}
