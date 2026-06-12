use std::rc::Rc;

use gtk4::prelude::*;

use super::CompareController;
use crate::editor_tab::EditorTab;
use crate::editor_zoom::effective_scroll_past_end_padding;

impl CompareController {
    // The compare minimaps mirror their views' bottom margins through
    // GtkSourceMap's own scaled property binding.
    pub(crate) fn apply_scroll_past_end_padding(&mut self, bottom_margin: i32) {
        self.scroll_past_end_floor = bottom_margin;
        self.refresh_scroll_past_end_padding();
    }

    pub(crate) fn refresh_scroll_past_end_padding(&self) {
        self.left_view
            .set_bottom_margin(effective_scroll_past_end_padding(
                self.scroll_past_end_floor,
                self.left_vadjustment.page_size(),
            ));
        self.right_view
            .set_bottom_margin(effective_scroll_past_end_padding(
                self.scroll_past_end_floor,
                self.right_vadjustment.page_size(),
            ));
        self.unified_view
            .set_bottom_margin(effective_scroll_past_end_padding(
                self.scroll_past_end_floor,
                self.unified_vadjustment.page_size(),
            ));
    }
}

impl EditorTab {
    pub(crate) fn refresh_compare_scroll_past_end_padding(&self) {
        if let Ok(state) = self.state.try_borrow()
            && let Some(compare) = state.compare.active.as_ref()
        {
            compare.refresh_scroll_past_end_padding();
        }
    }
}

pub(super) fn connect_compare_page_size_notifications(
    tab: &Rc<EditorTab>,
    left: &gtk4::ScrolledWindow,
    right: &gtk4::ScrolledWindow,
    unified: &gtk4::ScrolledWindow,
) -> (gtk4::Adjustment, gtk4::Adjustment, gtk4::Adjustment) {
    let left_vadjustment = left.vadjustment();
    let right_vadjustment = right.vadjustment();
    let unified_vadjustment = unified.vadjustment();
    connect_compare_page_size_notify(tab, &left_vadjustment);
    connect_compare_page_size_notify(tab, &right_vadjustment);
    connect_compare_page_size_notify(tab, &unified_vadjustment);
    (left_vadjustment, right_vadjustment, unified_vadjustment)
}

fn connect_compare_page_size_notify(tab: &Rc<EditorTab>, adjustment: &gtk4::Adjustment) {
    let weak = Rc::downgrade(tab);
    adjustment.connect_page_size_notify(move |_| {
        let Some(tab) = weak.upgrade() else {
            return;
        };
        tab.refresh_compare_scroll_past_end_padding();
    });
}
