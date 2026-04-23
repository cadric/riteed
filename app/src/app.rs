use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs;
use crate::window::Window;
use crate::workspace::OpenSource;
use crate::{APP_ID, APP_NAME};

pub(crate) struct AppState {
    pub(crate) window: Option<Rc<Window>>,
    pub(crate) session_restore_attempted: bool,
}

type WindowFactory = fn(&adw::Application) -> Result<Rc<Window>, crate::error::AppError>;

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
            window: None,
            session_restore_attempted: false,
        }));

        install_accels(&app);
        install_actions(&app, &state, Window::new);
        install_lifecycle(&app, &state, Window::new);

        Self { app, state }
    }

    #[must_use]
    pub fn run(&self) -> gtk4::glib::ExitCode {
        let _keep_state_alive = self.state.borrow().window.clone();
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
    install_actions(app, state, Window::new_for_tests);
    install_lifecycle(app, state, Window::new_for_tests);
}

#[cfg(test)]
pub(crate) fn ensure_window_for_tests(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
) -> Option<Rc<Window>> {
    ensure_window(app, state, Window::new_for_tests).map(|(window, _created)| window)
}

fn install_accels(app: &adw::Application) {
    app.set_accels_for_action("app.new", &["<Ctrl>n"]);
    app.set_accels_for_action("app.open", &["<Ctrl>o"]);
    app.set_accels_for_action("app.open-recent", &[]);
    app.set_accels_for_action("win.save", &["<Ctrl>s"]);
    app.set_accels_for_action("win.save-as", &["<Ctrl><Shift>s"]);
    app.set_accels_for_action("win.close", &["<Ctrl>w"]);
    app.set_accels_for_action("win.search", &["<Ctrl>f"]);
    app.set_accels_for_action("win.replace", &["<Ctrl>h"]);
    app.set_accels_for_action("win.find-next", &["F3"]);
    app.set_accels_for_action("win.find-prev", &["<Shift>F3"]);
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

fn install_actions(app: &adw::Application, state: &Rc<RefCell<AppState>>, factory: WindowFactory) {
    let new_action = gio::SimpleAction::new("new", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    new_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factory) {
            if created {
                window.ensure_default_tab();
            } else {
                window.request_new();
            }
            window.present();
        }
    });
    app.add_action(&new_action);

    let open_action = gio::SimpleAction::new("open", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    open_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factory) {
            if created {
                window.ensure_default_tab();
            }
            window.present();
            window.request_open_dialog();
        }
    });
    app.add_action(&open_action);

    let open_recent_action = gio::SimpleAction::new("open-recent", Some(glib::VariantTy::STRING));
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    open_recent_action.connect_activate(move |_, parameter| {
        let Some(uri) = parameter.and_then(glib::Variant::str) else {
            return;
        };
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factory) {
            if created {
                window.ensure_default_tab();
            }
            window.present();
            window.request_open_recent(uri);
        }
    });
    app.add_action(&open_recent_action);

    let preferences_action = gio::SimpleAction::new("preferences", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    preferences_action.connect_activate(move |_, _| {
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factory) {
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
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factory) {
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
        if let Some((window, created)) = ensure_window(&app_clone, &state_clone, factory) {
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
        if let Some((window, _created)) = ensure_window(&app_clone, &state_clone, factory) {
            window.widget().close();
        } else {
            app_clone.quit();
        }
    });
    app.add_action(&quit_action);
}

fn install_lifecycle(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factory: WindowFactory,
) {
    let state_clone = Rc::clone(state);
    app.connect_activate(move |app| {
        if let Some((window, _created)) = ensure_window(app, &state_clone, factory) {
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
        if let Some((window, created)) = ensure_window(app, &state_clone, factory) {
            if created {
                window.ensure_default_tab();
            }
            let items = files.to_vec();
            if items.is_empty() {
                window.present();
            } else {
                window.request_open_files(items, OpenSource::AppOpen);
                window.present();
            }
        }
    });
}

fn ensure_window(
    app: &adw::Application,
    state: &Rc<RefCell<AppState>>,
    factory: WindowFactory,
) -> Option<(Rc<Window>, bool)> {
    if let Some(window) = state.borrow().window.clone() {
        return Some((window, false));
    }

    match factory(app) {
        Ok(window) => {
            let state_clone = Rc::clone(state);
            let window_clone = Rc::clone(&window);
            window.widget().connect_destroy(move |_| {
                let should_clear = state_clone
                    .borrow()
                    .window
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &window_clone));
                if should_clear {
                    state_clone.borrow_mut().window = None;
                }
            });
            state.borrow_mut().window = Some(Rc::clone(&window));
            Some((window, true))
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
