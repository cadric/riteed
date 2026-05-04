use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use super::diff::compute_diff;
use super::gutter::CompareGutters;
use super::hatch::{CompareHatchEndpoint, CompareHatches};
use super::interaction::install_presentation_interaction;
use super::model::DiffRowModel;
use super::navigation::{target_hunk_for_navigation, top_visible_row};
use super::presentation::{DiffPresentation, PresentationSide};
use super::render::{
    CompareTags, apply_current_hunk_tags, apply_model_tags, apply_placeholder_tags,
    apply_presentation, clear_tags,
};
use super::scroll::{CompareScrollEndpoint, install_scroll_sync};
use super::ui::{compare_toolbar, configure_presentation_view};
use super::{CompareController, CompareTarget};
use crate::editor_tab::EditorTab;
use crate::editor_zoom::{clear_zoom_css_classes, restore_zoom_css_class};

impl CompareController {
    pub(super) fn new(tab: &Rc<EditorTab>, target: CompareTarget) -> Self {
        let row_model = Rc::new(RefCell::new(DiffRowModel::empty()));
        let presentation = Rc::new(RefCell::new(DiffPresentation::empty()));
        let left = build_presentation_pane(
            tab,
            &pgettext("compare pane", "Reference"),
            &presentation,
            PresentationSide::Reference,
        );
        let right = build_presentation_pane(
            tab,
            &pgettext("compare pane", "Current"),
            &presentation,
            PresentationSide::Current,
        );
        let gutters = CompareGutters::new(&left.view, &right.view, &presentation, &row_model);
        let toolbar = compare_toolbar(&target.title);
        let status_label = toolbar.status_label.clone();
        let paned = build_compare_paned(&left.root, &right.root);
        let scroll_sync = install_scroll_sync(
            CompareScrollEndpoint {
                adjustment: &left.scrolled.vadjustment(),
                buffer: &left.buffer,
                view: &left.view,
            },
            CompareScrollEndpoint {
                adjustment: &right.scrolled.vadjustment(),
                buffer: &right.buffer,
                view: &right.view,
            },
            &row_model,
        );
        let hatches = CompareHatches::new(
            CompareHatchEndpoint {
                view: &left.view,
                vadjustment: &left.scrolled.vadjustment(),
                hadjustment: &left.scrolled.hadjustment(),
            },
            CompareHatchEndpoint {
                view: &right.view,
                vadjustment: &right.scrolled.vadjustment(),
                hadjustment: &right.scrolled.hadjustment(),
            },
            &presentation,
        );
        let style_handlers = connect_style_handlers(tab);

        Self {
            target,
            toolbar: toolbar.root,
            status_label,
            paned,
            editable_snapshot: tab.buffer_text(),
            reference_text: String::new(),
            left_view: left.view,
            left_buffer: left.buffer.clone(),
            right_view: right.view,
            right_buffer: right.buffer.clone(),
            tags: CompareTags::new(&left.buffer, &right.buffer),
            presentation,
            row_model,
            gutters,
            hatches,
            scroll_sync,
            current_hunk: None,
            cancellable: None,
            style_manager: style_handlers.manager,
            style_handler: Some(style_handlers.style_handler),
            high_contrast_handler: Some(style_handlers.high_contrast_handler),
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

    pub(super) fn set_reference_text(&mut self, text: &str, _implicit_trailing_newline: bool) {
        self.reference_text.clear();
        self.reference_text.push_str(text);
    }

    pub(super) fn recompute(&mut self) -> Option<usize> {
        clear_tags(&self.left_buffer, &self.right_buffer, &self.tags);
        let previous = self.current_hunk;
        let computation = compute_diff(&self.reference_text, &self.editable_snapshot);
        let model = computation.model;
        let presentation = computation.presentation;
        let hunk_count = model.hunks.len();
        let too_large = model.too_large;
        apply_presentation(&self.left_buffer, &self.right_buffer, &presentation);
        self.current_hunk = current_hunk_after_recompute(previous, hunk_count, too_large);
        self.row_model.borrow_mut().clone_from(&model);
        self.presentation.borrow_mut().clone_from(&presentation);
        self.gutters.refresh();
        self.hatches.refresh();
        apply_model_tags(&self.left_buffer, &self.right_buffer, &model, &self.tags);
        apply_placeholder_tags(
            &self.left_buffer,
            &self.right_buffer,
            &presentation,
            &self.tags,
        );
        self.apply_current_hunk();
        self.update_status();
        if self.current_hunk == Some(0) {
            return self.current_hunk_row();
        }
        None
    }

    pub(super) fn move_hunk(&mut self, direction: i32) {
        let model = self.row_model.borrow();
        if model.hunks.is_empty() || model.too_large {
            return;
        }
        let top_row = top_visible_row(&self.left_view, model.rows.len());
        let base_row = self
            .current_hunk
            .and_then(|index| model.hunks.get(index))
            .map(|hunk| hunk.first_row)
            .filter(|first_row| *first_row >= top_row)
            .unwrap_or(top_row);
        let Some(next) = target_hunk_for_navigation(&model, base_row, direction) else {
            return;
        };
        drop(model);
        self.current_hunk = Some(next);
        self.apply_current_hunk();
        self.update_status();
        let _scrolled = self.scroll_current_hunk();
    }

    pub(super) fn apply_current_hunk(&self) {
        apply_current_hunk_tags(
            &self.left_buffer,
            &self.right_buffer,
            &self.row_model.borrow(),
            self.current_hunk,
            &self.tags,
        );
    }

    pub(super) fn apply_tag_colors(&self) {
        self.tags.apply_colors(&self.left_view);
        self.hatches.refresh_style();
    }

    pub(crate) fn apply_wrap_override(&self) {
        self.left_view.set_wrap_mode(gtk4::WrapMode::None);
        self.right_view.set_wrap_mode(gtk4::WrapMode::None);
        self.left_view.set_show_line_numbers(false);
        self.right_view.set_show_line_numbers(false);
        self.left_view.set_show_line_marks(false);
        self.right_view.set_show_line_marks(false);
        self.hatches.refresh();
    }

    pub(super) fn clear_zoom_style(&self) {
        clear_zoom_css_classes(&self.left_view);
        clear_zoom_css_classes(&self.right_view);
        self.gutters.refresh();
        self.hatches.refresh();
    }

    pub(super) fn restore_zoom_style(&self, css_class: &str) {
        restore_zoom_css_class(&self.left_view, css_class);
        restore_zoom_css_class(&self.right_view, css_class);
        self.gutters.refresh();
        self.hatches.refresh();
    }

    pub(super) fn detach_visual_layers(&mut self) {
        self.hatches.detach();
    }

    pub(super) fn scroll_to_row(&self, row: usize) -> bool {
        self.scroll_sync.scroll_to_row(row)
    }

    fn scroll_current_hunk(&self) -> bool {
        self.current_hunk_row()
            .is_some_and(|row| self.scroll_to_row(row))
    }

    fn current_hunk_row(&self) -> Option<usize> {
        let index = self.current_hunk?;
        let model = self.row_model.borrow();
        let hunk = model.hunks.get(index)?;
        Some(hunk.first_row)
    }

    fn update_status(&self) {
        let model = self.row_model.borrow();
        if model.too_large {
            self.status_label
                .set_label(&gettext("Too large to compare differences."));
            return;
        }
        let hunk_count = model.hunks.len();
        if hunk_count == 0 {
            self.status_label
                .set_label(&gettext("No differences were found."));
            return;
        }
        let changed_lines = model.changed_row_count();
        self.status_label.set_label(&compare_status_text(
            changed_lines,
            self.current_hunk,
            hunk_count,
        ));
    }
}

fn compare_status_text(
    changed_lines: usize,
    current_hunk: Option<usize>,
    hunk_count: usize,
) -> String {
    let plural_count = u32::try_from(changed_lines).map_or(u32::MAX, |value| value);
    let changed_lines = changed_lines.to_string();
    if let Some(current) = current_hunk {
        return ngettext(
            "%1$d changed line - %2$d/%3$d",
            "%1$d changed lines - %2$d/%3$d",
            plural_count,
        )
        .replace("%1$d", &changed_lines)
        .replace("%2$d", &(current + 1).to_string())
        .replace("%3$d", &hunk_count.to_string());
    }
    ngettext("%d changed line", "%d changed lines", plural_count).replace("%d", &changed_lines)
}

struct PresentationPane {
    root: gtk4::Box,
    buffer: sourceview5::Buffer,
    view: sourceview5::View,
    scrolled: gtk4::ScrolledWindow,
}

struct StyleHandlers {
    manager: adw::StyleManager,
    style_handler: gtk4::glib::SignalHandlerId,
    high_contrast_handler: gtk4::glib::SignalHandlerId,
}

fn build_presentation_pane(
    tab: &EditorTab,
    title: &str,
    presentation: &Rc<RefCell<DiffPresentation>>,
    side: PresentationSide,
) -> PresentationPane {
    let buffer = sourceview5::Buffer::builder()
        .enable_undo(false)
        .implicit_trailing_newline(false)
        .build();
    tab.settings.apply_source_style_scheme(&buffer);
    sync_reference_language(&tab.text_buffer, &buffer);
    let view = sourceview5::View::with_buffer(&buffer);
    configure_presentation_view(tab, &view);
    install_presentation_interaction(&view, &buffer, presentation, side);
    let scrolled = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Automatic)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .child(&view)
        .build();
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);
    scrolled.set_min_content_width(0);
    let label = gtk4::Label::builder()
        .label(title)
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_bottom(4)
        .build();
    label.add_css_class("dim-label");
    let root = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();
    root.append(&label);
    root.append(&scrolled);
    PresentationPane {
        root,
        buffer,
        view,
        scrolled,
    }
}

