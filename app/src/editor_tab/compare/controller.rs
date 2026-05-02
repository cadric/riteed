use std::rc::Rc;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use super::diff::{DiffPlan, compute_diff_plan};
use super::ui::{
    CompareTags, apply_diff_tags, apply_line_tag, buffer_text, clear_tags, compare_toolbar,
    configure_reference_view, install_scroll_sync, remove_current_tags, scroll_to_line,
};
use super::{CompareController, CompareTarget};
use crate::editor_tab::EditorTab;
use crate::editor_zoom::{clear_zoom_css_classes, restore_zoom_css_class};

impl CompareController {
    pub(super) fn new(tab: &Rc<EditorTab>, target: CompareTarget) -> Self {
        let reference_buffer = sourceview5::Buffer::builder()
            .enable_undo(false)
            .implicit_trailing_newline(false)
            .build();
        tab.settings.apply_source_style_scheme(&reference_buffer);
        sync_reference_language(&tab.text_buffer, &reference_buffer);
        let reference_view = sourceview5::View::with_buffer(&reference_buffer);
        configure_reference_view(tab, &reference_view);
        let reference_scrolled = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&reference_view)
            .build();
        reference_scrolled.set_hexpand(true);
        reference_scrolled.set_vexpand(true);
        reference_scrolled.set_min_content_width(0);

        let toolbar = compare_toolbar(&target.title);
        let status_label = toolbar.status_label.clone();
        let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
        paned.set_resize_start_child(true);
        paned.set_shrink_start_child(false);
        paned.set_resize_end_child(true);
        paned.set_shrink_end_child(false);
        paned.set_end_child(Some(&reference_scrolled));
        paned.set_hexpand(true);
        paned.set_vexpand(true);

