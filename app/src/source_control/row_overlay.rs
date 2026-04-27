use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;

/// Wires hit-test gating on `actions_box` so it only intercepts pointer events
/// while the pointer is inside `overlay` or keyboard focus is inside `actions_box`.
///
/// The overlay's actions are revealed visually via CSS (`opacity` transition on
/// `:hover`/`:focus-within`), but `opacity: 0` does not stop GTK from delivering
/// pointer events to the (invisible) buttons. Without this gate, clicks on the
/// row's right side would silently trigger stage/unstage/discard.
///
/// Both controllers use weak captures of `actions_box` to avoid reference cycles
/// (the focus controller is owned by `actions_box`, so a strong capture would loop).
pub(super) fn setup_action_overlay(overlay: &gtk4::Overlay, actions_box: &gtk4::Box) {
    let pointer_inside = Rc::new(Cell::new(false));
    let focus_inside = Rc::new(Cell::new(false));

    let motion = gtk4::EventControllerMotion::new();

    let actions_weak = actions_box.downgrade();
    let pointer_for_enter = pointer_inside.clone();
    let focus_for_enter = focus_inside.clone();
    motion.connect_enter(move |_, _, _| {
        let Some(actions_box) = actions_weak.upgrade() else {
            return;
        };
        pointer_for_enter.set(true);
        actions_box.set_can_target(pointer_for_enter.get() || focus_for_enter.get());
    });

    let actions_weak = actions_box.downgrade();
    let pointer_for_leave = pointer_inside.clone();
    let focus_for_leave = focus_inside.clone();
    motion.connect_leave(move |_| {
        let Some(actions_box) = actions_weak.upgrade() else {
            return;
        };
        pointer_for_leave.set(false);
        actions_box.set_can_target(pointer_for_leave.get() || focus_for_leave.get());
    });

    overlay.add_controller(motion);

    let focus = gtk4::EventControllerFocus::new();

    let actions_weak = actions_box.downgrade();
    let pointer_for_focus_enter = pointer_inside.clone();
    let focus_for_focus_enter = focus_inside.clone();
    focus.connect_enter(move |_| {
        let Some(actions_box) = actions_weak.upgrade() else {
            return;
        };
        focus_for_focus_enter.set(true);
        actions_box.set_can_target(pointer_for_focus_enter.get() || focus_for_focus_enter.get());
    });

    let actions_weak = actions_box.downgrade();
    let pointer_for_focus_leave = pointer_inside;
    let focus_for_focus_leave = focus_inside;
    focus.connect_leave(move |_| {
        let Some(actions_box) = actions_weak.upgrade() else {
            return;
        };
        focus_for_focus_leave.set(false);
        actions_box.set_can_target(pointer_for_focus_leave.get() || focus_for_focus_leave.get());
    });

    actions_box.add_controller(focus);
}
