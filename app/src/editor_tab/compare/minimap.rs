use std::cell::{Cell, RefCell};
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
    state: Rc<CompareMinimapState>,
}

struct CompareMinimapState {
    holder: gtk4::Box,
    map: sourceview5::Map,
    scrolled: gtk4::ScrolledWindow,
    view: sourceview5::View,
    desired_visible: Cell<bool>,
    syncing: Cell<bool>,
    font_desc: RefCell<Option<pango::FontDescription>>,
}

impl CompareMinimap {
    pub(super) fn new(
        tab: &EditorTab,
        view: &sourceview5::View,
        scrolled: &gtk4::ScrolledWindow,
    ) -> Self {
        let minimap_font = resolve_minimap_font_description(&tab.settings.editor_font());
        let map = sourceview5::Map::builder().font_desc(&minimap_font).build();
        configure_map(&map);

        let desired_visible = tab.settings.show_minimap();
        let holder = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .visible(desired_visible)
            .build();
        holder.set_hexpand(false);
        holder.set_vexpand(true);
        holder.set_width_request(COMPARE_MINIMAP_WIDTH);
        holder.append(&map);

        let state = Rc::new(CompareMinimapState {
            holder: holder.clone(),
            map,
            scrolled: scrolled.clone(),
            view: view.clone(),
            desired_visible: Cell::new(desired_visible),
            syncing: Cell::new(false),
            font_desc: RefCell::new(Some(minimap_font)),
        });
        CompareMinimapState::connect_holder_lifecycle(&state);
        state.sync_map();

        Self { holder, state }
    }

    pub(super) fn set_visible(&self, visible: bool) {
        self.state.desired_visible.set(visible);
        if visible {
            self.holder.set_visible(true);
            self.state.sync_map();
        } else {
            self.state.sync_map();
            self.holder.set_visible(false);
        }
        self.state
            .scrolled
            .set_vscrollbar_policy(gtk4::PolicyType::Automatic);
    }

    pub(super) fn set_font_desc(&self, font_desc: Option<&pango::FontDescription>) {
        self.state.font_desc.replace(font_desc.cloned());
        self.state.apply_font_desc();
    }

    #[cfg(test)]
    pub(super) fn visible_for_tests(&self) -> bool {
        self.holder.property::<bool>("visible")
    }

    #[cfg(test)]
    pub(super) fn scrollbar_policy_for_tests(&self) -> gtk4::PolicyType {
        self.state.scrolled.vscrollbar_policy()
    }

    #[cfg(test)]
    pub(super) fn map_attached_for_tests(&self) -> bool {
        self.state.map_attached()
    }

    #[cfg(test)]
    pub(super) fn holder_mapped_for_tests(&self) -> bool {
        self.holder.is_mapped()
    }

    #[cfg(test)]
    pub(super) fn viewport_range_for_tests(&self) -> (f64, f64) {
        self.state.viewport_range()
    }

    #[cfg(test)]
    pub(super) fn set_viewport_page_size_for_tests(&self, page_size: f64) {
        self.state.scrolled.vadjustment().set_page_size(page_size);
    }
}

impl CompareMinimapState {
    fn connect_holder_lifecycle(state: &Rc<Self>) {
        let weak_state = Rc::downgrade(state);
        state.holder.connect_map(move |_| {
            if let Some(state) = weak_state.upgrade() {
                state.sync_map();
            }
        });

        let weak_state = Rc::downgrade(state);
        state.holder.connect_unmap(move |_| {
            if let Some(state) = weak_state.upgrade() {
                state.detach_map();
            }
        });
    }

    fn sync_map(&self) {
        if self.syncing.get() {
            return;
        }
        let should_attach = self.desired_visible.get() && self.holder.is_mapped();
        if self.map_attached() == should_attach {
            return;
        }

        self.syncing.set(true);
        if should_attach {
            self.attach_map();
        } else {
            self.detach_map();
        }
        self.syncing.set(false);
    }

    fn attach_map(&self) {
        self.apply_font_desc();
        self.map.set_view(&self.view);
    }

    fn detach_map(&self) {
        if self.map_attached() {
            self.map
                .set_property("view", Option::<&sourceview5::View>::None);
        }
    }

    fn apply_font_desc(&self) {
        let font_desc = self.font_desc.borrow();
        self.map.set_font_desc(font_desc.as_ref());
    }

    fn map_attached(&self) -> bool {
        self.map.view().is_some()
    }

    #[cfg(test)]
    fn viewport_range(&self) -> (f64, f64) {
        self.view.vadjustment().map_or((0.0, 0.0), |adjustment| {
            (adjustment.upper(), adjustment.page_size())
        })
    }
}

fn configure_map(map: &sourceview5::Map) {
    map.set_can_focus(false);
    map.set_cursor_visible(false);
    map.set_editable(false);
    map.set_focusable(false);
    map.set_hexpand(false);
    map.set_monospace(true);
    map.set_vexpand(true);
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
