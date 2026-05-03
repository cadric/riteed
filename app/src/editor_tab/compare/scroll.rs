use std::cell::Cell;
use std::rc::Rc;

use gtk4::{glib, prelude::*};

#[derive(Clone)]
pub(super) struct CompareScrollMarks {
    left: gtk4::TextMark,
    right: gtk4::TextMark,
}

impl CompareScrollMarks {
    pub(super) fn new(left: &sourceview5::Buffer, right: &sourceview5::Buffer) -> Self {
        Self {
            left: create_scroll_mark(left),
            right: create_scroll_mark(right),
        }
    }

    pub(super) fn scroll_to_row(
        &self,
        left_buffer: &sourceview5::Buffer,
        left_view: &sourceview5::View,
        right_buffer: &sourceview5::Buffer,
        right_view: &sourceview5::View,
        row: usize,
    ) {
        scroll_to_row(left_buffer, left_view, &self.left, row);
        scroll_to_row(right_buffer, right_view, &self.right, row);
    }

    pub(super) fn queue_scroll_to_row(
        &self,
        left_buffer: &sourceview5::Buffer,
        left_view: &sourceview5::View,
        right_buffer: &sourceview5::Buffer,
        right_view: &sourceview5::View,
        row: usize,
    ) {
        let marks = self.clone();
        let left_buffer = left_buffer.clone();
        let left_view = left_view.clone();
        let right_buffer = right_buffer.clone();
        let right_view = right_view.clone();
        let _source = glib::idle_add_local_once(move || {
            marks.scroll_to_row(&left_buffer, &left_view, &right_buffer, &right_view, row);
        });
    }
}

pub(super) fn install_scroll_sync(
    left: &gtk4::Adjustment,
    right: &gtk4::Adjustment,
) -> (glib::SignalHandlerId, glib::SignalHandlerId) {
    let syncing = Rc::new(Cell::new(false));
    let left_syncing = Rc::clone(&syncing);
    let right_target = right.downgrade();
    let left_handler = left.connect_value_changed(move |left| {
        sync_adjustment(left, &right_target, &left_syncing);
    });

    let left_target = left.downgrade();
    let right_handler = right.connect_value_changed(move |right| {
        sync_adjustment(right, &left_target, &syncing);
    });
    (left_handler, right_handler)
}

fn create_scroll_mark(buffer: &sourceview5::Buffer) -> gtk4::TextMark {
    let iter = buffer.start_iter();
    buffer.create_mark(None, &iter, true)
}

fn scroll_to_row(
    buffer: &sourceview5::Buffer,
    view: &sourceview5::View,
    mark: &gtk4::TextMark,
    row: usize,
) {
    let Some(iter) = buffer.iter_at_line(i32::try_from(row).map_or(0, |value| value)) else {
        return;
    };
    buffer.place_cursor(&iter);
    buffer.move_mark(mark, &iter);
    view.scroll_to_mark(mark, 0.0, true, 0.0, 0.0);
}

fn sync_adjustment(
    source: &gtk4::Adjustment,
    target: &glib::WeakRef<gtk4::Adjustment>,
    syncing: &Rc<Cell<bool>>,
) {
    if syncing.get() {
        return;
    }
    let Some(target) = target.upgrade() else {
        return;
    };
    let value = source
        .value()
        .clamp(target.lower(), adjustment_upper(&target));
    if (target.value() - value).abs() < 0.5 {
        return;
    }
    syncing.set(true);
    target.set_value(value);
    syncing.set(false);
}

fn adjustment_upper(adjustment: &gtk4::Adjustment) -> f64 {
    (adjustment.upper() - adjustment.page_size()).max(adjustment.lower())
}
