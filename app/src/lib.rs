#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod app;
mod app_chrome;
pub mod close_flow;
mod dialog_shell;
pub mod dialogs;
pub mod document;
mod document_limits;
mod document_portal;
mod document_print;
mod document_print_preview;
mod document_statistics;
mod document_tools;
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
mod find_in_files;
mod git_process;
mod git_status;
mod large_file;
mod markdown;
mod palette_engine;
mod palette_preview;
mod project_browser;
mod project_tree;
mod project_tree_model;
mod project_tree_monitor;
mod runtime_icons;
pub mod session;
pub mod settings;
mod sidebar_host;
mod source_control;
mod source_styles;
pub mod window;
mod window_appearance;
mod window_compare;
mod window_format_menu;
mod window_preferences;
mod window_preferences_large_file;
mod window_project;
pub mod window_shell;
mod window_support;
mod window_theme;
pub mod workspace;
mod workspace_close;
mod workspace_menu;
mod workspace_monitor;
mod workspace_open;

#[cfg(feature = "fuzzing")]
pub mod fuzzing {
    #[must_use]
    pub fn parse_markdown_bytes(bytes: &[u8]) -> usize {
        let input = String::from_utf8_lossy(bytes);
        crate::markdown::parse_document(&input).body.blocks.len()
    }

    #[must_use]
    pub fn split_frontmatter_bytes(bytes: &[u8]) -> usize {
        let input = String::from_utf8_lossy(bytes);
        crate::markdown::fuzz_split_frontmatter(&input)
    }

    #[must_use]
    pub fn unsupported_diagnostics_bytes(bytes: &[u8]) -> usize {
        let input = String::from_utf8_lossy(bytes);
        crate::markdown::fuzz_unsupported_diagnostics(&input)
    }

    #[must_use]
    pub fn parse_git_status_bytes(bytes: &[u8]) -> usize {
        crate::git_status::parse_status(bytes).entries.len()
    }

    #[must_use]
    pub fn compute_diff_bytes(bytes: &[u8]) -> (bool, usize) {
        let split = bytes
            .iter()
            .position(|byte| *byte == 0)
            .map_or(bytes.len().saturating_div(2), |index| index);
        let reference = String::from_utf8_lossy(&bytes[..split]);
        let current_start = split.saturating_add(usize::from(split < bytes.len()));
        let current = String::from_utf8_lossy(&bytes[current_start..]);
        crate::editor_tab::fuzz_compute_diff(&reference, &current)
    }
}

#[cfg(test)]
mod gtk_test_fixtures;
#[cfg(test)]
mod gtk_tests;
#[cfg(test)]
mod gtk_tests_boundaries;
#[cfg(test)]
mod gtk_tests_dialog_lifecycle;
#[cfg(test)]
mod gtk_tests_document_close;
#[cfg(test)]
mod gtk_tests_document_reads;
#[cfg(test)]
mod gtk_tests_lifecycle;
#[cfg(test)]
mod gtk_tests_markdown;
#[cfg(test)]
mod gtk_tests_tabs;
#[cfg(test)]
mod gtk_tests_v10;
#[cfg(test)]
mod gtk_tests_v11;
#[cfg(test)]
mod gtk_tests_v11_git;
#[cfg(test)]
mod gtk_tests_v12;
#[cfg(test)]
mod gtk_tests_v13;
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
#[cfg(test)]
mod gtk_tests_v9;

use std::sync::OnceLock;

use gettextrs::TextDomain;
use gtk4 as gtk;

use settings::AppLanguage;

pub const APP_ID: &str = "io.github.cadric.Riteed";
pub const APP_NAME: &str = "Riteed";
pub const REPO_URL: &str = "https://github.com/cadric/riteed";

pub type RuntimeInitResult = Result<RuntimeInitReport, RuntimeInitError>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeInitReport {
    pub gettext_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeInitError {
    ResourceRegistration { message: String },
}

static RUNTIME_INIT: OnceLock<RuntimeInitResult> = OnceLock::new();

/// Initializes process metadata, bundled resources, and gettext.
///
/// # Errors
///
/// Returns [`RuntimeInitError::ResourceRegistration`] when the compiled
/// `GResource` bundle cannot be registered. Gettext initialization failures are
/// reported in [`RuntimeInitReport::gettext_error`] and do not fail startup.
#[must_use = "startup initialization errors must be handled"]
pub fn bootstrap_runtime() -> RuntimeInitResult {
    RUNTIME_INIT
        .get_or_init(|| {
            gtk::glib::set_prgname(Some(APP_ID));
            gtk::glib::set_application_name(APP_NAME);
            let resource = gtk::gio::resources_register_include!("riteed.gresource")
                .map_err(|error| error.to_string());
            let gettext = match settings::startup_language_preference() {
                AppLanguage::System => TextDomain::new(APP_ID).init(),
                AppLanguage::English => TextDomain::new(APP_ID).locale("C").init(),
                AppLanguage::Danish => TextDomain::new(APP_ID).locale("da_DK.UTF-8").init(),
            }
            .map(|_locales| ())
            .map_err(|error| error.to_string());
            build_runtime_report(resource, gettext)
        })
        .clone()
}

fn build_runtime_report(
    resource: Result<(), String>,
    gettext: Result<(), String>,
) -> RuntimeInitResult {
    resource.map_err(|message| RuntimeInitError::ResourceRegistration { message })?;
    Ok(RuntimeInitReport {
        gettext_error: gettext.err(),
    })
}

#[must_use]
pub fn run() -> gtk::glib::ExitCode {
    match bootstrap_runtime() {
        Ok(report) => {
            if let Some(error) = report.gettext_error.as_deref() {
                gtk::glib::g_warning!(APP_ID, "Gettext initialization failed: {}", error);
            }
            app::RiteedApp::new().run()
        }
        Err(RuntimeInitError::ResourceRegistration { message }) => {
            gtk::glib::g_critical!(APP_ID, "Resource registration failed: {}", message);
            gtk::glib::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::{RuntimeInitError, RuntimeInitReport, build_runtime_report};

    #[test]
    fn resource_registration_failure_is_hard_error() {
        assert_eq!(
            build_runtime_report(
                Err(String::from("missing resource")),
                Err(String::from("gettext unavailable")),
            ),
            Err(RuntimeInitError::ResourceRegistration {
                message: String::from("missing resource"),
            })
        );
    }

    #[test]
    fn gettext_failure_is_reported_but_nonfatal() {
        assert_eq!(
            build_runtime_report(Ok(()), Err(String::from("gettext unavailable"))),
            Ok(RuntimeInitReport {
                gettext_error: Some(String::from("gettext unavailable")),
            })
        );
    }

    #[test]
    fn successful_runtime_init_has_empty_report() {
        assert_eq!(
            build_runtime_report(Ok(()), Ok(())),
            Ok(RuntimeInitReport {
                gettext_error: None,
            })
        );
    }
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
        let init = crate::bootstrap_runtime();
        assert!(init.is_ok(), "bootstrap_runtime failed: {init:?}");
        let _adw = adw::init();
        guard
    }
}
