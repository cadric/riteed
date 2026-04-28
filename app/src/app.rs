use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gettextrs::gettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs;
use crate::window::Window;
use crate::{APP_ID, APP_NAME};

pub(crate) struct AppState {
    pub(crate) windows: Vec<Rc<Window>>,
    pub(crate) last_focused_window: Option<Weak<Window>>,
    pub(crate) session_restore_attempted: bool,
}

type WindowFactory = fn(&adw::Application) -> Result<Rc<Window>, crate::error::AppError>;

#[derive(Clone, Copy)]
struct WindowFactories {
    primary: WindowFactory,
    secondary: WindowFactory,
}

#[derive(Clone)]
pub struct RiteedApp {
    app: adw::Application,
    state: Rc<RefCell<AppState>>,
}

impl Default for RiteedApp {
    fn default() -> Self {
        Self::new()
    }
}

impl RiteedApp {
    #[must_use]
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id(APP_ID)
            .flags(gio::ApplicationFlags::HANDLES_OPEN)
            .resource_base_path("/io/github/cadric/Riteed")
            .build();
        let state = Rc::new(RefCell::new(AppState {
            windows: Vec::new(),
            last_focused_window: None,
            session_restore_attempted: false,
        }));

        install_accels(&app);
        let factories = WindowFactories {
            primary: Window::new,
            secondary: Window::new_secondary,
        };
        install_actions(&app, &state, factories);
        install_lifecycle(&app, &state, factories);

        Self { app, state }
    }

    #[must_use]
    pub fn run(&self) -> gtk4::glib::ExitCode {
        let _keep_state_alive = self.state.borrow().windows.clone();
        self.app.run()
    }

    #[cfg(test)]
    pub(crate) fn application(&self) -> &adw::Application {
        &self.app
    }
}

#[cfg(test)]
pub(crate) fn install_for_tests(app: &adw::Application, state: &Rc<RefCell<AppState>>) {
    install_accels(app);
    let factories = WindowFactories {
        primary: Window::new_for_tests,
        secondary: Window::new_secondary_for_tests,
    };
    install_actions(app, state, factories);
    install_lifecycle(app, state, factories);
}

#[cfg(test)]
pub(crate) fn ensure_window_for_tests(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
) -> Option<Rc<Window>> {
    let factories = WindowFactories {
        primary: Window::new_for_tests,
        secondary: Window::new_secondary_for_tests,
    };
    ensure_window(app, state, factories).map(|(window, _created)| window)
}

