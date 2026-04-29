use std::rc::Rc;

use gtk4::glib::variant::ToVariant;
use gtk4::{gio, glib, prelude::*};

use crate::settings::{AppSettings, ThemePreference};
use crate::workspace::Workspace;

pub(crate) const ACTION_NAME: &str = "theme";
pub(crate) const DETAILED_ACTION_NAME: &str = "win.theme";

pub(crate) fn create_action(settings: &AppSettings) -> gio::SimpleAction {
    gio::SimpleAction::new_stateful(
        ACTION_NAME,
        Some(glib::VariantTy::STRING),
        &settings.theme().nick().to_variant(),
    )
}

pub(crate) fn install(
    action: &gio::SimpleAction,
    settings: &AppSettings,
    workspace: &Rc<Workspace>,
    menu_button: &gtk4::MenuButton,
) {
    sync_theme_action_from_settings(action, settings);

    let settings_for_action = settings.clone();
    let weak_workspace = Rc::downgrade(workspace);
    action.connect_change_state(move |action, value| {
        let Some(theme) = theme_from_variant(value) else {
            sync_theme_action_from_settings(action, &settings_for_action);
            return;
        };
        settings_for_action.set_theme(theme);
        settings_for_action.apply_theme();
        action.set_state(&theme.nick().to_variant());
        if let Some(workspace) = weak_workspace.upgrade() {
            workspace.apply_source_style_scheme_to_tabs();
        }
    });

    let action_for_active = action.clone();
    let settings_for_active = settings.clone();
    menu_button.connect_active_notify(move |button| {
        if button.is_active() {
            sync_theme_action_from_settings(&action_for_active, &settings_for_active);
        }
    });

    let action_for_popover = action.clone();
    let settings_for_popover = settings.clone();
    menu_button.connect_notify_local(Some("popover"), move |_, _| {
        sync_theme_action_from_settings(&action_for_popover, &settings_for_popover);
    });
}

#[cfg(test)]
pub(crate) fn set_theme_for_tests(action: &gio::SimpleAction, theme: ThemePreference) {
    action.change_state(&theme.nick().to_variant());
}

fn theme_from_variant(value: Option<&glib::Variant>) -> Option<ThemePreference> {
    value
        .and_then(glib::Variant::get::<String>)
        .and_then(|nick| ThemePreference::from_nick(&nick))
}

fn sync_theme_action_from_settings(action: &gio::SimpleAction, settings: &AppSettings) {
    action.set_state(&settings.theme().nick().to_variant());
}
