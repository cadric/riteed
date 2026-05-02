use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::glib::variant::ToVariant;
use gtk4::{gio, glib, prelude::*};

use crate::app_chrome::AppChromeController;
use crate::settings::{AppSettings, ThemePreference};
use crate::window_appearance::WindowAppearanceController;
use crate::workspace::Workspace;

pub(crate) const ACTION_NAME: &str = "theme";
pub(crate) const DETAILED_ACTION_NAME: &str = "win.theme";

pub(crate) fn build_selector() -> gtk4::Box {
    let selector = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::Center)
        .hexpand(true)
        .spacing(12)
        .build();
    selector.add_css_class("riteed-theme-selector");
    selector.set_accessible_role(gtk4::AccessibleRole::RadioGroup);

    let mut group: Option<gtk4::CheckButton> = None;
    for theme in ThemePreference::ALL {
        let button = theme_button(theme);
        if let Some(group) = group.as_ref() {
            button.set_group(Some(group));
        } else {
            group = Some(button.clone());
        }
        selector.append(&button);
    }
    selector
}

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
    appearance: &WindowAppearanceController,
    chrome: Option<&AppChromeController>,
    menu_button: &gtk4::MenuButton,
) {
    sync_theme_action_from_settings(action, settings);

    let settings_for_action = settings.clone();
    let weak_workspace = Rc::downgrade(workspace);
    let appearance_for_action = appearance.clone();
    let chrome_for_action = chrome.cloned();
    action.connect_change_state(move |action, value| {
        let Some(theme) = theme_from_variant(value) else {
            sync_theme_action_from_settings(action, &settings_for_action);
            return;
        };
        settings_for_action.set_theme(theme);
        settings_for_action.apply_theme();
        action.set_state(&theme.nick().to_variant());
        if let Some(chrome) = chrome_for_action.as_ref() {
            chrome.refresh();
        }
        if let Some(workspace) = weak_workspace.upgrade() {
            workspace.apply_source_style_scheme_to_tabs();
        }
        appearance_for_action.sync();
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

fn theme_button(theme: ThemePreference) -> gtk4::CheckButton {
    let button = gtk4::CheckButton::builder()
        .accessible_role(gtk4::AccessibleRole::Radio)
        .focusable(true)
        .halign(gtk4::Align::Center)
        .hexpand(true)
        .build();
    button.add_css_class("riteed-theme-choice");
    button.add_css_class(theme_css_class(theme));
    button.set_size_request(44, 44);
    button.set_focus_on_click(false);
    button.set_tooltip_text(Some(&theme_tooltip(theme)));
    button.set_action_name(Some(DETAILED_ACTION_NAME));
    button.set_action_target_value(Some(&theme.nick().to_variant()));
    button.update_property(&[gtk4::accessible::Property::Label(&theme_accessible_label(
        theme,
    ))]);
    button
}

fn theme_css_class(theme: ThemePreference) -> &'static str {
    match theme {
        ThemePreference::System => "system",
        ThemePreference::Light => "light",
        ThemePreference::Dark => "dark",
    }
}

fn theme_tooltip(theme: ThemePreference) -> String {
    match theme {
        ThemePreference::System => pgettext("theme selector tooltip", "Follow System Style"),
        ThemePreference::Light => pgettext("theme selector tooltip", "Light Style"),
        ThemePreference::Dark => pgettext("theme selector tooltip", "Dark Style"),
    }
}

fn theme_accessible_label(theme: ThemePreference) -> String {
    match theme {
        ThemePreference::System => pgettext("theme selector accessibility", "Follow system style"),
        ThemePreference::Light => pgettext("theme selector accessibility", "Light style"),
        ThemePreference::Dark => pgettext("theme selector accessibility", "Dark style"),
    }
}