fn install_accels(app: &adw::Application) {
    app.set_accels_for_action("app.new-window", &["<Ctrl>n"]);
    app.set_accels_for_action("app.new", &["<Ctrl>t"]);
    app.set_accels_for_action("win.tab-move-to-new-window", &["<Ctrl><Shift>n"]);
    app.set_accels_for_action("app.open", &["<Ctrl>o"]);
    app.set_accels_for_action("app.open-folder", &["<Ctrl><Shift>o"]);
    app.set_accels_for_action("win.save", &["<Ctrl>s"]);
    app.set_accels_for_action("win.save-as", &["<Ctrl><Shift>s"]);
    app.set_accels_for_action("win.close", &["<Ctrl>w"]);
    app.set_accels_for_action("win.search", &["<Ctrl>f"]);
    app.set_accels_for_action("win.replace", &["<Ctrl>h"]);
    app.set_accels_for_action("win.find-next", &["<Ctrl>g", "F3"]);
    app.set_accels_for_action("win.find-prev", &["<Ctrl><Shift>g", "<Shift>F3"]);
    app.set_accels_for_action("win.diff-next", &["F8"]);
    app.set_accels_for_action("win.diff-prev", &["<Shift>F8"]);
    app.set_accels_for_action("win.refresh-project-tree", &["<Ctrl>r", "F5"]);
    app.set_accels_for_action("win.project-sidebar-visible", &["F9"]);
    app.set_accels_for_action("win.fullscreen", &["F11"]);
    app.set_accels_for_action(
        "win.zoom-in",
        &["<Ctrl>plus", "<Ctrl>equal", "<Ctrl>KP_Add"],
    );
    app.set_accels_for_action("win.zoom-out", &["<Ctrl>minus", "<Ctrl>KP_Subtract"]);
    app.set_accels_for_action("win.zoom-reset", &["<Ctrl>0"]);
    app.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
    app.set_accels_for_action("win.show-help-overlay", &["<Ctrl>question"]);
    app.set_accels_for_action("app.help", &["F1"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
}

fn install_actions(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factories: WindowFactories,
) {
    install_file_actions(app, state, factories);
    install_app_actions(app, state, factories);
}

fn install_file_actions(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factories: WindowFactories,
) {
    let new_action = gio::SimpleAction::new("new", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    new_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            } else {
                window.request_new();
            }
            window.present();
        }
    });
    app.add_action(&new_action);

    let new_window_action = gio::SimpleAction::new("new-window", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    new_window_action.connect_activate(move |_, _| {
        if let Some(window) =
            create_window(&app_clone, &state_clone, factories.secondary, factories)
        {
            window.ensure_default_tab();
            window.present();
        }
    });
    app.add_action(&new_window_action);

    let open_action = gio::SimpleAction::new("open", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    open_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            window.present();
            window.request_open_dialog();
        }
    });
    app.add_action(&open_action);

    let open_folder_action = gio::SimpleAction::new("open-folder", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    open_folder_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            window.present();
            window.request_open_folder_dialog();
        }
    });
    app.add_action(&open_folder_action);

    let open_recent_action = gio::SimpleAction::new("open-recent", Some(glib::VariantTy::STRING));
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    open_recent_action.connect_activate(move |_, parameter| {
        let Some(uri) = parameter.and_then(glib::Variant::str) else {
            return;
        };
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            window.present();
            window.request_open_recent(uri);
        }
    });
    app.add_action(&open_recent_action);
}

fn install_app_actions(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factories: WindowFactories,
) {
    let preferences_action = gio::SimpleAction::new("preferences", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    preferences_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            window.show_preferences();
            window.present();
        }
    });
    app.add_action(&preferences_action);

    let help_action = gio::SimpleAction::new("help", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    help_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            window.show_help();
            window.present();
        }
    });
    app.add_action(&help_action);

    let about_action = gio::SimpleAction::new("about", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    about_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            window.show_about();
            window.present();
        }
    });
    app.add_action(&about_action);

    let quit_action = gio::SimpleAction::new("quit", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    quit_action.connect_activate(move |_, _| {
        let windows = state_clone.borrow().windows.clone();
        if windows.is_empty() {
            app_clone.quit();
            return;
        }
        for window in windows {
            window.widget().close();
        }
    });
    app.add_action(&quit_action);
}

fn install_lifecycle(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factories: WindowFactories,
) {
    let state_clone = Rc::clone(state);
    app.connect_activate(move |app| {
        if let Some((window, _created)) = ensure_window(app, &state_clone, factories) {
            let should_restore = {
                let mut app_state = state_clone.borrow_mut();
                if app_state.session_restore_attempted {
                    false
                } else {
                    app_state.session_restore_attempted = true;
                    true
                }
            };
            if should_restore {
                window.restore_session();
            } else {
                window.ensure_default_tab();
            }
            window.present();
        }
    });

    let state_clone = Rc::clone(state);
    app.connect_open(move |app, files, _hint| {
        if let Some((window, created)) = ensure_window(app, &state_clone, factories) {
            if created {
                window.ensure_default_tab();
            }
            let items = files.to_vec();
            if items.is_empty() {
                window.present();
            } else {
                window.handle_application_open(items);
                window.present();
            }
        }
    });
}

fn ensure_window(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factories: WindowFactories,
) -> Option<(Rc<Window>, bool)> {
    if let Some(window) = resolve_window(state) {
        return Some((window, false));
    }

    create_window(app, state, factories.primary, factories).map(|window| (window, true))
}

fn resolve_window(state: &Rc<RefCell<AppState>>) -> Option<Rc<Window>> {
    let app_state = state.borrow();
    if let Some(window) = app_state
        .last_focused_window
        .as_ref()
        .and_then(Weak::upgrade)
    {
        return Some(window);
    }
    app_state.windows.last().cloned()
}