fn build_compare_paned(
    left: &impl IsA<gtk4::Widget>,
    right: &impl IsA<gtk4::Widget>,
) -> gtk4::Paned {
    let paned = gtk4::Paned::new(gtk4::Orientation::Horizontal);
    paned.set_resize_start_child(true);
    paned.set_shrink_start_child(false);
    paned.set_resize_end_child(true);
    paned.set_shrink_end_child(false);
    paned.set_start_child(Some(left));
    paned.set_end_child(Some(right));
    paned.set_hexpand(true);
    paned.set_vexpand(true);
    paned
}

fn connect_style_handlers(tab: &Rc<EditorTab>) -> StyleHandlers {
    let manager = adw::StyleManager::default();
    let weak = Rc::downgrade(tab);
    let style_handler = manager.connect_dark_notify(move |_| {
        if let Some(tab) = weak.upgrade() {
            tab.apply_compare_style();
        }
    });
    let weak = Rc::downgrade(tab);
    let high_contrast_handler = manager.connect_high_contrast_notify(move |_| {
        if let Some(tab) = weak.upgrade() {
            tab.apply_compare_style();
        }
    });
    StyleHandlers {
        manager,
        style_handler,
        high_contrast_handler,
    }
}

fn current_hunk_after_recompute(
    previous: Option<usize>,
    hunk_count: usize,
    too_large: bool,
) -> Option<usize> {
    if hunk_count == 0 || too_large {
        return None;
    }
    let previous = previous.map_or(0, |previous| previous);
    Some(previous.min(hunk_count - 1))
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
        self.detach_visual_layers();
        self.cancel();
        self.scroll_sync.disconnect();
        if let Some(handler) = self.style_handler.take() {
            self.style_manager.disconnect(handler);
        }
        if let Some(handler) = self.high_contrast_handler.take() {
            self.style_manager.disconnect(handler);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::compare_status_text;

    #[test]
    fn compare_status_text_keeps_hunk_position_translatable() {
        assert_eq!(compare_status_text(1, None, 2), "1 changed line");
        assert_eq!(compare_status_text(2, None, 2), "2 changed lines");
        assert_eq!(compare_status_text(1, Some(0), 2), "1 changed line - 1/2");
        assert_eq!(compare_status_text(2, Some(1), 2), "2 changed lines - 2/2");
    }
}
