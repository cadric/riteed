use std::cell::RefCell;
use std::rc::Rc;

use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::window_project::{
    DEFAULT_PROJECT_SIDEBAR_WIDTH, MAX_PROJECT_SIDEBAR_WIDTH, MIN_PROJECT_SIDEBAR_WIDTH,
    ProjectState,
};

pub(super) fn sync_actions_for_root(state: &Rc<RefCell<ProjectState>>) {
    let mut state = state.borrow_mut();
    let has_root = state.root.is_some();
    state.sidebar_visible_action.set_enabled(has_root);
    state.show_hidden_action.set_enabled(has_root);
    state.refresh_action.set_enabled(has_root);
    state.close_action.set_enabled(has_root);
    if !has_root {
        state.sidebar_visible_action.set_state(&false.to_variant());
        set_sidebar_visibility(&mut state, false);
    }
}

pub(super) fn set_sidebar_visible_for_root(state: &Rc<RefCell<ProjectState>>, visible: bool) {
    if let Ok(mut state) = state.try_borrow_mut() {
        set_sidebar_visibility(&mut state, visible);
    }
}

pub(super) fn set_sidebar_visibility(state: &mut ProjectState, visible: bool) {
    if visible {
        let width = remembered_width(state);
        state.sidebar_width = width;
        persist_sidebar_visible(state, true);
        state.sidebar_visible_action.set_state(&true.to_variant());
        animate_split_position(state, width);
    } else {
        let position = state.split_view.position();
        if position >= MIN_PROJECT_SIDEBAR_WIDTH {
            state.sidebar_width = clamp_visible_width(position);
        }
        persist_sidebar_visible(state, false);
        state.sidebar_visible_action.set_state(&false.to_variant());
        animate_split_position(state, 0);
    }
}

pub(super) fn set_sidebar_position_from_move(state: &mut ProjectState, split_view: &gtk4::Paned) {
    if state.sidebar_position_guard.get() {
        return;
    }
    if state.root.is_none() {
        set_split_position(state, 0);
        persist_sidebar_visible(state, false);
        state.sidebar_visible_action.set_state(&false.to_variant());
        return;
    }
    cancel_sidebar_animation(state);

    let position = split_view.position();
    if position <= 0 {
        if !state.settings.project_sidebar_visible() {
            state.sidebar_visible_action.set_state(&false.to_variant());
            return;
        }
        let width = remembered_width(state);
        state.sidebar_width = width;
        set_split_position(state, width);
        persist_sidebar_visible(state, true);
        state.sidebar_visible_action.set_state(&true.to_variant());
        return;
    }

    let width = clamp_visible_width(position);
    state.sidebar_width = width;
    if width != position {
        set_split_position(state, width);
    }
    persist_sidebar_visible(state, true);
    state.sidebar_visible_action.set_state(&true.to_variant());
}

fn remembered_width(state: &ProjectState) -> i32 {
    if state.sidebar_width <= 0 {
        return DEFAULT_PROJECT_SIDEBAR_WIDTH;
    }
    clamp_visible_width(state.sidebar_width)
}

fn clamp_visible_width(width: i32) -> i32 {
    width.clamp(MIN_PROJECT_SIDEBAR_WIDTH, MAX_PROJECT_SIDEBAR_WIDTH)
}

fn set_split_position(state: &mut ProjectState, position: i32) {
    cancel_sidebar_animation(state);
    if state.split_view.position() == position {
        return;
    }
    state.sidebar_position_guard.set(true);
    state.split_view.set_position(position);
    state.sidebar_position_guard.set(false);
}

fn animate_split_position(state: &mut ProjectState, position: i32) {
    let current = state.split_view.position();
    if (current - position).abs() < 2 {
        set_split_position(state, position);
        return;
    }
    cancel_sidebar_animation(state);
    let split_view = state.split_view.clone();
    let guard = Rc::clone(&state.sidebar_position_guard);
    let target = adw::CallbackAnimationTarget::new(move |value| {
        guard.set(true);
        split_view.set_position(rounded_i32(value));
        guard.set(false);
    });
    let animation = adw::TimedAnimation::new(
        &state.split_view,
        current.into(),
        position.into(),
        180,
        target,
    );
    animation.set_easing(adw::Easing::EaseOutCubic);
    animation.play();
    state.sidebar_animation = Some(animation);
}

fn cancel_sidebar_animation(state: &mut ProjectState) {
    if let Some(animation) = state.sidebar_animation.take() {
        animation.pause();
    }
}

fn rounded_i32(value: f64) -> i32 {
    let rounded = value.round();
    if rounded <= f64::from(i32::MIN) {
        i32::MIN
    } else if rounded >= f64::from(i32::MAX) {
        i32::MAX
    } else {
        format!("{rounded:.0}")
            .parse::<i32>()
            .map_or(0, |value| value)
    }
}

fn persist_sidebar_visible(state: &ProjectState, visible: bool) {
    if state.settings.project_sidebar_visible() != visible {
        state.settings.set_project_sidebar_visible(visible);
    }
}
