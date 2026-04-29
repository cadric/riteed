use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::accessible::Property;
use gtk4::{gdk, glib, prelude::*};
use sourceview5::prelude::*;

use super::diff::{CompareLineAnchor, DiffPlan, map_line_with_anchors};
use crate::editor_tab::EditorTab;
use crate::editor_zoom::{
    EDITOR_VIEW_CSS_CLASS, copy_zoom_css_classes, resolve_scroll_past_end_padding,
};

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

    pub(super) fn apply_colors(&self, dark: bool) {
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
    view.set_highlight_current_line(false);
    view.set_left_margin(12);
    view.set_monospace(true);
    view.set_right_margin(12);
    view.set_show_line_numbers(tab.settings.show_line_numbers());
    view.set_top_margin(12);
    view.set_vexpand(true);
    view.add_css_class(EDITOR_VIEW_CSS_CLASS);
    copy_zoom_css_classes(&tab.text_view, view);
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
        .label(ellipsis_label(pgettext(
            "compare status",
            "Loading Reference",
        )))
        .xalign(1.0)
        .build();
    status.add_css_class("dim-label");
    toolbar.append(&status);
    toolbar
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

pub(super) fn install_scroll_sync(
    left: &gtk4::Adjustment,
    right: &gtk4::Adjustment,
    left_view: &sourceview5::View,
    right_view: &sourceview5::View,
    anchors: &Rc<RefCell<Vec<CompareLineAnchor>>>,
) -> (glib::SignalHandlerId, glib::SignalHandlerId) {
    let syncing = Rc::new(Cell::new(false));
    let idle_redraw_pending = Rc::new(Cell::new(false));

    let left_view = left_view.downgrade();
    let right_view = right_view.downgrade();

    let right_for_left = right.downgrade();
    let idle_for_left = Rc::clone(&idle_redraw_pending);
    let left_view_for_left = left_view.clone();
    let right_view_for_left = right_view.clone();
    let left_sync = AnchorSync {
        target: right_for_left,
        source_view: left_view_for_left.clone(),
        target_view: right_view_for_left.clone(),
        anchors: Rc::clone(anchors),
        from_editable: true,
        syncing: Rc::clone(&syncing),
    };
    let left_handler = left.connect_value_changed(move |left| {
        if left_sync.syncing.get() {
            return;
        }
        left_sync.sync(left);
        queue_compare_redraw(&left_view_for_left, &right_view_for_left, &idle_for_left);
    });

    let left_for_right = left.downgrade();
    let idle_for_right = Rc::clone(&idle_redraw_pending);
    let left_view_for_right = left_view.clone();
    let right_view_for_right = right_view.clone();
    let right_sync = AnchorSync {
        target: left_for_right,
        source_view: right_view_for_right.clone(),
        target_view: left_view_for_right.clone(),
        anchors: Rc::clone(anchors),
        from_editable: false,
        syncing,
    };
    let right_handler = right.connect_value_changed(move |right| {
        if right_sync.syncing.get() {
            return;
        }
        right_sync.sync(right);
        queue_compare_redraw(&left_view_for_right, &right_view_for_right, &idle_for_right);
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
    let button = gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .action_name(action_name)
        .build();
    button.update_property(&[Property::Label(tooltip)]);
    button
}

struct AnchorSync {
    target: glib::WeakRef<gtk4::Adjustment>,
    source_view: glib::WeakRef<sourceview5::View>,
    target_view: glib::WeakRef<sourceview5::View>,
    anchors: Rc<RefCell<Vec<CompareLineAnchor>>>,
    from_editable: bool,
    syncing: Rc<Cell<bool>>,
}

impl AnchorSync {
    fn sync(&self, source: &gtk4::Adjustment) {
        let Some(target) = self.target.upgrade() else {
            return;
        };
        let value = anchor_adjustment_value(
            source,
            &target,
            &self.source_view,
            &self.target_view,
            &self.anchors.borrow(),
            self.from_editable,
        )
        .unwrap_or_else(|| proportional_adjustment_value(source, &target));
        if (target.value() - value).abs() < 0.5 {
            return;
        }
        self.syncing.set(true);
        target.set_value(value);
        self.syncing.set(false);
    }
}

fn anchor_adjustment_value(
    _source: &gtk4::Adjustment,
    target: &gtk4::Adjustment,
    source_view: &glib::WeakRef<sourceview5::View>,
    target_view: &glib::WeakRef<sourceview5::View>,
    anchors: &[CompareLineAnchor],
    from_editable: bool,
) -> Option<f64> {
    let source_view = source_view.upgrade()?;
    let target_view = target_view.upgrade()?;
    let (source_line, line_offset) = first_visible_line_offset(&source_view)?;
    let target_buffer = target_view.buffer();
    let target_line_count = usize::try_from(target_buffer.line_count()).ok()?;
    let target_line = map_line_with_anchors(source_line, anchors, from_editable, target_line_count);
    let iter = target_buffer.iter_at_line(i32::try_from(target_line).ok()?)?;
    let (line_y, _height) = target_view.line_yrange(&iter);
    let target_upper = adjustment_upper(target);
    let value = (f64::from(line_y) + f64::from(line_offset)).clamp(target.lower(), target_upper);
    Some(value.round())
}

fn first_visible_line_offset(view: &sourceview5::View) -> Option<(usize, i32)> {
    let visible = view.visible_rect();
    let iter = view.iter_at_location(0, visible.y())?;
    let (line_y, _height) = view.line_yrange(&iter);
    let offset = visible.y().saturating_sub(line_y).max(0);
    Some((usize::try_from(iter.line()).ok()?, offset))
}

fn proportional_adjustment_value(source: &gtk4::Adjustment, target: &gtk4::Adjustment) -> f64 {
    proportional_value(
        source.value(),
        source.lower(),
        adjustment_upper(source),
        target.lower(),
        adjustment_upper(target),
    )
}

fn proportional_value(
    source_value: f64,
    source_lower: f64,
    source_upper: f64,
    target_lower: f64,
    target_upper: f64,
) -> f64 {
    let source_range = source_upper - source_lower;
    if source_range <= f64::EPSILON {
        return target_lower;
    }
    let ratio = ((source_value - source_lower) / source_range).clamp(0.0, 1.0);
    (target_lower + ((target_upper - target_lower) * ratio)).round()
}

fn adjustment_upper(adjustment: &gtk4::Adjustment) -> f64 {
    (adjustment.upper() - adjustment.page_size()).max(adjustment.lower())
}

fn queue_compare_redraw(
    left_view: &glib::WeakRef<sourceview5::View>,
    right_view: &glib::WeakRef<sourceview5::View>,
    idle_pending: &Rc<Cell<bool>>,
) {
    let Some(left_view) = left_view.upgrade() else {
        return;
    };
    let Some(right_view) = right_view.upgrade() else {
        return;
    };
    left_view.queue_draw();
    right_view.queue_draw();

    if idle_pending.replace(true) {
        return;
    }

    let left_view = left_view.downgrade();
    let right_view = right_view.downgrade();
    let idle_pending = Rc::clone(idle_pending);
    glib::idle_add_local_once(move || {
        idle_pending.set(false);
        let Some(left_view) = left_view.upgrade() else {
            return;
        };
        let Some(right_view) = right_view.upgrade() else {
            return;
        };
        left_view.queue_draw();
        right_view.queue_draw();
    });
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

#[cfg(test)]
mod tests {
    use super::proportional_value;

    #[test]
    fn proportional_fallback_preserves_scroll_region() {
        let value = proportional_value(50.0, 0.0, 100.0, 0.0, 400.0);
        assert!((value - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn proportional_fallback_handles_empty_source_range() {
        let value = proportional_value(0.0, 0.0, 0.0, 20.0, 400.0);
        assert!((value - 20.0).abs() < f64::EPSILON);
    }
}
