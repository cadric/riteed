use std::cell::RefCell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs;
use crate::window::Window;
use crate::{APP_ID, APP_NAME};

#[derive(Clone)]
pub struct RiteedApp {
    app: adw::Application,
    window: Rc<RefCell<Option<Rc<Window>>>>,
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
        let state = Rc::new(RefCell::new(None));

        install_accels(&app);
        install_actions(&app, &state);
        install_lifecycle(&app, &state);

        Self { app, window: state }
    }

    #[must_use]
    pub fn run(&self) -> gtk4::glib::ExitCode {
        let _keep_state_alive = self.window.borrow().clone();
        self.app.run()
    }

    #[cfg(test)]
    pub(crate) fn application(&self) -> &adw::Application {
        &self.app
    }
}

#[cfg(test)]
pub(crate) fn install_for_tests(app: &adw::Application, state: &Rc<RefCell<Option<Rc<Window>>>>) {
    install_accels(app);
    install_actions(app, state);
    install_lifecycle(app, state);
}

#[cfg(test)]
pub(crate) fn ensure_window_for_tests(
    app: &adw::Application,
    state: &Rc<RefCell<Option<Rc<Window>>>>,
) -> Option<Rc<Window>> {
    ensure_window(app, state)
}

fn install_accels(app: &adw::Application) {
    app.set_accels_for_action("app.new", &["<Ctrl>n"]);
    app.set_accels_for_action("app.open", &["<Ctrl>o"]);
    app.set_accels_for_action("win.save", &["<Ctrl>s"]);
    app.set_accels_for_action("win.save-as", &["<Ctrl><Shift>s"]);
    app.set_accels_for_action("win.close", &["<Ctrl>w"]);
    app.set_accels_for_action("app.preferences", &["<Ctrl>comma"]);
    app.set_accels_for_action("win.show-help-overlay", &["<Ctrl>question"]);
    app.set_accels_for_action("app.help", &["F1"]);
    app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
}

fn install_actions(app: &adw::Application, state: &Rc<RefCell<Option<Rc<Window>>>>) {
    let new_action = gio::SimpleAction::new("new", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    new_action.connect_activate(move |_, _| {
        if let Some(window) = ensure_window(&app_clone, &state_clone) {
            window.request_new();
        }
    });
    app.add_action(&new_action);

    let open_action = gio::SimpleAction::new("open", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    open_action.connect_activate(move |_, _| {
        if let Some(window) = ensure_window(&app_clone, &state_clone) {
            window.request_open_dialog();
        }
    });
    app.add_action(&open_action);

    let preferences_action = gio::SimpleAction::new("preferences", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    preferences_action.connect_activate(move |_, _| {
        if let Some(window) = ensure_window(&app_clone, &state_clone) {
            window.show_preferences();
        }
    });
    app.add_action(&preferences_action);

    let help_action = gio::SimpleAction::new("help", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    help_action.connect_activate(move |_, _| {
        if let Some(window) = ensure_window(&app_clone, &state_clone) {
            window.show_help();
        }
    });
    app.add_action(&help_action);

    let about_action = gio::SimpleAction::new("about", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    about_action.connect_activate(move |_, _| {
        if let Some(window) = ensure_window(&app_clone, &state_clone) {
            window.show_about();
        }
    });
    app.add_action(&about_action);

    let quit_action = gio::SimpleAction::new("quit", None);
    let app_clone = app.clone();
    let state_clone = Rc::clone(state);
    quit_action.connect_activate(move |_, _| {
        if let Some(window) = ensure_window(&app_clone, &state_clone) {
            window.widget().close();
        } else {
            app_clone.quit();
        }
    });
    app.add_action(&quit_action);
}

fn install_lifecycle(app: &adw::Application, state: &Rc<RefCell<Option<Rc<Window>>>>) {
    let state_clone = Rc::clone(state);
    app.connect_activate(move |app| {
        if let Some(window) = ensure_window(app, &state_clone) {
            window.present();
        }
    });

    let state_clone = Rc::clone(state);
    app.connect_open(move |app, files, _hint| {
        if let Some(window) = ensure_window(app, &state_clone) {
            if let Some(file) = files.first() {
                window.request_open_file(file.clone());
            } else {
                window.present();
            }
        }
    });
}

fn ensure_window(
    app: &adw::Application,
    state: &Rc<RefCell<Option<Rc<Window>>>>,
) -> Option<Rc<Window>> {
    if let Some(window) = state.borrow().clone() {
        return Some(window);
    }

    match Window::new(app) {
        Ok(window) => {
            let state_clone = Rc::clone(state);
            let window_clone = Rc::clone(&window);
            window.widget().connect_destroy(move |_| {
                let should_clear = state_clone
                    .borrow()
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &window_clone));
                if should_clear {
                    *state_clone.borrow_mut() = None;
                }
            });
            *state.borrow_mut() = Some(Rc::clone(&window));
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
