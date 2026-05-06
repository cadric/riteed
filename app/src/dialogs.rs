use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::error::AppError;
use crate::{APP_NAME, REPO_URL};

pub(crate) mod encoding;
pub(crate) mod help;
#[cfg(test)]
pub(crate) mod lifecycle;
pub(crate) mod recent_files;

pub use encoding::{
    DecodeFailureResponse, InvalidCharsSaveResponse, ReopenWithEncodingResponse, choose_encoding,
    confirm_decode_failure, confirm_invalid_chars_save, confirm_reopen_with_encoding,
};
pub(crate) use help::show_help;
pub use recent_files::show_recent_files_dialog;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsavedResponse {
    Cancel,
    Discard,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalReloadResponse {
    KeepCurrent,
    Compare,
    Reload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaleSaveResponse {
    Cancel,
    Compare,
    SaveAnyway,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GitDiscardResponse {
    Cancel,
    Discard,
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
        ("compare", &pgettext("alert response", "Compare")),
        ("reload", &pgettext("alert response", "Reload")),
    ]);
    dialog.set_response_appearance("reload", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("keep-current"));
    dialog.set_close_response("keep-current");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = match response.as_str() {
            "compare" => ExternalReloadResponse::Compare,
            "reload" => ExternalReloadResponse::Reload,
            _ => ExternalReloadResponse::KeepCurrent,
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
        ("compare", &pgettext("alert response", "Compare")),
        ("save-anyway", &pgettext("alert response", "Save Anyway")),
    ]);
    dialog.set_response_appearance("save-anyway", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "save-anyway" {
            StaleSaveResponse::SaveAnyway
        } else if response == "compare" {
            StaleSaveResponse::Compare
        } else {
            StaleSaveResponse::Cancel
        };
        on_response(outcome);
    });
}

pub fn confirm_git_discard(
    parent: &impl IsA<gtk4::Widget>,
    file_name: &str,
    on_response: impl Fn(GitDiscardResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_git_discard_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Discard File Changes?"))
        .body(git_discard_body(file_name))
        .build();
    dialog.add_responses(&[
        ("cancel", &pgettext("alert response", "Cancel")),
        ("discard", &pgettext("alert response", "Discard")),
    ]);
    dialog.set_response_appearance("discard", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "discard" {
            GitDiscardResponse::Discard
        } else {
            GitDiscardResponse::Cancel
        };
        on_response(outcome);
    });
}

fn git_discard_body(file_name: &str) -> String {
    git_discard_body_from_template(
        &pgettext(
            "source control discard dialog",
            "Unstaged changes to \"%s\" will be permanently lost and replaced with the version from the Git index. This cannot be undone.",
        ),
        file_name,
    )
}

fn git_discard_body_from_template(template: &str, file_name: &str) -> String {
    template.replace("%s", file_name)
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
fn git_discard_response_queue()
-> &'static std::sync::Mutex<std::collections::VecDeque<GitDiscardResponse>> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<GitDiscardResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_git_discard_response_for_tests() -> Option<GitDiscardResponse> {
    match git_discard_response_queue().lock() {
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

#[cfg(test)]
pub(crate) fn queue_git_discard_responses_for_tests(responses: &[GitDiscardResponse]) {
    match git_discard_response_queue().lock() {
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
mod tests {
    use super::{
        ExternalReloadResponse, GitDiscardResponse, StaleSaveResponse, UnsavedResponse,
        git_discard_body_from_template, queue_external_reload_responses_for_tests,
        queue_git_discard_responses_for_tests, queue_stale_save_responses_for_tests,
        queue_unsaved_responses_for_tests, take_external_reload_response_for_tests,
        take_git_discard_response_for_tests, take_stale_save_response_for_tests,
        take_unsaved_response_for_tests,
    };

    #[test]
    fn git_discard_body_formats_file_name() {
        let body = git_discard_body_from_template("Discard %s now.", "tracked.rs");
        assert_eq!(body, "Discard tracked.rs now.");
    }

    #[test]
    fn git_discard_response_queue_preserves_order() {
        queue_git_discard_responses_for_tests(&[
            GitDiscardResponse::Cancel,
            GitDiscardResponse::Discard,
        ]);
        assert_eq!(
            take_git_discard_response_for_tests(),
            Some(GitDiscardResponse::Cancel)
        );
        assert_eq!(
            take_git_discard_response_for_tests(),
            Some(GitDiscardResponse::Discard)
        );
        assert_eq!(take_git_discard_response_for_tests(), None);
    }

    #[test]
    fn dialog_response_queues_preserve_order() {
        queue_unsaved_responses_for_tests(&[UnsavedResponse::Save, UnsavedResponse::Discard]);
        assert_eq!(
            take_unsaved_response_for_tests(),
            Some(UnsavedResponse::Save)
        );
        assert_eq!(
            take_unsaved_response_for_tests(),
            Some(UnsavedResponse::Discard)
        );

        queue_external_reload_responses_for_tests(&[
            ExternalReloadResponse::Compare,
            ExternalReloadResponse::Reload,
        ]);
        assert_eq!(
            take_external_reload_response_for_tests(),
            Some(ExternalReloadResponse::Compare)
        );
        assert_eq!(
            take_external_reload_response_for_tests(),
            Some(ExternalReloadResponse::Reload)
        );

        queue_stale_save_responses_for_tests(&[
            StaleSaveResponse::Compare,
            StaleSaveResponse::SaveAnyway,
        ]);
        assert_eq!(
            take_stale_save_response_for_tests(),
            Some(StaleSaveResponse::Compare)
        );
        assert_eq!(
            take_stale_save_response_for_tests(),
            Some(StaleSaveResponse::SaveAnyway)
        );
    }
}