        let left_adjustment = tab.scrolled.vadjustment();
        let right_adjustment = reference_scrolled.vadjustment();
        let scroll_anchors = Rc::new(std::cell::RefCell::new(Vec::new()));
        let (left_handler, right_handler) = install_scroll_sync(
            &left_adjustment,
            &right_adjustment,
            &tab.text_view,
            &reference_view,
            &scroll_anchors,
        );
        let style_manager = adw::StyleManager::default();
        let weak = Rc::downgrade(tab);
        let style_handler = style_manager.connect_dark_notify(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.apply_compare_style();
            }
        });
        let weak = Rc::downgrade(tab);
        let high_contrast_handler = style_manager.connect_high_contrast_notify(move |_| {
            if let Some(tab) = weak.upgrade() {
                tab.apply_compare_style();
            }
        });

        Self {
            target,
            toolbar: toolbar.root,
            status_label,
            paned,
            reference_view,
            reference_buffer: reference_buffer.clone(),
            tags: CompareTags::new(&tab.text_buffer, &reference_buffer),
            diff_plan: DiffPlan::empty(),
            current_hunk: None,
            cancellable: None,
            left_adjustment,
            right_adjustment,
            scroll_anchors,
            left_handler: Some(left_handler),
            right_handler: Some(right_handler),
            style_manager,
            style_handler: Some(style_handler),
            high_contrast_handler: Some(high_contrast_handler),
        }
    }

    pub(super) fn set_loading(&mut self, cancellable: &gio::Cancellable) {
        if let Some(previous) = self.cancellable.take() {
            previous.cancel();
        }
        self.cancellable = Some(cancellable.clone());
        self.status_label.set_label(&ellipsis_label(pgettext(
            "compare status",
            "Loading Reference",
        )));
    }

    pub(super) fn finish_loading(&mut self) {
        self.cancellable = None;
    }

    pub(super) fn cancel(&mut self) {
        if let Some(cancellable) = self.cancellable.take() {
            cancellable.cancel();
        }
    }

    pub(super) fn set_reference_text(&self, text: &str, implicit_trailing_newline: bool) {
        self.reference_buffer
            .set_implicit_trailing_newline(implicit_trailing_newline);
        self.reference_buffer.set_text(text);
        self.reference_buffer.set_modified(false);
    }

    pub(super) fn recompute(
        &mut self,
        editable_buffer: &sourceview5::Buffer,
        editable_view: &sourceview5::View,
        editable_text: &str,
    ) {
        clear_tags(editable_buffer, &self.reference_buffer, &self.tags);
        let reference_text = buffer_text(&self.reference_buffer);
        let previous = self.current_hunk;
        self.diff_plan = compute_diff_plan(editable_text, &reference_text);
        self.scroll_anchors
            .borrow_mut()
            .clone_from(&self.diff_plan.anchors);
        self.current_hunk = if self.diff_plan.hunks.is_empty() || self.diff_plan.too_large {
            None
        } else {
            Some(previous.unwrap_or(0).min(self.diff_plan.hunks.len() - 1))
        };
        apply_diff_tags(
            editable_buffer,
            &self.reference_buffer,
            &self.diff_plan,
            &self.tags,
        );
        self.apply_current_hunk(editable_buffer);
        self.update_status();
        if self.current_hunk == Some(0) {
            self.scroll_current_hunk(editable_buffer, editable_view);
        }
    }

    pub(super) fn move_hunk(
        &mut self,
        editable_buffer: &sourceview5::Buffer,
        editable_view: &sourceview5::View,
        direction: i32,
    ) {
        if self.diff_plan.hunks.is_empty() || self.diff_plan.too_large {
            return;
        }
        let len = self.diff_plan.hunks.len();
        let current = self.current_hunk.unwrap_or(0);
        let next = if direction < 0 {
            current.checked_sub(1).unwrap_or(len - 1)
        } else {
            (current + 1) % len
        };
        self.current_hunk = Some(next);
        self.apply_current_hunk(editable_buffer);
        self.update_status();
        self.scroll_current_hunk(editable_buffer, editable_view);
    }

    pub(super) fn apply_current_hunk(&self, editable_buffer: &sourceview5::Buffer) {
        remove_current_tags(editable_buffer, &self.reference_buffer, &self.tags);
        let Some(index) = self.current_hunk else {
            return;
        };
        let Some(hunk) = self.diff_plan.hunks.get(index) else {
            return;
        };
        for line in &hunk.editable_lines {
            apply_line_tag(editable_buffer, *line, &self.tags.editable_current);
        }
        for line in &hunk.reference_lines {
            apply_line_tag(&self.reference_buffer, *line, &self.tags.reference_current);
        }
    }

    pub(super) fn apply_tag_colors(&self, dark: bool, high_contrast: bool) {
        self.tags.apply_colors(dark, high_contrast);
    }

    pub(super) fn clear_zoom_style(&self) {
        clear_zoom_css_classes(&self.reference_view);
    }

    pub(super) fn restore_zoom_style(&self, css_class: &str) {
        restore_zoom_css_class(&self.reference_view, css_class);
    }

    fn scroll_current_hunk(
        &self,
        editable_buffer: &sourceview5::Buffer,
        editable_view: &sourceview5::View,
    ) {
        let Some(index) = self.current_hunk else {
            return;
        };
        let Some(hunk) = self.diff_plan.hunks.get(index) else {
            return;
        };
        if let Some(line) = hunk.editable_lines.first() {
            scroll_to_line(editable_buffer, editable_view, *line);
        }
        if let Some(line) = hunk.reference_lines.first() {
            scroll_to_line(&self.reference_buffer, &self.reference_view, *line);
        }
    }

    fn update_status(&self) {
        if self.diff_plan.too_large {
            self.status_label
                .set_label(&gettext("Too large to compare differences."));
            return;
        }
        let hunk_count = self.diff_plan.hunks.len();
        if hunk_count == 0 {
            self.status_label
                .set_label(&gettext("No differences were found."));
            return;
        }
        let changed_lines = self.diff_plan.changed_line_count();
        let plural_count = u32::try_from(changed_lines).map_or(u32::MAX, |value| value);
        let text = ngettext("%d changed line", "%d changed lines", plural_count)
            .replace("%d", &changed_lines.to_string());
        if let Some(current) = self.current_hunk {
            self.status_label
                .set_label(&format!("{} · {}/{}", text, current + 1, hunk_count));
        } else {
            self.status_label.set_label(&text);
        }
    }
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

pub(super) fn sync_reference_language(
    editable_buffer: &sourceview5::Buffer,
    reference_buffer: &sourceview5::Buffer,
) {
    let language = editable_buffer.language();
    reference_buffer.set_language(language.as_ref());
    reference_buffer.set_highlight_syntax(language.is_some());
}

impl Drop for CompareController {
    fn drop(&mut self) {
        self.cancel();
        if let Some(handler) = self.left_handler.take() {
            self.left_adjustment.disconnect(handler);
        }
        if let Some(handler) = self.right_handler.take() {
            self.right_adjustment.disconnect(handler);
        }
        if let Some(handler) = self.style_handler.take() {
            self.style_manager.disconnect(handler);
        }
        if let Some(handler) = self.high_contrast_handler.take() {
            self.style_manager.disconnect(handler);
        }
    }
}
