use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeFailureResponse {
    Cancel,
    ChooseEncoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReopenWithEncodingResponse {
    Cancel,
    Reopen,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvalidCharsSaveResponse {
    Cancel,
    ChooseEncoding,
}

pub fn confirm_decode_failure(
    parent: &impl IsA<gtk4::Widget>,
    document_name: &str,
    on_response: impl Fn(DecodeFailureResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_decode_failure_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Choose a Text Encoding"))
        .body(format!(
            "{}\n\n{}",
            gettext(
                "Automatic text detection or safe conversion was not reliable for this file. Choose an encoding manually to try opening it.",
            ),
            document_name
        ))
        .build();
    dialog.add_responses(&[
        ("cancel", &pgettext("alert response", "Cancel")),
        (
            "choose-encoding",
            &ellipsis_label(pgettext("alert response", "Choose Encoding")),
        ),
    ]);
    dialog.set_default_response(Some("choose-encoding"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "choose-encoding" {
            DecodeFailureResponse::ChooseEncoding
        } else {
            DecodeFailureResponse::Cancel
        };
        on_response(outcome);
    });
}

pub fn confirm_reopen_with_encoding(
    parent: &impl IsA<gtk4::Widget>,
    document_name: &str,
    on_response: impl Fn(ReopenWithEncodingResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_reopen_with_encoding_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Reopen With a Different Encoding?"))
        .body(format!(
            "{}\n\n{}",
            gettext("Reopening with another encoding will discard unsaved changes in this tab."),
            document_name
        ))
        .build();
    dialog.add_responses(&[
        ("cancel", &pgettext("alert response", "Cancel")),
        ("reopen", &pgettext("alert response", "Reopen")),
    ]);
    dialog.set_response_appearance("reopen", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "reopen" {
            ReopenWithEncodingResponse::Reopen
        } else {
            ReopenWithEncodingResponse::Cancel
        };
        on_response(outcome);
    });
}

pub fn confirm_invalid_chars_save(
    parent: &impl IsA<gtk4::Widget>,
    encoding_name: &str,
    on_response: impl Fn(InvalidCharsSaveResponse) + 'static,
) {
    #[cfg(test)]
    if let Some(response) = take_invalid_chars_save_response_for_tests() {
        on_response(response);
        return;
    }

    let dialog = adw::AlertDialog::builder()
        .heading(gettext("Choose a Different Encoding to Save"))
        .body(format!(
            "{}\n\n{}",
            gettext("This document contains characters that cannot be saved in the current text encoding."),
            encoding_name
        ))
        .build();
    dialog.add_responses(&[
        ("cancel", &pgettext("alert response", "Cancel")),
        (
            "choose-encoding",
            &ellipsis_label(pgettext("alert response", "Choose Encoding")),
        ),
    ]);
    dialog.set_default_response(Some("choose-encoding"));
    dialog.set_close_response("cancel");
    dialog.choose(Some(parent), None::<&gio::Cancellable>, move |response| {
        let outcome = if response == "choose-encoding" {
            InvalidCharsSaveResponse::ChooseEncoding
        } else {
            InvalidCharsSaveResponse::Cancel
        };
        on_response(outcome);
    });
}

pub fn choose_encoding(
    parent: &adw::ApplicationWindow,
    title: &str,
    description: &str,
    encodings: &[sourceview5::Encoding],
    current: Option<&sourceview5::Encoding>,
    accept_label: &str,
    on_response: impl FnOnce(Option<sourceview5::Encoding>) + 'static,
) {
    #[cfg(test)]
    if let Some(choice) = take_encoding_choice_for_tests() {
        on_response(match choice {
            EncodingChoice::Selected(charset) => sourceview5::Encoding::from_charset(&charset),
            EncodingChoice::Cancelled => None,
        });
        return;
    }

    present_encoding_dialog(
        parent,
        title,
        description,
        encodings,
        current,
        accept_label,
        on_response,
    );
}

#[cfg(test)]
pub(crate) fn choose_encoding_dialog_for_tests(parent: &adw::ApplicationWindow) -> adw::Dialog {
    let candidates = sourceview5::Encoding::default_candidates();
    present_encoding_dialog(
        parent,
        "Choose",
        "Body",
        &candidates,
        None,
        "Apply",
        |_encoding| {},
    )
}

fn present_encoding_dialog(
    parent: &adw::ApplicationWindow,
    title: &str,
    description: &str,
    encodings: &[sourceview5::Encoding],
    current: Option<&sourceview5::Encoding>,
    accept_label: &str,
    on_response: impl FnOnce(Option<sourceview5::Encoding>) + 'static,
) -> adw::Dialog {
    let model = encodings
        .iter()
        .map(|encoding| encoding.to_str().to_string())
        .collect::<gtk4::StringList>();
    let dropdown = gtk4::DropDown::builder().model(&model).build();
    dropdown.set_enable_search(true);

    if let Some(current_encoding) = current
        && let Some(index) = encodings
            .iter()
            .position(|encoding| encoding == current_encoding)
        && let Ok(selected) = u32::try_from(index)
    {
        dropdown.set_selected(selected);
    }

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(18)
        .margin_bottom(18)
        .margin_end(18)
        .margin_start(18)
        .margin_top(18)
        .build();

    let description_label = gtk4::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .label(description)
        .build();
    content.append(&description_label);
    content.append(&dropdown);

    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::End)
        .spacing(12)
        .build();
    let cancel_button = gtk4::Button::with_label(&pgettext("dialog button", "Cancel"));
    let accept_button = gtk4::Button::with_label(accept_label);
    accept_button.add_css_class("suggested-action");
    button_box.append(&cancel_button);
    button_box.append(&accept_button);
    content.append(&button_box);

    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(420)
        .follows_content_size(true)
        .can_close(true)
        .child(&content)
        .build();

    let state = Rc::new(EncodingDialogState {
        dialog: dialog.downgrade(),
        dropdown,
        encodings: encodings.to_vec(),
        callback: RefCell::new(Some(Box::new(on_response))),
        handled: Cell::new(false),
        #[cfg(test)]
        _leak_canary: crate::dialogs::lifecycle::DialogLeakCanary::new(
            crate::dialogs::lifecycle::DialogLeakKind::Encoding,
        ),
    });

    let weak = Rc::downgrade(&state);
    cancel_button.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        state.cancel();
    });

    let weak = Rc::downgrade(&state);
    accept_button.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        state.accept();
    });

    let state_for_closed = Rc::clone(&state);
    dialog.connect_closed(move |_| {
        state_for_closed.cancel_if_unhandled();
    });

    dialog.present(Some(parent));
    dialog
}

