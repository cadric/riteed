use std::cell::Cell;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gdk, glib, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use super::diff::DiffPlan;
use crate::editor_tab::EditorTab;
use crate::editor_zoom::{EDITOR_VIEW_CSS_CLASS, resolve_scroll_past_end_padding};

pub(super) struct CompareTags {
    pub(super) editable_changed: gtk4::TextTag,
    pub(super) editable_current: gtk4::TextTag,
    pub(super) reference_changed: gtk4::TextTag,
    pub(super) reference_current: gtk4::TextTag,
}

impl CompareTags {
    pub(super) fn new(
        editable_buffer: &sourceview5::Buffer,
        reference_buffer: &sourceview5::Buffer,
    ) -> Self {
        let tags = Self {
            editable_changed: gtk4::TextTag::new(None),
            editable_current: gtk4::TextTag::new(None),
            reference_changed: gtk4::TextTag::new(None),
            reference_current: gtk4::TextTag::new(None),
        };
        let editable_table = editable_buffer.tag_table();
        let reference_table = reference_buffer.tag_table();
        let _added = editable_table.add(&tags.editable_changed);
        let _added = editable_table.add(&tags.editable_current);
        let _added = reference_table.add(&tags.reference_changed);
        let _added = reference_table.add(&tags.reference_current);
        tags
    }

    pub(super) fn apply_colors(&self) {
        let dark = adw::StyleManager::default().is_dark();
        self.editable_changed
            .set_background_rgba(Some(&compare_color(dark, CompareColor::Editable)));
        self.reference_changed
            .set_background_rgba(Some(&compare_color(dark, CompareColor::Reference)));
        self.editable_current
            .set_background_rgba(Some(&compare_color(dark, CompareColor::Current)));
        self.reference_current
            .set_background_rgba(Some(&compare_color(dark, CompareColor::Current)));
    }
}

pub(super) fn configure_reference_view(tab: &EditorTab, view: &sourceview5::View) {
    let padding = resolve_scroll_past_end_padding(&tab.settings.editor_font());
    view.set_accepts_tab(false);
    view.set_bottom_margin(padding);
    view.set_cursor_visible(false);
    view.set_editable(false);
    view.set_hexpand(true);
    view.set_left_margin(12);
    view.set_monospace(true);
    view.set_right_margin(12);
    view.set_show_line_numbers(tab.settings.show_line_numbers());
    view.set_top_margin(12);
    view.set_vexpand(true);
    view.add_css_class(EDITOR_VIEW_CSS_CLASS);
    tab.settings.apply_word_wrap(view);
    tab.settings.apply_indentation(view);
}

pub(super) fn compare_toolbar(reference_title: &str) -> gtk4::Box {
    let toolbar = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .margin_start(6)
        .margin_end(6)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    toolbar.set_hexpand(true);
    toolbar.append(
        &gtk4::Label::builder()
            .label(pgettext("compare toolbar", "Compare"))
            .build(),
    );
    let reference = gtk4::Label::builder()
        .label(reference_title)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .xalign(0.0)
        .build();
    reference.set_hexpand(true);
    toolbar.append(&reference);
    toolbar.append(&toolbar_button(
        "go-up-symbolic",
        &pgettext("compare action", "Previous Difference"),
        "win.diff-prev",
    ));
    toolbar.append(&toolbar_button(
        "go-down-symbolic",
        &pgettext("compare action", "Next Difference"),
        "win.diff-next",
    ));
    toolbar.append(&toolbar_button(
        "view-refresh-symbolic",
        &pgettext("compare action", "Refresh Reference"),
        "win.compare-refresh-reference",
    ));
    toolbar.append(&toolbar_button(
        "window-close-symbolic",
        &pgettext("compare action", "Exit Compare"),
        "win.compare-exit",
    ));
    let status = gtk4::Label::builder()
        .label(pgettext("compare status", "Loading Reference..."))
        .xalign(1.0)
        .build();
    status.add_css_class("dim-label");
    toolbar.append(&status);
    toolbar
}

pub(super) fn install_scroll_sync(
    left: &gtk4::Adjustment,
    right: &gtk4::Adjustment,
) -> (glib::SignalHandlerId, glib::SignalHandlerId) {
    let syncing = Rc::new(Cell::new(false));
    let right_for_left = right.clone();
    let syncing_for_left = Rc::clone(&syncing);
    let left_handler = left.connect_value_changed(move |left| {
        sync_adjustment_ratio(left, &right_for_left, &syncing_for_left);
    });
    let left_for_right = left.clone();
    let syncing_for_right = Rc::clone(&syncing);
    let right_handler = right.connect_value_changed(move |right| {
        sync_adjustment_ratio(right, &left_for_right, &syncing_for_right);
    });
    (left_handler, right_handler)
}

