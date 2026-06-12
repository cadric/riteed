use std::cell::Cell;
use std::rc::Rc;

use gtk4::{pango, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use sourceview5::prelude::*;

use crate::editor_tab::EditorTab;
use crate::editor_zoom::resolve_minimap_font_description;

const COMPARE_MINIMAP_WIDTH: i32 = 72;
const COMPARE_MINIMAP_HIDE_MAX_WIDTH_SP: f64 = 960.0;

#[derive(Clone)]
pub(super) struct CompareMinimap {
    pub(super) holder: gtk4::Box,
    map: sourceview5::Map,
    scrolled: gtk4::ScrolledWindow,
}

impl CompareMinimap {
    pub(super) fn new(tab: &EditorTab, view: &sourceview5::View) -> Self {
        let minimap_font = resolve_minimap_font_description(&tab.settings.editor_font());
        let map = sourceview5::Map::builder()
            .view(view)
            .font_desc(&minimap_font)
            .build();
        map.set_can_focus(false);
        map.set_cursor_visible(false);
        map.set_editable(false);
        map.set_focusable(false);
        map.set_hexpand(false);
        map.set_monospace(true);
        map.set_vexpand(true);

        let holder = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .visible(tab.settings.show_minimap())
            .build();
        holder.set_hexpand(false);
        holder.set_vexpand(true);
        holder.set_width_request(COMPARE_MINIMAP_WIDTH);
        holder.append(&map);

        Self {
            holder,
            map,
            scrolled: gtk4::ScrolledWindow::new(),
        }
    }

    pub(super) fn with_scrolled(mut self, scrolled: &gtk4::ScrolledWindow) -> Self {
        self.scrolled = scrolled.clone();
        self
    }

    pub(super) fn set_visible(&self, visible: bool) {
        self.holder.set_visible(visible);
        self.scrolled
            .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
    }

    pub(super) fn set_font_desc(&self, font_desc: Option<&pango::FontDescription>) {
        self.map.set_font_desc(font_desc);
    }

    #[cfg(test)]
    pub(super) fn visible_for_tests(&self) -> bool {
        self.holder.property::<bool>("visible")
    }

    #[cfg(test)]
    pub(super) fn scrollbar_policy_for_tests(&self) -> gtk4::PolicyType {
        self.scrolled.vscrollbar_policy()
    }

    #[cfg(test)]
    pub(super) fn set_viewport_page_size_for_tests(&self, page_size: f64) {
        self.scrolled.vadjustment().set_page_size(page_size);
    }
}

pub(super) fn install_width_suppression_breakpoint(
    root: &adw::BreakpointBin,
    left: &CompareMinimap,
    right: &CompareMinimap,
    unified: &CompareMinimap,
    user_visible: Rc<Cell<bool>>,
    width_suppressed: Rc<Cell<bool>>,
) {
    let condition = adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        COMPARE_MINIMAP_HIDE_MAX_WIDTH_SP,
        adw::LengthUnit::Sp,
    );
    let breakpoint = adw::Breakpoint::new(condition);
    let left_for_apply = left.clone();
    let right_for_apply = right.clone();
    let unified_for_apply = unified.clone();
    let user_visible_for_apply = Rc::clone(&user_visible);
    let width_suppressed_for_apply = Rc::clone(&width_suppressed);
    breakpoint.connect_apply(move |_| {
        width_suppressed_for_apply.set(true);
        apply_effective_visibility(
            &left_for_apply,
            &right_for_apply,
            &unified_for_apply,
            user_visible_for_apply.get(),
            true,
        );
    });

    let left_for_unapply = left.clone();
    let right_for_unapply = right.clone();
    let unified_for_unapply = unified.clone();
    breakpoint.connect_unapply(move |_| {
        width_suppressed.set(false);
        apply_effective_visibility(
            &left_for_unapply,
            &right_for_unapply,
            &unified_for_unapply,
            user_visible.get(),
            false,
        );
    });
    root.add_breakpoint(breakpoint);
}

pub(super) fn apply_width_suppressed_visibility(
    left: &CompareMinimap,
    right: &CompareMinimap,
    unified: &CompareMinimap,
    user_visible: bool,
    width_suppressed: bool,
) {
    apply_effective_visibility(left, right, unified, user_visible, width_suppressed);
}

fn apply_effective_visibility(
    left: &CompareMinimap,
    right: &CompareMinimap,
    unified: &CompareMinimap,
    user_visible: bool,
    width_suppressed: bool,
) {
    let visible = user_visible && !width_suppressed;
    left.set_visible(visible);
    right.set_visible(visible);
    unified.set_visible(visible);
}