fn create_window(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factory: WindowFactory,
    factories: WindowFactories,
) -> Option<Rc<Window>> {
    match factory(app) {
        Ok(window) => {
            register_window(state, &window);
            install_transfer_window_handler(app, state, factories, &window);
            Some(window)
        }
        Err(error) => {
            let fallback = adw::ApplicationWindow::builder()
                .application(app)
                .default_width(460)
                .default_height(240)
                .title(APP_NAME)
                .build();
            let page = adw::StatusPage::builder()
                .icon_name("dialog-error-symbolic")
                .title(gettext("Riteed Could Not Start"))
                .description(error.body())
                .build();
            fallback.set_content(Some(&page));
            fallback.present();
            dialogs::present_error(&fallback, &error);
            None
        }
    }
}

fn install_transfer_window_handler(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factories: WindowFactories,
    window: &Rc<Window>,
) {
    let app = app.clone();
    let state = Rc::downgrade(state);
    window.set_tab_transfer_window_handler(Rc::new(move || {
        let state = state.upgrade()?;
        let window = create_window(&app, &state, factories.secondary, factories)?;
        window.present();
        Some(window.workspace())
    }));
}

fn register_window(state: &Rc<RefCell<AppState>>, window: &Rc<Window>) {
    let window_weak = Rc::downgrade(window);
    let state_for_destroy = Rc::clone(state);
    window.widget().connect_destroy(move |_| {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        let mut app_state = state_for_destroy.borrow_mut();
        if let Some(focused) = app_state
            .last_focused_window
            .as_ref()
            .and_then(Weak::upgrade)
            && Rc::ptr_eq(&focused, &window)
        {
            app_state.last_focused_window = None;
        }
        app_state
            .windows
            .retain(|candidate| !Rc::ptr_eq(candidate, &window));
    });

    let window_weak = Rc::downgrade(window);
    let state_for_focus = Rc::clone(state);
    window
        .widget()
        .connect_is_active_notify(move |window_widget| {
            if !window_widget.is_active() {
                return;
            }
            if let Some(window) = window_weak.upgrade() {
                state_for_focus.borrow_mut().last_focused_window = Some(Rc::downgrade(&window));
            }
        });

    let mut app_state = state.borrow_mut();
    app_state.last_focused_window = Some(Rc::downgrade(window));
    app_state.windows.push(Rc::clone(window));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accelerators_match_hig_primary_order() {
        let riteed_app = RiteedApp::new();
        let app = riteed_app.application();
        assert_accels(app, "app.new-window", &["<Ctrl>n"]);
        assert_accels(app, "app.new", &["<Ctrl>t"]);
        assert_accels(app, "win.tab-move-to-new-window", &["<Ctrl><Shift>n"]);
        assert_accels(app, "win.find-next", &["<Ctrl>g", "F3"]);
        assert_accels(app, "win.find-prev", &["<Ctrl><Shift>g", "<Shift>F3"]);
        assert_accels(app, "win.refresh-project-tree", &["<Ctrl>r", "F5"]);
        assert_accels(
            app,
            "win.zoom-in",
            &["<Ctrl>plus", "<Ctrl>equal", "<Ctrl>KP_Add"],
        );
        assert_accels(app, "win.project-sidebar-visible", &["F9"]);
        assert!(accel_strings(app, "app.open-recent").is_empty());
        assert!(accel_strings(app, "win.focus-project-sidebar").is_empty());
    }

    fn assert_accels(app: &adw::Application, action: &str, expected: &[&str]) {
        let expected: Vec<String> = expected.iter().map(|accel| String::from(*accel)).collect();
        assert_eq!(accel_strings(app, action), expected);
    }

    fn accel_strings(app: &adw::Application, action: &str) -> Vec<String> {
        app.accels_for_action(action)
            .into_iter()
            .map(|accel| normalize_accel(&accel))
            .collect()
    }

    fn normalize_accel(accel: &str) -> String {
        accel
            .replace("<Control>", "<Ctrl>")
            .replace("<Shift><Ctrl>", "<Ctrl><Shift>")
    }
}
