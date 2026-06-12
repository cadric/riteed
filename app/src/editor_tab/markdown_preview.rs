use std::rc::Rc;
use std::time::Duration;

use gtk4::{gdk, gio, glib, prelude::*};

use super::EditorTab;
use crate::editor_search::SearchTarget;
use crate::editor_zoom::restore_preview_zoom_css_class;

#[cfg(test)]
use crate::editor_zoom::{EDITOR_ZOOM_CSS_CLASS_PREFIX, MARKDOWN_PREVIEW_CSS_CLASS};

const MARKDOWN_PREVIEW_DEBOUNCE_MS: u64 = 180;
struct MarkdownPreviewSnapshot {
    scroll_value: f64,
    selection: Option<(i32, i32)>,
}

impl EditorTab {
    #[must_use]
    pub fn markdown_preview_available(&self) -> bool {
        self.is_document()
            && !self.is_compare_active()
            && self
                .state
                .borrow()
                .large_file
                .file_size
                .is_none_or(crate::document_limits::markdown_preview_enabled)
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

    // PARSER-BOUNDARY: id=markdown_preview_render
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

    pub(crate) fn install_markdown_preview_interactions(self: &Rc<Self>) {
        self.install_markdown_preview_link_handler();
        self.install_markdown_preview_clipboard();
    }

    pub(crate) fn clear_markdown_preview_zoom_style(&self) {
        crate::editor_zoom::clear_zoom_css_classes(&self.preview_view);
    }

    pub(crate) fn restore_markdown_preview_zoom_style(&self, css_class: &str) {
        restore_preview_zoom_css_class(&self.preview_view, css_class);
    }

    #[must_use]
    pub(crate) fn capture_search_target_for_open(&self) -> SearchTarget {
        if self.text_view.has_focus() {
            SearchTarget::Source
        } else if self.preview_view.has_focus() || self.is_markdown_preview_active() {
            SearchTarget::Preview
        } else {
            SearchTarget::Source
        }
    }

    #[must_use]
    pub(crate) fn capture_search_target_for_rebind(&self) -> SearchTarget {
        if self.text_view.has_focus() {
            SearchTarget::Source
        } else if self.is_markdown_preview_active() {
            SearchTarget::Preview
        } else {
            SearchTarget::Source
        }
    }

    #[must_use]
    pub(crate) fn single_line_search_selection_text(&self, target: SearchTarget) -> Option<String> {
        match target {
            SearchTarget::Source => self.single_line_selection_text(),
            SearchTarget::Preview => single_line_buffer_selection_text(&self.preview_buffer),
        }
    }

    #[must_use]
    pub(crate) fn preview_search_widgets(
        &self,
    ) -> Option<(gtk4::TextBuffer, gtk4::TextView, gtk4::ScrolledWindow)> {
        if self.is_markdown_preview_active() {
            Some((
                self.preview_buffer.clone(),
                self.preview_view.clone(),
                self.preview_scrolled.clone(),
            ))
        } else {
            None
        }
    }

    pub(crate) fn notify_markdown_preview_change(&self) {
        let callback = self.on_markdown_preview_change.borrow().clone();
        if let Some(callback) = callback {
            callback();
        }
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
        self.notify_markdown_preview_change();
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
            self.notify_markdown_preview_change();
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
        let snapshot = self.capture_markdown_preview_snapshot();
        let text = self.buffer_text();
        let output = if markdown_preview_uses_fallback(text.len()) {
            crate::markdown::render_large_document_fallback(&self.preview_buffer)
        } else {
            let document = crate::markdown::parse_document(&text);
            crate::markdown::render_document(&self.preview_buffer, &document)
        };
        self.state.borrow_mut().ui.markdown_preview.links = output.links;
        self.restore_markdown_preview_snapshot(&snapshot);
        self.notify_markdown_preview_change();
    }

    fn install_markdown_preview_link_handler(self: &Rc<Self>) {
        let click = gtk4::GestureClick::new();
        click.set_button(gdk::BUTTON_PRIMARY);
        let view = self.preview_view.clone();
        let buffer = self.preview_buffer.clone();
        let weak = Rc::downgrade(self);
        click.connect_released(move |gesture, press_count, x, y| {
            if press_count != 1
                || has_click_modifier(gesture.current_event_state())
                || buffer_has_selection(&buffer)
            {
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

    fn install_markdown_preview_clipboard(&self) {
        let buffer = self.preview_buffer.clone();
        let scrolled = self.preview_scrolled.clone();
        self.preview_view.connect_copy_clipboard(move |view| {
            if copy_markdown_preview_selection(&buffer, view, &scrolled) {
                view.stop_signal_emission_by_name("copy-clipboard");
            }
        });

        let controller = gtk4::ShortcutController::new();
        controller.set_scope(gtk4::ShortcutScope::Local);
        add_copy_shortcut(&controller, "<Control>c");
        add_copy_shortcut(&controller, "<Control>Insert");
        self.preview_view.add_controller(controller);
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

    fn capture_markdown_preview_snapshot(&self) -> MarkdownPreviewSnapshot {
        let adjustment = self.preview_scrolled.vadjustment();
        MarkdownPreviewSnapshot {
            scroll_value: adjustment.value(),
            selection: selection_offsets(&self.preview_buffer),
        }
    }

    fn restore_markdown_preview_snapshot(&self, snapshot: &MarkdownPreviewSnapshot) {
        if let Some((start, end)) = snapshot.selection {
            let max_offset = self.preview_buffer.end_iter().offset();
            let start = start.clamp(0, max_offset);
            let end = end.clamp(0, max_offset);
            let start_iter = self.preview_buffer.iter_at_offset(start);
            let end_iter = self.preview_buffer.iter_at_offset(end);
            if start != end {
                self.preview_buffer.select_range(&start_iter, &end_iter);
            }
        }

        let adjustment = self.preview_scrolled.vadjustment();
        let upper = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        adjustment.set_value(snapshot.scroll_value.clamp(adjustment.lower(), upper));
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
    pub(crate) fn markdown_preview_zoom_css_classes_for_tests(&self) -> Vec<String> {
        self.preview_view
            .css_classes()
            .into_iter()
            .filter(|css_class| css_class.as_str().starts_with(EDITOR_ZOOM_CSS_CLASS_PREFIX))
            .map(|css_class| css_class.to_string())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn markdown_preview_has_base_css_class_for_tests(&self) -> bool {
        self.preview_view.has_css_class(MARKDOWN_PREVIEW_CSS_CLASS)
    }

    #[cfg(test)]
    pub(crate) fn select_markdown_preview_offsets_for_tests(&self, start: i32, end: i32) {
        let start_iter = self.preview_buffer.iter_at_offset(start);
        let end_iter = self.preview_buffer.iter_at_offset(end);
        self.preview_buffer.select_range(&start_iter, &end_iter);
    }

    #[cfg(test)]
    pub(crate) fn markdown_preview_scroll_value_for_tests(&self) -> f64 {
        self.preview_scrolled.vadjustment().value()
    }

    #[cfg(test)]
    pub(crate) fn set_markdown_preview_scroll_value_for_tests(&self, value: f64) {
        self.preview_scrolled.vadjustment().set_value(value);
    }

    #[cfg(test)]
    pub(crate) fn copy_markdown_preview_selection_for_tests(&self) -> bool {
        copy_markdown_preview_selection(
            &self.preview_buffer,
            &self.preview_view,
            &self.preview_scrolled,
        )
    }
}

fn add_copy_shortcut(controller: &gtk4::ShortcutController, trigger: &str) {
    let Some(trigger) = gtk4::ShortcutTrigger::parse_string(trigger) else {
        return;
    };
    controller.add_shortcut(gtk4::Shortcut::new(
        Some(trigger),
        Some(gtk4::SignalAction::new("copy-clipboard")),
    ));
}

fn copy_markdown_preview_selection(
    buffer: &gtk4::TextBuffer,
    view: &gtk4::TextView,
    scrolled: &gtk4::ScrolledWindow,
) -> bool {
    let Some((start, end)) = buffer.selection_bounds() else {
        return false;
    };
    if start.offset() == end.offset() {
        return false;
    }
    let scroll_value = scrolled.vadjustment().value();
    let text = buffer.text(&start, &end, true);
    view.display().clipboard().set_text(&text);
    restore_scroll_value(scrolled, scroll_value);
    true
}

fn restore_scroll_value(scrolled: &gtk4::ScrolledWindow, value: f64) {
    let adjustment = scrolled.vadjustment();
    let upper = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
    adjustment.set_value(value.clamp(adjustment.lower(), upper));
    let weak = scrolled.downgrade();
    glib::idle_add_local_once(move || {
        let Some(scrolled) = weak.upgrade() else {
            return;
        };
        let adjustment = scrolled.vadjustment();
        let upper = (adjustment.upper() - adjustment.page_size()).max(adjustment.lower());
        adjustment.set_value(value.clamp(adjustment.lower(), upper));
    });
}

fn single_line_buffer_selection_text(buffer: &gtk4::TextBuffer) -> Option<String> {
    let (start, end) = buffer.selection_bounds()?;
    if start.line() != end.line() || start.offset() == end.offset() {
        return None;
    }
    Some(String::from(buffer.text(&start, &end, true)))
}

fn selection_offsets(buffer: &gtk4::TextBuffer) -> Option<(i32, i32)> {
    buffer
        .selection_bounds()
        .map(|(start, end)| (start.offset(), end.offset()))
}

fn buffer_has_selection(buffer: &gtk4::TextBuffer) -> bool {
    selection_offsets(buffer).is_some_and(|(start, end)| start != end)
}

fn has_click_modifier(state: gdk::ModifierType) -> bool {
    state.intersects(
        gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK
            | gdk::ModifierType::META_MASK,
    )
}

fn markdown_link_is_launchable(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

fn markdown_preview_uses_fallback(len: usize) -> bool {
    u64::try_from(len).map_or(true, |len| {
        !crate::document_limits::markdown_preview_enabled(len)
    })
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
    use super::markdown_preview_uses_fallback;

    fn preview_limit() -> usize {
        usize::try_from(crate::document_limits::SMALL_FILE_LIMIT_BYTES)
            .map_or(usize::MAX, |value| value)
    }

    #[test]
    fn markdown_preview_at_minus_one_renders() {
        assert!(!markdown_preview_uses_fallback(preview_limit() - 1));
    }

    #[test]
    fn markdown_preview_at_exact_falls_back() {
        assert!(markdown_preview_uses_fallback(preview_limit()));
    }

    #[test]
    fn markdown_preview_at_plus_one_falls_back() {
        assert!(markdown_preview_uses_fallback(preview_limit() + 1));
    }
}
