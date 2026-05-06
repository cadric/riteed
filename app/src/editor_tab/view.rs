use gtk4::{pango, prelude::*};
use sourceview5::prelude::*;

use super::EditorTab;
use crate::editor_zoom::{clear_zoom_css_classes, restore_zoom_css_class};

#[cfg(test)]
use crate::editor_zoom::{EDITOR_VIEW_CSS_CLASS, EDITOR_ZOOM_CSS_CLASS_PREFIX};

#[cfg(test)]
use std::rc::Rc;

impl EditorTab {
    pub fn grab_focus(&self) {
        self.text_view.grab_focus();
    }

    pub fn apply_word_wrap(&self) {
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
        let show_minimap = self.settings.show_minimap() && !self.is_compare_active();
        self.minimap_holder.set_visible(show_minimap);
        let policy = if show_minimap {
            gtk4::PolicyType::External
        } else {
            gtk4::PolicyType::Automatic
        };
        self.scrolled.set_vscrollbar_policy(policy);
    }

    pub fn apply_indentation(&self) {
        self.settings.apply_indentation(&self.text_view);
    }

    pub(crate) fn apply_source_style_scheme(&self) {
        self.settings.apply_source_style_scheme(&self.text_buffer);
        self.apply_compare_style();
    }

    pub fn apply_current_line_highlight(&self) {
        self.text_view
            .set_highlight_current_line(self.settings.highlight_current_line());
    }

    pub fn apply_minimap_font_desc(&self, font_desc: Option<&pango::FontDescription>) {
        self.minimap.set_font_desc(font_desc);
    }

    pub fn apply_scroll_past_end_padding(&self, bottom_margin: i32) {
        self.text_view.set_bottom_margin(bottom_margin);
        self.minimap.set_bottom_margin(bottom_margin);
    }

    pub fn clear_zoom_style(&self) {
        clear_zoom_css_classes(&self.text_view);
        self.clear_compare_zoom_style();
        self.minimap
            .set_font_desc(Option::<&pango::FontDescription>::None);
    }

    pub fn restore_zoom_style(&self, css_class: &str) {
        restore_zoom_css_class(&self.text_view, css_class);
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
        self: &Rc<Self>,
        event: crate::editor_monitor::ExternalFileEvent,
    ) {
        self.handle_external_event(event);
    }
}
