#[cfg(test)]
use std::cell::Cell;
use std::cell::{Ref, RefCell};
use std::rc::Rc;

use gtk4::{glib, prelude::*};

use super::model::DiffRowModel;

const OFFSET_TOLERANCE: f64 = 2.0;

// Compare panes use real blank presentation rows for placeholders, so vertical
// sync preserves both logical row and intra-row pixel offset. All coordinates
// used here are GtkTextView buffer coordinates; horizontal scrolling is
// intentionally independent.
pub(super) struct CompareScrollSync {
    left: CompareScrollPane,
    right: CompareScrollPane,
    row_model: Rc<RefCell<DiffRowModel>>,
    left_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>,
    right_handler: Rc<RefCell<Option<glib::SignalHandlerId>>>,
    #[cfg(test)]
    event_counts: Rc<CompareScrollEventCounts>,
}

#[derive(Clone, Copy)]
pub(super) struct CompareScrollEndpoint<'a> {
    pub(super) adjustment: &'a gtk4::Adjustment,
    pub(super) buffer: &'a sourceview5::Buffer,
    pub(super) view: &'a sourceview5::View,
}

#[derive(Clone)]
struct CompareScrollPane {
    adjustment: gtk4::Adjustment,
    buffer: sourceview5::Buffer,
    view: sourceview5::View,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ViewportPosition {
    pub(super) row: usize,
    pub(super) offset: f64,
}

#[cfg(test)]
struct CompareScrollEventCounts {
    left: Cell<usize>,
    right: Cell<usize>,
}

pub(super) fn install_scroll_sync(
    left_endpoint: CompareScrollEndpoint<'_>,
    right_endpoint: CompareScrollEndpoint<'_>,
    row_model: &Rc<RefCell<DiffRowModel>>,
) -> CompareScrollSync {
    let left = CompareScrollPane::new(left_endpoint);
    let right = CompareScrollPane::new(right_endpoint);
    let left_handler = Rc::new(RefCell::new(None));
    let right_handler = Rc::new(RefCell::new(None));
    #[cfg(test)]
    let event_counts = Rc::new(CompareScrollEventCounts {
        left: Cell::new(0),
        right: Cell::new(0),
    });

    let left_source = left.clone();
    let left_target = right.clone();
    let left_row_model = Rc::clone(row_model);
    let left_target_handler = Rc::clone(&right_handler);
    #[cfg(test)]
    let left_counts = Rc::clone(&event_counts);
    let connected_left_handler = left.adjustment.connect_value_changed(move |_| {
        #[cfg(test)]
        left_counts.left.set(left_counts.left.get() + 1);
        sync_position(
            &left_source,
            &left_target,
            &left_row_model,
            &left_target_handler,
        );
    });
    *left_handler.borrow_mut() = Some(connected_left_handler);

    let right_source = right.clone();
    let right_target = left.clone();
    let right_row_model = Rc::clone(row_model);
    let right_target_handler = Rc::clone(&left_handler);
    #[cfg(test)]
    let right_counts = Rc::clone(&event_counts);
    let connected_right_handler = right.adjustment.connect_value_changed(move |_| {
        #[cfg(test)]
        right_counts.right.set(right_counts.right.get() + 1);
        sync_position(
            &right_source,
            &right_target,
            &right_row_model,
            &right_target_handler,
        );
    });
    *right_handler.borrow_mut() = Some(connected_right_handler);

    CompareScrollSync {
        left,
        right,
        row_model: Rc::clone(row_model),
        left_handler,
        right_handler,
        #[cfg(test)]
        event_counts,
    }
}

fn sync_position(
    source: &CompareScrollPane,
    target: &CompareScrollPane,
    row_model: &Rc<RefCell<DiffRowModel>>,
    target_handler: &Rc<RefCell<Option<glib::SignalHandlerId>>>,
) {
    let row_count = row_model.borrow().rows.len();
    if row_count == 0 {
        return;
    }
    let Some(source_position) = viewport_position(&source.view, row_count) else {
        return;
    };
    if position_matches(&target.view, row_count, source_position) {
        return;
    }
    let _blocked = ScopedSignalBlock::new(&target.adjustment, target_handler);
    let _scrolled = scroll_pane_to_position(target, source_position.row, source_position.offset);
}

fn position_matches(
    view: &sourceview5::View,
    row_count: usize,
    source_position: ViewportPosition,
) -> bool {
    viewport_position(view, row_count).is_some_and(|target_position| {
        target_position.row == source_position.row
            && (target_position.offset - source_position.offset).abs() <= OFFSET_TOLERANCE
    })
}

fn viewport_position(view: &sourceview5::View, row_count: usize) -> Option<ViewportPosition> {
    if row_count == 0 {
        return None;
    }
    let visible_y = view.visible_rect().y();
    let (iter, line_top) = view.line_at_y(visible_y);
    let row = usize::try_from(iter.line())
        .map_or(0, |row| row)
        .min(row_count.saturating_sub(1));
    let offset = f64::from(visible_y.saturating_sub(line_top)).max(0.0);
    Some(ViewportPosition { row, offset })
}

fn scroll_pane_to_position(pane: &CompareScrollPane, row: usize, offset: f64) -> bool {
    let Some(iter) = pane
        .buffer
        .iter_at_line(i32::try_from(row).map_or(0, |value| value))
    else {
        return false;
    };
    let (line_y, line_height) = pane.view.line_yrange(&iter);
    if line_height <= 0 {
        return false;
    }
    let max_offset = f64::from(line_height.saturating_sub(1));
    let target_buffer_y = f64::from(line_y) + offset.clamp(0.0, max_offset);
    let buffer_adjustment_delta = f64::from(pane.view.visible_rect().y()) - pane.adjustment.value();
    let target = target_buffer_y - buffer_adjustment_delta;
    let lower = pane.adjustment.lower();
    let upper = adjustment_upper(&pane.adjustment);
    if row > 0 && upper <= lower {
        return false;
    }
    pane.adjustment.set_value(target.clamp(lower, upper));
    true
}

fn adjustment_upper(adjustment: &gtk4::Adjustment) -> f64 {
    (adjustment.upper() - adjustment.page_size()).max(adjustment.lower())
}

struct ScopedSignalBlock<'a> {
    adjustment: &'a gtk4::Adjustment,
    handler: Option<Ref<'a, glib::SignalHandlerId>>,
}

