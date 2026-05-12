use std::cell::RefCell;
use std::rc::Rc;

use gtk4::glib::variant::ToVariant;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::settings::{
    AppSettings, CompareReviewSettingsSnapshot, CompareViewMode, SettingsSubscription,
};

use super::WindowCompareController;

pub(super) struct CompareSettingsActions {
    pub(super) view_mode: gio::SimpleAction,
    pub(super) collapse_unchanged: gio::SimpleAction,
    pub(super) ignore_whitespace: gio::SimpleAction,
    pub(super) word_wrap: gio::SimpleAction,
    pub(super) context_lines: gio::SimpleAction,
    subscriptions: RefCell<Vec<SettingsSubscription>>,
}

impl CompareSettingsActions {
    #[must_use]
    pub(super) fn new(settings: &AppSettings) -> Self {
        Self {
            view_mode: gio::SimpleAction::new_stateful(
                "compare-view-mode",
                Some(glib::VariantTy::STRING),
                &settings.compare_view_mode().nick().to_variant(),
            ),
            collapse_unchanged: gio::SimpleAction::new_stateful(
                "compare-collapse-unchanged",
                None,
                &settings.compare_collapse_unchanged().to_variant(),
            ),
            ignore_whitespace: gio::SimpleAction::new_stateful(
                "compare-ignore-leading-trailing-whitespace",
                None,
                &settings
                    .compare_ignore_leading_trailing_whitespace()
                    .to_variant(),
            ),
            word_wrap: gio::SimpleAction::new_stateful(
                "compare-word-wrap",
                None,
                &settings.compare_word_wrap().to_variant(),
            ),
            context_lines: gio::SimpleAction::new_stateful(
                "compare-context-lines",
                Some(glib::VariantTy::INT32),
                &settings.compare_context_lines().to_variant(),
            ),
            subscriptions: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn add_to_window(&self, window: &adw::ApplicationWindow) {
        window.add_action(&self.view_mode);
        window.add_action(&self.collapse_unchanged);
        window.add_action(&self.ignore_whitespace);
        window.add_action(&self.word_wrap);
        window.add_action(&self.context_lines);
    }
}

impl WindowCompareController {
    pub(super) fn install_compare_settings_callbacks(self: &Rc<Self>) {
        self.install_compare_settings_action_handlers();
        self.install_compare_settings_observers();
        self.sync_compare_settings_actions();
    }

    pub(super) fn sync_compare_settings_actions(&self) {
        let settings = &self.workspace.settings;
        let snapshot = settings.compare_review_settings_snapshot();
        self.compare_settings_actions
            .view_mode
            .set_state(&snapshot.view_mode.nick().to_variant());
        self.compare_settings_actions
            .collapse_unchanged
            .set_state(&snapshot.collapse_unchanged.to_variant());
        self.compare_settings_actions
            .ignore_whitespace
            .set_state(&snapshot.ignore_leading_trailing_whitespace.to_variant());
        self.compare_settings_actions
            .word_wrap
            .set_state(&snapshot.word_wrap.to_variant());
        self.compare_settings_actions
            .context_lines
            .set_state(&snapshot.context_lines.to_variant());
        self.sync_compare_settings_action_sensitivity(snapshot);
    }

    fn install_compare_settings_action_handlers(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.compare_settings_actions
            .view_mode
            .connect_change_state(move |action, value| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let Some(mode) = compare_mode_from_variant(value) else {
                    controller.sync_compare_settings_actions();
                    return;
                };
                controller.workspace.settings.set_compare_view_mode(mode);
                action.set_state(&mode.nick().to_variant());
                controller.refresh_compare_settings_for_active_tab();
            });

        let weak = Rc::downgrade(self);
        self.compare_settings_actions
            .collapse_unchanged
            .connect_change_state(move |action, value| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let Some(enabled) = boolean_from_variant(value) else {
                    controller.sync_compare_settings_actions();
                    return;
                };
                controller
                    .workspace
                    .settings
                    .set_compare_collapse_unchanged(enabled);
                action.set_state(&enabled.to_variant());
                controller.refresh_compare_settings_for_active_tab();
            });

        let weak = Rc::downgrade(self);
        self.compare_settings_actions
            .ignore_whitespace
            .connect_change_state(move |action, value| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let Some(enabled) = boolean_from_variant(value) else {
                    controller.sync_compare_settings_actions();
                    return;
                };
                controller
                    .workspace
                    .settings
                    .set_compare_ignore_leading_trailing_whitespace(enabled);
                action.set_state(&enabled.to_variant());
                controller.refresh_compare_settings_for_active_tab();
            });

