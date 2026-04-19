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
    let dialog = adw::AlertDialog::builder()
        .heading(error.title())
        .body(error.body())
        .build();
    dialog.add_response("close", &pgettext("alert response", "Close"));
    dialog.set_close_response("close");
    dialog.present(Some(parent));
}

pub fn confirm_unsaved_changes(
    parent: &impl IsA<gtk4::Widget>,
    on_response: impl Fn(UnsavedResponse) + 'static,
) {
    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Save Changes Before Continuing"))
        .body(gettext("The Current Document Has Unsaved Changes."))
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
    dialog.set_version("0.1.0");
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