impl<'a> ScopedSignalBlock<'a> {
    fn new(
        adjustment: &'a gtk4::Adjustment,
        handler: &'a RefCell<Option<glib::SignalHandlerId>>,
    ) -> Self {
        let handler = Ref::filter_map(handler.borrow(), Option::as_ref).ok();
        if let Some(handler) = handler.as_ref() {
            adjustment.block_signal(handler);
        }
        Self {
            adjustment,
            handler,
        }
    }
}

impl Drop for ScopedSignalBlock<'_> {
    fn drop(&mut self) {
        if let Some(handler) = self.handler.as_ref() {
            self.adjustment.unblock_signal(handler);
        }
    }
}

impl CompareScrollPane {
    fn new(endpoint: CompareScrollEndpoint<'_>) -> Self {
        Self {
            adjustment: endpoint.adjustment.clone(),
            buffer: endpoint.buffer.clone(),
            view: endpoint.view.clone(),
        }
    }
}

impl CompareScrollSync {
    pub(super) fn scroll_to_row(&self, row: usize) -> bool {
        let _left_block = ScopedSignalBlock::new(&self.left.adjustment, &self.left_handler);
        let _right_block = ScopedSignalBlock::new(&self.right.adjustment, &self.right_handler);
        let left_scrolled = scroll_pane_to_position(&self.left, row, 0.0);
        let right_scrolled = scroll_pane_to_position(&self.right, row, 0.0);
        if !left_scrolled || !right_scrolled {
            return false;
        }
        let row_count = self.row_model.borrow().rows.len();
        let target_position = ViewportPosition { row, offset: 0.0 };
        position_matches(&self.left.view, row_count, target_position)
            && position_matches(&self.right.view, row_count, target_position)
    }

    pub(super) fn disconnect(&mut self) {
        if let Some(handler) = self.left_handler.borrow_mut().take() {
            self.left.adjustment.disconnect(handler);
        }
        if let Some(handler) = self.right_handler.borrow_mut().take() {
            self.right.adjustment.disconnect(handler);
        }
    }

    #[cfg(test)]
    pub(super) fn viewport_positions_for_tests(
        &self,
        row_count: usize,
    ) -> (Option<ViewportPosition>, Option<ViewportPosition>) {
        (
            viewport_position(&self.left.view, row_count),
            viewport_position(&self.right.view, row_count),
        )
    }

    #[cfg(test)]
    pub(super) fn scroll_left_to_row_offset_for_tests(&self, row: usize, offset: f64) -> bool {
        scroll_pane_to_position(&self.left, row, offset)
    }

    #[cfg(test)]
    pub(super) fn scroll_right_to_row_offset_for_tests(&self, row: usize, offset: f64) -> bool {
        scroll_pane_to_position(&self.right, row, offset)
    }

    #[cfg(test)]
    pub(super) fn set_left_value_for_tests(&self, value: f64) {
        self.left.adjustment.set_value(value);
    }

    #[cfg(test)]
    pub(super) fn left_value_for_tests(&self) -> f64 {
        self.left.adjustment.value()
    }

    #[cfg(test)]
    pub(super) fn event_counts_for_tests(&self) -> (usize, usize) {
        (self.event_counts.left.get(), self.event_counts.right.get())
    }

    #[cfg(test)]
    pub(super) fn reset_event_counts_for_tests(&self) {
        self.event_counts.left.set(0);
        self.event_counts.right.set(0);
    }
}

impl Drop for CompareScrollSync {
    fn drop(&mut self) {
        self.disconnect();
    }
}