        let weak = Rc::downgrade(self);
        self.compare_settings_actions
            .word_wrap
            .connect_change_state(move |action, value| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let Some(enabled) = boolean_from_variant(value) else {
                    controller.sync_compare_settings_actions();
                    return;
                };
                controller.workspace.settings.set_compare_word_wrap(enabled);
                action.set_state(&enabled.to_variant());
                controller.refresh_compare_settings_for_active_tab();
            });

        let weak = Rc::downgrade(self);
        self.compare_settings_actions
            .context_lines
            .connect_change_state(move |action, value| {
                let Some(controller) = weak.upgrade() else {
                    return;
                };
                let Some(lines) = value.and_then(glib::Variant::get::<i32>) else {
                    controller.sync_compare_settings_actions();
                    return;
                };
                let lines = lines.clamp(1, 10);
                controller
                    .workspace
                    .settings
                    .set_compare_context_lines(lines);
                action.set_state(&lines.to_variant());
                controller.refresh_compare_settings_for_active_tab();
            });
    }

    fn install_compare_settings_observers(self: &Rc<Self>) {
        let mut subscriptions = self.compare_settings_actions.subscriptions.borrow_mut();
        if !subscriptions.is_empty() {
            return;
        }
        let settings = self.workspace.settings.clone();

        let weak = Rc::downgrade(self);
        subscriptions.push(settings.connect_compare_view_mode_changed(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.sync_compare_settings_actions();
                controller.refresh_compare_settings_for_active_tab();
            }
        }));

        let weak = Rc::downgrade(self);
        subscriptions.push(settings.connect_compare_collapse_changed(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.sync_compare_settings_actions();
                controller.refresh_compare_settings_for_active_tab();
            }
        }));

        let weak = Rc::downgrade(self);
        subscriptions.push(settings.connect_compare_context_lines_changed(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.sync_compare_settings_actions();
                controller.refresh_compare_settings_for_active_tab();
            }
        }));

        let weak = Rc::downgrade(self);
        subscriptions.push(
            settings.connect_compare_ignore_whitespace_changed(move |_| {
                if let Some(controller) = weak.upgrade() {
                    controller.sync_compare_settings_actions();
                    controller.refresh_compare_settings_for_active_tab();
                }
            }),
        );

        let weak = Rc::downgrade(self);
        subscriptions.push(settings.connect_compare_word_wrap_changed(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller.sync_compare_settings_actions();
                controller.refresh_compare_settings_for_active_tab();
            }
        }));
    }

    fn refresh_compare_settings_for_active_tab(&self) {
        self.sync_compare_settings_actions();
        if let Some(tab) = self.workspace.selected_tab() {
            tab.refresh_compare_settings();
        }
        self.workspace.refresh_selected_state();
    }

    fn sync_compare_settings_action_sensitivity(&self, _snapshot: CompareReviewSettingsSnapshot) {
        let selected = self.workspace.selected_tab();
        let has_compare_surface = selected.as_ref().is_some_and(|tab| {
            tab.is_compare_active() || tab.kind() == crate::editor_tab::TabKind::GitReview
        });
        let unified_available = selected.as_ref().is_some_and(|tab| {
            if tab.kind() == crate::editor_tab::TabKind::GitReview {
                true
            } else {
                tab.compare_uses_unified_layout()
            }
        });
        self.compare_settings_actions
            .view_mode
            .set_enabled(has_compare_surface);
        self.compare_settings_actions
            .collapse_unchanged
            .set_enabled(has_compare_surface);
        self.compare_settings_actions
            .ignore_whitespace
            .set_enabled(has_compare_surface);
        self.compare_settings_actions
            .context_lines
            .set_enabled(has_compare_surface);
        self.compare_settings_actions
            .word_wrap
            .set_enabled(unified_available);
    }
}

fn compare_mode_from_variant(value: Option<&glib::Variant>) -> Option<CompareViewMode> {
    value
        .and_then(glib::Variant::get::<String>)
        .and_then(|nick| CompareViewMode::from_nick(&nick))
}

fn boolean_from_variant(value: Option<&glib::Variant>) -> Option<bool> {
    value.and_then(glib::Variant::get::<bool>)
}