pub(super) fn apply_diff_tags(
    editable_buffer: &sourceview5::Buffer,
    reference_buffer: &sourceview5::Buffer,
    plan: &DiffPlan,
    tags: &CompareTags,
) {
    if plan.too_large {
        return;
    }
    for line in &plan.editable_lines {
        apply_line_tag(editable_buffer, *line, &tags.editable_changed);
    }
    for line in &plan.reference_lines {
        apply_line_tag(reference_buffer, *line, &tags.reference_changed);
    }
}

pub(super) fn apply_line_tag(buffer: &sourceview5::Buffer, line: usize, tag: &gtk4::TextTag) {
    let Some((start, end)) = line_bounds(buffer, line) else {
        return;
    };
    buffer.apply_tag(tag, &start, &end);
}

pub(super) fn clear_tags(
    editable_buffer: &sourceview5::Buffer,
    reference_buffer: &sourceview5::Buffer,
    tags: &CompareTags,
) {
    remove_buffer_tag(editable_buffer, &tags.editable_changed);
    remove_buffer_tag(editable_buffer, &tags.editable_current);
    remove_buffer_tag(reference_buffer, &tags.reference_changed);
    remove_buffer_tag(reference_buffer, &tags.reference_current);
}

pub(super) fn remove_current_tags(
    editable_buffer: &sourceview5::Buffer,
    reference_buffer: &sourceview5::Buffer,
    tags: &CompareTags,
) {
    remove_buffer_tag(editable_buffer, &tags.editable_current);
    remove_buffer_tag(reference_buffer, &tags.reference_current);
}

pub(super) fn scroll_to_line(buffer: &sourceview5::Buffer, view: &sourceview5::View, line: usize) {
    let Some((mut iter, _end)) = line_bounds(buffer, line) else {
        return;
    };
    let _scrolled = view.scroll_to_iter(&mut iter, 0.12, false, 0.0, 0.0);
}

pub(super) fn buffer_text(buffer: &sourceview5::Buffer) -> String {
    buffer
        .text(&buffer.start_iter(), &buffer.end_iter(), true)
        .to_string()
}

fn toolbar_button(icon_name: &str, tooltip: &str, action_name: &str) -> gtk4::Button {
    gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .action_name(action_name)
        .build()
}

fn sync_adjustment_ratio(
    source: &gtk4::Adjustment,
    target: &gtk4::Adjustment,
    syncing: &Cell<bool>,
) {
    if syncing.get() {
        return;
    }
    let source_max = (source.upper() - source.page_size()).max(0.0);
    let target_max = (target.upper() - target.page_size()).max(0.0);
    if source_max <= f64::EPSILON || target_max <= f64::EPSILON {
        return;
    }
    syncing.set(true);
    target.set_value((source.value() / source_max) * target_max);
    syncing.set(false);
}

fn remove_buffer_tag(buffer: &sourceview5::Buffer, tag: &gtk4::TextTag) {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.remove_tag(tag, &start, &end);
}

fn line_bounds(
    buffer: &sourceview5::Buffer,
    line: usize,
) -> Option<(gtk4::TextIter, gtk4::TextIter)> {
    let line = i32::try_from(line).ok()?;
    let start = buffer.iter_at_line(line)?;
    let mut end = start;
    if !end.forward_line() {
        end = buffer.end_iter();
    }
    Some((start, end))
}

enum CompareColor {
    Editable,
    Reference,
    Current,
}

fn compare_color(dark: bool, color: CompareColor) -> gdk::RGBA {
    match (dark, color) {
        (false, CompareColor::Editable) => gdk::RGBA::new(0.80, 0.94, 0.82, 1.0),
        (false, CompareColor::Reference) => gdk::RGBA::new(1.00, 0.86, 0.84, 1.0),
        (false, CompareColor::Current) => gdk::RGBA::new(0.80, 0.88, 1.00, 1.0),
        (true, CompareColor::Editable) => gdk::RGBA::new(0.12, 0.34, 0.20, 1.0),
        (true, CompareColor::Reference) => gdk::RGBA::new(0.42, 0.16, 0.14, 1.0),
        (true, CompareColor::Current) => gdk::RGBA::new(0.16, 0.24, 0.40, 1.0),
    }
}