type EncodingCallback = Box<dyn FnOnce(Option<sourceview5::Encoding>)>;

struct EncodingDialogState {
    dialog: gtk4::glib::WeakRef<adw::Dialog>,
    dropdown: gtk4::DropDown,
    encodings: Vec<sourceview5::Encoding>,
    callback: RefCell<Option<EncodingCallback>>,
    handled: Cell<bool>,
    #[cfg(test)]
    _leak_canary: crate::dialogs::lifecycle::DialogLeakCanary,
}

impl EncodingDialogState {
    fn accept(&self) {
        let selected_index = self.dropdown.selected() as usize;
        let selection = self.encodings.get(selected_index).cloned();
        self.handled.set(true);
        self.respond(selection);
        self.close_dialog();
    }

    fn cancel(&self) {
        self.handled.set(true);
        self.respond(None);
        self.close_dialog();
    }

    fn cancel_if_unhandled(&self) {
        if self.handled.get() {
            return;
        }
        self.handled.set(true);
        self.respond(None);
    }

    fn respond(&self, encoding: Option<sourceview5::Encoding>) {
        if let Some(callback) = self.callback.borrow_mut().take() {
            callback(encoding);
        }
    }

    fn close_dialog(&self) {
        if let Some(dialog) = self.dialog.upgrade() {
            let _closed = dialog.close();
        }
    }
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

#[cfg(test)]
fn decode_failure_response_queue()
-> &'static std::sync::Mutex<std::collections::VecDeque<DecodeFailureResponse>> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<DecodeFailureResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_decode_failure_response_for_tests() -> Option<DecodeFailureResponse> {
    match decode_failure_response_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
fn reopen_with_encoding_response_queue()
-> &'static std::sync::Mutex<std::collections::VecDeque<ReopenWithEncodingResponse>> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<ReopenWithEncodingResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_reopen_with_encoding_response_for_tests() -> Option<ReopenWithEncodingResponse> {
    match reopen_with_encoding_response_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
fn invalid_chars_save_response_queue()
-> &'static std::sync::Mutex<std::collections::VecDeque<InvalidCharsSaveResponse>> {
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<InvalidCharsSaveResponse>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_invalid_chars_save_response_for_tests() -> Option<InvalidCharsSaveResponse> {
    match invalid_chars_save_response_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum EncodingChoice {
    Selected(String),
    Cancelled,
}

#[cfg(test)]
fn encoding_choice_queue() -> &'static std::sync::Mutex<std::collections::VecDeque<EncodingChoice>>
{
    use std::collections::VecDeque;
    use std::sync::{Mutex, OnceLock};

    static RESPONSES: OnceLock<Mutex<VecDeque<EncodingChoice>>> = OnceLock::new();
    RESPONSES.get_or_init(|| Mutex::new(VecDeque::new()))
}

#[cfg(test)]
fn take_encoding_choice_for_tests() -> Option<EncodingChoice> {
    match encoding_choice_queue().lock() {
        Ok(mut guard) => guard.pop_front(),
        Err(poisoned) => poisoned.into_inner().pop_front(),
    }
}

#[cfg(test)]
pub(crate) fn queue_decode_failure_responses_for_tests(responses: &[DecodeFailureResponse]) {
    match decode_failure_response_queue().lock() {
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
pub(crate) fn queue_reopen_with_encoding_responses_for_tests(
    responses: &[ReopenWithEncodingResponse],
) {
    match reopen_with_encoding_response_queue().lock() {
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
pub(crate) fn queue_invalid_chars_save_responses_for_tests(responses: &[InvalidCharsSaveResponse]) {
    match invalid_chars_save_response_queue().lock() {
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
pub(crate) fn queue_encoding_choices_for_tests(responses: &[Option<&str>]) {
    match encoding_choice_queue().lock() {
        Ok(mut guard) => {
            guard.clear();
            guard.extend(responses.iter().map(|item| match item {
                Some(charset) => EncodingChoice::Selected((*charset).to_string()),
                None => EncodingChoice::Cancelled,
            }));
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.clear();
            guard.extend(responses.iter().map(|item| match item {
                Some(charset) => EncodingChoice::Selected((*charset).to_string()),
                None => EncodingChoice::Cancelled,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DecodeFailureResponse, EncodingChoice, InvalidCharsSaveResponse,
        ReopenWithEncodingResponse, queue_decode_failure_responses_for_tests,
        queue_encoding_choices_for_tests, queue_invalid_chars_save_responses_for_tests,
        queue_reopen_with_encoding_responses_for_tests, take_decode_failure_response_for_tests,
        take_encoding_choice_for_tests, take_invalid_chars_save_response_for_tests,
        take_reopen_with_encoding_response_for_tests,
    };

    #[test]
    fn test_queues_roundtrip() {
        let _guard = crate::test_support::lock_for_tests();
        queue_decode_failure_responses_for_tests(&[DecodeFailureResponse::ChooseEncoding]);
        assert_eq!(
            take_decode_failure_response_for_tests(),
            Some(DecodeFailureResponse::ChooseEncoding)
        );

        queue_reopen_with_encoding_responses_for_tests(&[ReopenWithEncodingResponse::Reopen]);
        assert_eq!(
            take_reopen_with_encoding_response_for_tests(),
            Some(ReopenWithEncodingResponse::Reopen)
        );

        queue_invalid_chars_save_responses_for_tests(&[InvalidCharsSaveResponse::ChooseEncoding]);
        assert_eq!(
            take_invalid_chars_save_response_for_tests(),
            Some(InvalidCharsSaveResponse::ChooseEncoding)
        );

        queue_encoding_choices_for_tests(&[Some("utf-8"), None]);
        assert_eq!(
            take_encoding_choice_for_tests(),
            Some(EncodingChoice::Selected(String::from("utf-8")))
        );
        assert_eq!(
            take_encoding_choice_for_tests(),
            Some(EncodingChoice::Cancelled)
        );
    }
}
