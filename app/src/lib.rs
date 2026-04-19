#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
pub mod dialogs;
pub mod document;
pub mod error;
pub mod settings;
pub mod window;

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
