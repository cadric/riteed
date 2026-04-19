#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod close_flow;
pub mod dialogs;
pub mod document;
pub mod editor_io;
pub mod editor_tab;
pub mod error;
pub mod session;
pub mod settings;
pub mod window;
pub mod window_shell;
pub mod workspace;
mod workspace_close;
mod workspace_menu;
mod workspace_open;

#[cfg(test)]
mod gtk_tests;

use std::sync::OnceLock;

use gettextrs::TextDomain;
use gtk4 as gtk;

pub const APP_ID: &str = "io.github.cadric.Riteed";
pub const APP_NAME: &str = "Riteed";
pub const REPO_URL: &str = "https://github.com/cadric/riteed";

static RUNTIME_INIT: OnceLock<()> = OnceLock::new();

pub fn bootstrap_runtime() {
    RUNTIME_INIT.get_or_init(|| {
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

    pub(crate) fn init_gtk_for_tests() -> MutexGuard<'static, ()> {
        let lock = GTK_TEST_LOCK.get_or_init(|| Mutex::new(()));
        let guard = match lock.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let _gtk = gtk4::init();
        crate::bootstrap_runtime();
        let _adw = adw::init();
        guard
    }
}
