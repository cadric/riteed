use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::error::AppError;
use crate::{APP_NAME, REPO_URL};

pub(crate) mod encoding;

pub use encoding::{
    DecodeFailureResponse, InvalidCharsSaveResponse, ReopenWithEncodingResponse, choose_encoding,
    confirm_decode_failure, confirm_invalid_chars_save, confirm_reopen_with_encoding,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsavedResponse {
    Cancel,
    Discard,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalReloadResponse {
    KeepCurrent,
    Reload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaleSaveResponse {
    Cancel,
    SaveAnyway,
}

pub fn present_error(parent: &impl IsA<gtk4::Widget>, error: &AppError) {
    if matches!(error, AppError::Cancelled) {
        return;
    }
    present_message(parent, &error.title(), &error.body());
}

pub fn present_message(parent: &impl IsA<gtk4::Widget>, heading: &str, body: &str) {
    let dialog = adw::AlertDialog::builder()
        .heading(heading)
        .body(body)
        .build();
    dialog.add_response("close", &pgettext("alert response", "Close"));
    dialog.set_close_response("close");
    dialog.present(Some(parent));
}

pub fn confirm_unsaved_changes(
    parent: &impl IsA<gtk4::Widget>,
    document_name: &str,
    on_response: impl Fn(UnsavedResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_unsaved_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Save Changes Before Continuing?"))
        .body(format!(
            "{}\n\n{}",
            gettext("This document has unsaved changes."),
            document_name
        ))
        .build();
    dialog.add_responses(&[
        ("cancel", &pgettext("alert response", "Cancel")),
        ("discard", &pgettext("alert response", "Discard")),
        ("save", &pgettext("alert response", "Save")),
    ]);
    dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("save"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = match response.as_str() {
            "discard" => UnsavedResponse::Discard,
            "save" => UnsavedResponse::Save,
            _ => UnsavedResponse::Cancel,
        };
        on_response(outcome);
    });
}

pub fn confirm_external_reload(
    parent: &impl IsA<gtk4::Widget>,
    document_name: &str,
    on_response: impl Fn(ExternalReloadResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_external_reload_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Reload the Changed File?"))
        .body(format!(
            "{}\n\n{}",
            gettext("This file changed on disk while you also have unsaved changes."),
            document_name
        ))
        .build();
    dialog.add_responses(&[
        ("keep-current", &pgettext("alert response", "Keep Current")),
        ("reload", &pgettext("alert response", "Reload")),
    ]);
    dialog.set_response_appearance("reload", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep-current"));
    dialog.set_close_response("keep-current");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "reload" {
            ExternalReloadResponse::Reload
        } else {
            ExternalReloadResponse::KeepCurrent
        };
        on_response(outcome);
    });
}

pub fn confirm_stale_save(
    parent: &impl IsA<gtk4::Widget>,
    document_name: &str,
    on_response: impl Fn(StaleSaveResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_stale_save_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Overwrite the Changed File?"))
        .body(format!(
            "{}\n\n{}",
            gettext("This file changed on disk. Saving now will replace the external version."),
            document_name
        ))
        .build();
    dialog.add_responses(&[
        ("cancel", &pgettext("alert response", "Cancel")),
        ("save-anyway", &pgettext("alert response", "Save Anyway")),
    ]);
    dialog.set_response_appearance("save-anyway", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "save-anyway" {
            StaleSaveResponse::SaveAnyway
        } else {
            StaleSaveResponse::Cancel
        };
        on_response(outcome);
    });
}

pub fn show_about(parent: &impl IsA<gtk4::Widget>) {
    let dialog = adw::AboutDialog::from_appdata(
        "/io/github/cadric/Riteed/io.github.cadric.Riteed.metainfo.xml",
        None,
    );
    dialog.set_application_name(APP_NAME);
    dialog.set_application_icon("io.github.cadric.Riteed-about");
    dialog.set_version(env!("CARGO_PKG_VERSION"));
    dialog.set_developer_name("cadric");
    dialog.set_website(REPO_URL);
    dialog.set_issue_url(&format!("{REPO_URL}/issues"));
    dialog.set_license_type(gtk4::License::MitX11);
    dialog.present(Some(parent));
}

pub fn show_help(parent: &impl IsA<gtk4::Widget>) {
    let dialog = adw::PreferencesDialog::builder()
        .title(pgettext("help dialog", "Help"))
        .content_width(560)
        .content_height(420)
        .build();

    let overview = adw::PreferencesPage::builder()
        .title(pgettext("help page", "Overview"))
        .build();

    let getting_started = adw::PreferencesGroup::builder()
        .title(pgettext("help section", "Getting Started"))
        .description(gettext(
            "Riteed is a lightweight GNOME editor for text, code, config, and markdown files, with tabs, search, syntax highlighting, session restore, and encoding-aware open and save behavior.",
        ))
        .build();
    getting_started.add(&help_row(
        &pgettext("help row", "Tabs and Files"),
        &gettext(
            "Use New Tab to start another document, and Open to load local plain text files into separate tabs with their saved encoding and line endings.",
        ),
    ));
    getting_started.add(&help_row(
        &pgettext("help row", "Saving Work"),
        &gettext(
            "Riteed tracks unsaved changes per tab, restores saved files from your previous session, and warns before external file changes replace your work.",
        ),
    ));

    let editing = adw::PreferencesGroup::builder()
        .title(pgettext("help section", "Everyday Editing"))
        .build();
    editing.add(&help_row(
        &pgettext("help row", "Search and Replace"),
        &gettext(
            "Press Ctrl+F to search in the selected document, Ctrl+H to show replace, and F3 or Shift+F3 to move between matches.",
        ),
    ));
    editing.add(&help_row(
        &pgettext("help row", "Editor Tools"),
        &gettext(
            "Enable line numbers and the minimap in Preferences when you want more structure while reading longer code or markdown files.",
        ),
    ));
    editing.add(&help_row(
        &pgettext("help row", "More Shortcuts"),
        &gettext(
            "Open Keyboard Shortcuts from the main menu to review the available file, tab, search, and app commands.",
        ),
    ));

    overview.add(&getting_started);
    overview.add(&editing);
    dialog.add(&overview);
    dialog.present(Some(parent));
}

fn help_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    row.set_activatable(false);
    row
}

#[cfg(test)]
fn unsaved_response_queue() -> &'static std::sync::Mutex<std::collections::VecDeque<UnsavedResponse>>
{
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<UnsavedResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_unsaved_response_for_tests() -> Option<UnsavedResponse> {
    match unsaved_response_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
fn external_reload_response_queue()
-> &'static std::sync::Mutex<std::collections::VecDeque<ExternalReloadResponse>> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<ExternalReloadResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_external_reload_response_for_tests() -> Option<ExternalReloadResponse> {
    match external_reload_response_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
fn stale_save_response_queue()
-> &'static std::sync::Mutex<std::collections::VecDeque<StaleSaveResponse>> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<StaleSaveResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_stale_save_response_for_tests() -> Option<StaleSaveResponse> {
    match stale_save_response_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
pub(crate) fn queue_unsaved_responses_for_tests(responses: &[UnsavedResponse]) {
    match unsaved_response_queue().lock() {
        Ok(mut guard) => {
            guard.clear();
            guard.extend(responses.iter().copied());
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.clear();
            guard.extend(responses.iter().copied());
        }
    }
}

#[cfg(test)]
pub(crate) fn queue_external_reload_responses_for_tests(responses: &[ExternalReloadResponse]) {
    match external_reload_response_queue().lock() {
        Ok(mut guard) => {
            guard.clear();
            guard.extend(responses.iter().copied());
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.clear();
            guard.extend(responses.iter().copied());
        }
    }
}

#[cfg(test)]
pub(crate) fn queue_stale_save_responses_for_tests(responses: &[StaleSaveResponse]) {
    match stale_save_response_queue().lock() {
        Ok(mut guard) => {
            guard.clear();
            guard.extend(responses.iter().copied());
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.clear();
            guard.extend(responses.iter().copied());
        }
    }
}
