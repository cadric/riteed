use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, glib::variant::ToVariant, prelude::*};
use libadwaita as adw;

use crate::app_chrome::{AppChromeController, ChromeObserver};
use crate::settings::AppSettings;
use crate::window_appearance::WindowAppearanceController;
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

pub(crate) struct WindowActions {
    pub(crate) save: gio::SimpleAction,
    pub(crate) save_as: gio::SimpleAction,
    pub(crate) close: gio::SimpleAction,
    pub(crate) recent_files: gio::SimpleAction,
    pub(crate) search: gio::SimpleAction,
    pub(crate) replace: gio::SimpleAction,
    pub(crate) find_in_files: gio::SimpleAction,
    pub(crate) find_next: gio::SimpleAction,
    pub(crate) find_prev: gio::SimpleAction,
    pub(crate) fullscreen: gio::SimpleAction,
    pub(crate) theme: gio::SimpleAction,
    pub(crate) change_encoding: gio::SimpleAction,
    pub(crate) line_ending: gio::SimpleAction,
}

pub(crate) fn create_window_actions(
    window: &adw::ApplicationWindow,
    settings: &AppSettings,
) -> WindowActions {
    let (change_encoding, line_ending) = crate::window_format_menu::create_actions();
    let actions = WindowActions {
        save: gio::SimpleAction::new("save", None),
        save_as: gio::SimpleAction::new("save-as", None),
        close: gio::SimpleAction::new("close", None),
        recent_files: gio::SimpleAction::new("recent-files", None),
        search: gio::SimpleAction::new("search", None),
        replace: gio::SimpleAction::new("replace", None),
        find_in_files: gio::SimpleAction::new("find-in-files", None),
        find_next: gio::SimpleAction::new("find-next", None),
        find_prev: gio::SimpleAction::new("find-prev", None),
        fullscreen: gio::SimpleAction::new_stateful("fullscreen", None, &false.to_variant()),
        theme: crate::window_theme::create_action(settings),
        change_encoding,
        line_ending,
    };
    add_window_actions(window, &actions);
    actions
}

fn add_window_actions(window: &adw::ApplicationWindow, actions: &WindowActions) {
    for action in [
        &actions.save,
        &actions.save_as,
        &actions.close,
        &actions.recent_files,
        &actions.search,
        &actions.replace,
        &actions.find_in_files,
        &actions.find_next,
        &actions.find_prev,
        &actions.fullscreen,
        &actions.theme,
        &actions.change_encoding,
        &actions.line_ending,
    ] {
        window.add_action(action);
    }
}

pub(crate) fn configure_open_button(shell: &WindowShell) {
    let menu = crate::workspace_menu::build_open_menu();
    shell.open_button.set_menu_model(Some(&menu));
    shell
        .open_button
        .set_dropdown_tooltip(&pgettext("open menu tooltip", "Open Choices"));
}

pub(crate) fn install_chrome_observer(
    chrome: Option<&AppChromeController>,
    appearance: &WindowAppearanceController,
    workspace: &Rc<Workspace>,
) -> Option<ChromeObserver> {
    chrome.map(|chrome| {
        let appearance = appearance.clone();
        let workspace_weak = Rc::downgrade(workspace);
        chrome.add_observer(move || {
            appearance.sync();
            if let Some(workspace) = workspace_weak.upgrade() {
                workspace.apply_source_style_scheme_to_tabs();
            }
        })
    })
}

#[cfg(test)]
pub(crate) fn install_sourceview_for_tests() {
    sourceview5::init();
    crate::source_styles::install_builtin_style_schemes();
}
