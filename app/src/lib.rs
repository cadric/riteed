#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod close_flow;
pub mod dialogs;
pub mod document;
pub mod editor_format;
pub mod editor_io;
mod editor_language;
mod editor_monitor;
pub mod editor_search;
pub mod editor_status;
pub mod editor_tab;
mod editor_view;
mod editor_zoom;
pub mod error;
mod project_browser;
mod project_tree;
mod project_tree_model;
mod project_tree_monitor;
pub mod session;
pub mod settings;
pub mod window;
mod window_compare;
mod window_preferences;
mod window_project;
pub mod window_shell;
pub mod workspace;
mod workspace_close;
mod workspace_menu;
mod workspace_monitor;
mod workspace_open;

#[cfg(test)]
mod gtk_tests;
#[cfg(test)]
mod gtk_tests_v4;
#[cfg(test)]
mod gtk_tests_v5;
#[cfg(test)]
mod gtk_tests_v5b;
#[cfg(test)]
mod gtk_tests_v6;
#[cfg(test)]
mod gtk_tests_v7;
#[cfg(test)]
mod gtk_tests_v8;

use std::sync::OnceLock;

use gettextrs::TextDomain;
use gtk4 as gtk;

pub const APP_ID: &str = "io.github.cadric.Riteed";
pub const APP_NAME: &str = "Riteed";
pub const REPO_URL: &str = "https://github.com/cadric/riteed";

static RUNTIME_INIT: OnceLock<()> = OnceLock::new();

pub fn bootstrap_runtime() {
    RUNTIME_INIT.get_or_init(|| {
        gtk::glib::set_prgname(Some(APP_ID));
        gtk::glib::set_application_name(APP_NAME);
        if let Err(_error) = gtk::gio::resources_register_include!("riteed.gresource") {}
        if let Err(_error) = TextDomain::new(APP_ID).init() {}
    });
}

#[must_use]
pub fn run() -> gtk::glib::ExitCode {
    bootstrap_runtime();
    app::RiteedApp::new().run()
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use libadwaita as adw;

    static GTK_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    pub(crate) fn lock_for_tests() -> MutexGuard<'static, ()> {
        let lock = GTK_TEST_LOCK.get_or_init(|| Mutex::new(()));
        match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub(crate) fn init_gtk_for_tests() -> MutexGuard<'static, ()> {
        let guard = lock_for_tests();
        let _gtk = gtk4::init();
        crate::bootstrap_runtime();
        let _adw = adw::init();
        guard
    }
}
