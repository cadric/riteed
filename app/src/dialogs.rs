use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::error::AppError;
use crate::{APP_ID, APP_NAME, REPO_URL};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsavedResponse {
    Cancel,
    Discard,
    Save,
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

pub fn show_about(parent: &impl IsA<gtk4::Widget>) {
    let dialog = adw::AboutDialog::from_appdata(
        "/io/github/cadric/Riteed/io.github.cadric.Riteed.metainfo.xml",
        None,
    );
    dialog.set_application_name(APP_NAME);
    dialog.set_application_icon(APP_ID);
    dialog.set_version(env!("CARGO_PKG_VERSION"));
    dialog.set_developer_name("cadric");
    dialog.set_website(REPO_URL);
    dialog.set_issue_url(&format!("{REPO_URL}/issues"));
    dialog.set_license_type(gtk4::License::MitX11);
    dialog.present(Some(parent));
}

pub fn launch_help(parent: &impl IsA<gtk4::Window>, on_error: impl Fn(AppError) + 'static) {
    let launcher = gtk4::UriLauncher::new(REPO_URL);
    launcher.launch(Some(parent), None::<&gio::Cancellable>, move |result| {
        if let Err(error) = result {
            on_error(AppError::HelpLaunchFailed(error.message().to_string()));
        }
    });
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
