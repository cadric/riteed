use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, pango, prelude::*};
use sourceview5::prelude::*;

use super::EditorTab;
use crate::editor_zoom::{clear_zoom_css_classes, restore_zoom_css_class};

#[cfg(test)]
use crate::editor_zoom::{EDITOR_VIEW_CSS_CLASS, EDITOR_ZOOM_CSS_CLASS_PREFIX};

const MARKDOWN_PREVIEW_DEBOUNCE_MS: u64 = 180;
const MARKDOWN_PREVIEW_MAX_BYTES: usize = 1_000_000;

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
        let show_minimap = self.settings.show_minimap()
            && !self.is_compare_active()
            && !self.is_markdown_preview_active();
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
    pub fn markdown_preview_available(&self) -> bool {
        self.is_document()
            && !self.is_compare_active()
            && self
                .state
                .borrow()
                .document
                .document
                .path()
                .is_some_and(|path| crate::markdown::is_markdown_path(&path))
    }

    #[must_use]
    pub fn can_toggle_markdown_preview(&self) -> bool {
        self.is_markdown_preview_active() || self.markdown_preview_available()
    }

    #[must_use]
    pub fn is_markdown_preview_active(&self) -> bool {
        self.state.borrow().ui.markdown_preview.active
    }

    pub fn toggle_markdown_preview(self: &Rc<Self>) {
        if self.is_markdown_preview_active() {
            self.exit_markdown_preview();
        } else {
            self.enter_markdown_preview();
        }
    }

    pub(crate) fn sync_markdown_preview_availability(self: &Rc<Self>) {
        if self.is_markdown_preview_active() && !self.markdown_preview_available() {
            self.exit_markdown_preview();
        } else if self.is_markdown_preview_active() {
            self.schedule_markdown_preview_update();
        }
    }

    pub(crate) fn schedule_markdown_preview_update(self: &Rc<Self>) {
        let generation = {
            let mut state = self.state.borrow_mut();
            if !state.ui.markdown_preview.active {
                return;
            }
            if let Some(source) = state.ui.markdown_preview.debounce.take() {
                source.remove();
            }
            state.ui.markdown_preview.generation =
                state.ui.markdown_preview.generation.saturating_add(1);
            state.ui.markdown_preview.generation
        };
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(
            Duration::from_millis(MARKDOWN_PREVIEW_DEBOUNCE_MS),
            move || {
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                tab.render_scheduled_markdown_preview(generation);
            },
        );
        self.state.borrow_mut().ui.markdown_preview.debounce = Some(source);
    }

    pub(crate) fn install_markdown_preview_link_handler(self: &Rc<Self>) {
        let click = gtk4::GestureClick::new();
        click.set_button(0);
        let view = self.preview_view.clone();
        let weak = Rc::downgrade(self);
        click.connect_released(move |_, press_count, x, y| {
            if press_count != 1 {
                return;
            }
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let (buffer_x, buffer_y) = view.window_to_buffer_coords(
                gtk4::TextWindowType::Widget,
                text_view_coordinate(x),
                text_view_coordinate(y),
            );
            if let Some(iter) = view.iter_at_location(buffer_x, buffer_y) {
                tab.open_markdown_link_at_offset(iter.offset());
            }
        });
        self.preview_view.add_controller(click);
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

    fn enter_markdown_preview(self: &Rc<Self>) {
        if !self.markdown_preview_available() {
            return;
        }
        self.exit_compare();
        {
            let mut state = self.state.borrow_mut();
            if state.ui.markdown_preview.active {
                return;
            }
            state.ui.markdown_preview.active = true;
            state.ui.markdown_preview.links.clear();
        }
        self.root.remove(&self.content);
        self.root.append(&self.preview_scrolled);
        self.apply_minimap_visibility();
        self.schedule_markdown_preview_update();
        self.preview_view.grab_focus();
        self.sync_presentation();
    }

    pub(crate) fn exit_markdown_preview(&self) {
        let was_active = {
            let mut state = self.state.borrow_mut();
            if let Some(source) = state.ui.markdown_preview.debounce.take() {
                source.remove();
            }
            state.ui.markdown_preview.links.clear();
            let was_active = state.ui.markdown_preview.active;
            state.ui.markdown_preview.active = false;
            was_active
        };
        if was_active {
            self.root.remove(&self.preview_scrolled);
            self.root.append(&self.content);
            self.apply_minimap_visibility();
            self.sync_presentation();
        }
    }

    fn render_scheduled_markdown_preview(&self, generation: u64) {
        let should_render = {
            let mut state = self.state.borrow_mut();
            if state.ui.markdown_preview.generation != generation
                || !state.ui.markdown_preview.active
            {
                return;
            }
            state.ui.markdown_preview.debounce = None;
            true
        };
        if !should_render {
            return;
        }
        let text = self.buffer_text();
        let output = if markdown_preview_uses_fallback(text.len()) {
            crate::markdown::render_large_document_fallback(&self.preview_buffer)
        } else {
            let document = crate::markdown::parse_document(&text);
            crate::markdown::render_document(&self.preview_buffer, &document)
        };
        self.state.borrow_mut().ui.markdown_preview.links = output.links;
    }

    fn open_markdown_link_at_offset(&self, offset: i32) {
        let target = {
            let state = self.state.borrow();
            crate::markdown::link_target_at(&state.ui.markdown_preview.links, offset)
        };
        let Some(target) = target.filter(|target| markdown_link_is_launchable(target)) else {
            return;
        };
        let launcher = gtk4::UriLauncher::new(&target);
        launcher.launch(
            None::<&gtk4::Window>,
            None::<&gio::Cancellable>,
            |_result| {},
        );
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
    pub(crate) fn minimap_scrollbar_policy_for_tests(&self) -> gtk4::PolicyType {
        self.scrolled.vscrollbar_policy()
    }

    #[cfg(test)]
    pub(crate) fn markdown_preview_active_for_tests(&self) -> bool {
        self.is_markdown_preview_active()
    }

    #[cfg(test)]
    pub(crate) fn markdown_preview_text_for_tests(&self) -> String {
        self.preview_buffer
            .text(
                &self.preview_buffer.start_iter(),
                &self.preview_buffer.end_iter(),
                true,
            )
            .to_string()
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

fn markdown_link_is_launchable(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

fn markdown_preview_uses_fallback(len: usize) -> bool {
    len > MARKDOWN_PREVIEW_MAX_BYTES
}

fn text_view_coordinate(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    let rounded = value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX));
    rounded.to_string().parse::<i32>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{MARKDOWN_PREVIEW_MAX_BYTES, markdown_preview_uses_fallback};

    #[test]
    fn markdown_preview_at_minus_one_renders() {
        assert!(!markdown_preview_uses_fallback(
            MARKDOWN_PREVIEW_MAX_BYTES - 1
        ));
    }

    #[test]
    fn markdown_preview_at_exact_renders() {
        assert!(!markdown_preview_uses_fallback(MARKDOWN_PREVIEW_MAX_BYTES));
    }

    #[test]
    fn markdown_preview_at_plus_one_falls_back() {
        assert!(markdown_preview_uses_fallback(
            MARKDOWN_PREVIEW_MAX_BYTES + 1
        ));
    }
}
