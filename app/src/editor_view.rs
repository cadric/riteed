use gtk4::prelude::*;
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::editor_zoom::{
    EDITOR_VIEW_CSS_CLASS, resolve_minimap_font_description, resolve_scroll_past_end_padding,
};
use crate::settings::AppSettings;

pub struct EditorView {
    pub root: gtk4::Box,
    pub banner: adw::Banner,
    pub text_buffer: sourceview5::Buffer,
    pub text_view: sourceview5::View,
    pub minimap: sourceview5::Map,
    pub minimap_holder: gtk4::Box,
    pub scrolled: gtk4::ScrolledWindow,
}

impl EditorView {
    #[must_use]
    pub fn new(settings: &AppSettings) -> Self {
        let text_buffer = sourceview5::Buffer::builder().enable_undo(true).build();
        let text_view = sourceview5::View::with_buffer(&text_buffer);
        let scroll_past_end_padding = resolve_scroll_past_end_padding(&settings.editor_font());
        text_view.set_accepts_tab(true);
        text_view.set_bottom_margin(scroll_past_end_padding);
        text_view.set_hexpand(true);
        text_view.set_left_margin(12);
        text_view.set_monospace(true);
        text_view.set_right_margin(12);
        text_view.set_show_line_numbers(settings.show_line_numbers());
        text_view.set_top_margin(12);
        text_view.set_vexpand(true);
        text_view.add_css_class(EDITOR_VIEW_CSS_CLASS);
        settings.apply_word_wrap(&text_view);
        settings.apply_indentation(&text_view);

        let scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&text_view)
            .build();
        scrolled.set_hexpand(true);
        scrolled.set_min_content_height(0);
        scrolled.set_min_content_width(0);
        scrolled.set_propagate_natural_height(false);
        scrolled.set_propagate_natural_width(false);
        scrolled.set_vexpand(true);

        let minimap_font = resolve_minimap_font_description(&settings.editor_font());
        let minimap = sourceview5::Map::builder()
            .view(&text_view)
            .font_desc(&minimap_font)
            .build();
        minimap.set_can_focus(false);
        minimap.set_cursor_visible(false);
        minimap.set_editable(false);
        minimap.set_focusable(false);
        minimap.set_hexpand(false);
        minimap.set_monospace(true);
        minimap.set_bottom_margin(scroll_past_end_padding);
        minimap.set_vexpand(true);

        let minimap_holder = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .visible(settings.show_minimap())
            .build();
        minimap_holder.set_hexpand(false);
        minimap_holder.set_vexpand(true);
        minimap_holder.set_width_request(96);
        minimap_holder.append(&minimap);

        let banner = adw::Banner::new("");
        banner.set_button_label(None);
        banner.set_revealed(false);

        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .build();
        content.set_hexpand(true);
        content.set_vexpand(true);
        content.append(&scrolled);
        content.append(&minimap_holder);

        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        root.set_hexpand(true);
        root.set_vexpand(true);
        root.append(&banner);
        root.append(&content);

        Self {
            root,
            banner,
            text_buffer,
            text_view,
            minimap,
            minimap_holder,
            scrolled,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReloadSnapshot {
    cursor_line: i32,
    cursor_column: i32,
    selection: Option<((i32, i32), (i32, i32))>,
}

impl ReloadSnapshot {
    #[must_use]
    pub fn capture(buffer: &sourceview5::Buffer) -> Self {
        let cursor = buffer.iter_at_mark(&buffer.get_insert());
        let selection = buffer.selection_bounds().map(|(start, end)| {
            (
                (start.line(), start.line_offset()),
                (end.line(), end.line_offset()),
            )
        });

        Self {
            cursor_line: cursor.line(),
            cursor_column: cursor.line_offset(),
            selection,
        }
    }

    pub fn apply(&self, buffer: &sourceview5::Buffer) {
        if let Some((start, end)) = self.selection {
            let start_iter = clamped_iter(buffer, start.0, start.1);
            let end_iter = clamped_iter(buffer, end.0, end.1);
            buffer.select_range(&start_iter, &end_iter);
        } else {
            let iter = clamped_iter(buffer, self.cursor_line, self.cursor_column);
            buffer.place_cursor(&iter);
        }
    }
}

fn clamped_iter(buffer: &sourceview5::Buffer, line: i32, column: i32) -> gtk4::TextIter {
    let line_count = buffer.line_count().max(1);
    let clamped_line = line.clamp(0, line_count - 1);
    let mut iter = buffer
        .iter_at_line(clamped_line)
        .unwrap_or_else(|| buffer.end_iter());
    let max_offset = iter.chars_in_line();
    iter.forward_chars(column.clamp(0, max_offset));
    iter
}
