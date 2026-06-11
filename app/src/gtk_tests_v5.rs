use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::dialogs::encoding::{
    DecodeFailureResponse, InvalidCharsSaveResponse, ReopenWithEncodingResponse,
    queue_decode_failure_responses_for_tests, queue_encoding_choices_for_tests,
    queue_invalid_chars_save_responses_for_tests, queue_reopen_with_encoding_responses_for_tests,
};
use crate::dialogs::{self, StaleSaveResponse};
use crate::editor_format::LineEndingMode;
use crate::editor_monitor::ExternalFileEvent;
use crate::gtk_tests::{build_window, drain_events, spin_until, write_temp_file};
use crate::window::Window;
use crate::workspace::OpenSource;

fn exercise_encoding_dialog_test_hooks(window: &Rc<Window>) {
    queue_decode_failure_responses_for_tests(&[DecodeFailureResponse::ChooseEncoding]);
    let decode = Rc::new(RefCell::new(None));
    crate::dialogs::confirm_decode_failure(window.widget(), "broken.txt", {
        let decode = decode.clone();
        move |response| {
            decode.replace(Some(response));
        }
    });
    assert_eq!(decode.take(), Some(DecodeFailureResponse::ChooseEncoding));

    queue_reopen_with_encoding_responses_for_tests(&[ReopenWithEncodingResponse::Reopen]);
    let reopen = Rc::new(RefCell::new(None));
    crate::dialogs::confirm_reopen_with_encoding(window.widget(), "dirty.txt", {
        let reopen = reopen.clone();
        move |response| {
            reopen.replace(Some(response));
        }
    });
    assert_eq!(reopen.take(), Some(ReopenWithEncodingResponse::Reopen));

    queue_invalid_chars_save_responses_for_tests(&[InvalidCharsSaveResponse::ChooseEncoding]);
    let invalid = Rc::new(RefCell::new(None));
    crate::dialogs::confirm_invalid_chars_save(window.widget(), "ISO-8859-1", {
        let invalid = invalid.clone();
        move |response| {
            invalid.replace(Some(response));
        }
    });
    assert_eq!(
        invalid.take(),
        Some(InvalidCharsSaveResponse::ChooseEncoding)
    );

    let candidates = sourceview5::Encoding::default_candidates();
    queue_encoding_choices_for_tests(&[Some("UTF-8"), None]);
    let selected = Rc::new(RefCell::new(None));
    crate::dialogs::choose_encoding(
        window.widget(),
        "Choose",
        "Body",
        &candidates,
        None,
        "Apply",
        {
            let selected = selected.clone();
            move |encoding| {
                selected.replace(encoding.map(|encoding| encoding.charset().to_string()));
            }
        },
    );
    assert_eq!(selected.take().as_deref(), Some("UTF-8"));

    let cancelled = Rc::new(RefCell::new(Some(String::from("pending"))));
    crate::dialogs::choose_encoding(
        window.widget(),
        "Choose",
        "Body",
        &candidates,
        None,
        "Apply",
        {
            let cancelled = cancelled.clone();
            move |encoding| {
                cancelled.replace(encoding.map(|encoding| encoding.charset().to_string()));
            }
        },
    );
    assert_eq!(cancelled.take(), None);
}

fn exercise_manual_open_and_reopen(window: &Rc<Window>, latin1_path: &Path, latin1_uri: &str) {
    queue_decode_failure_responses_for_tests(&[DecodeFailureResponse::ChooseEncoding]);
    queue_encoding_choices_for_tests(&[Some("ISO-8859-1")]);
    window.request_open_files(vec![gio::File::for_path(latin1_path)], OpenSource::AppOpen);
    spin_until("latin1 open with manual encoding", || {
        window.selected_saved_uri_for_tests() == latin1_uri
            && window.selected_text_for_tests() == "héj"
            && window.status_format_summary_for_tests() == "ISO-8859-1 · LF"
    });

    window.set_selected_text_for_tests("changed");
    queue_encoding_choices_for_tests(&[Some("UTF-8"), Some("ISO-8859-1")]);
    queue_reopen_with_encoding_responses_for_tests(&[ReopenWithEncodingResponse::Reopen]);
    queue_decode_failure_responses_for_tests(&[DecodeFailureResponse::ChooseEncoding]);
    window.request_selected_encoding_from_format_menu_for_tests();
    spin_until("dirty reopen with fallback encoding", || {
        window.selected_text_for_tests() == "héj"
            && window.status_format_summary_for_tests() == "ISO-8859-1 · LF"
            && window.status_labels_for_tests().1.is_empty()
    });
}

fn exercise_save_and_stale(window: &Rc<Window>, ascii_path: &Path, ascii_uri: &str) {
    window.request_open_files(vec![gio::File::for_path(ascii_path)], OpenSource::AppOpen);
    spin_until("ascii file opens", || {
        window.selected_saved_uri_for_tests() == ascii_uri
            && window.selected_text_for_tests() == "plain"
            && window.status_format_summary_for_tests() == "UTF-8 · LF"
    });
    queue_encoding_choices_for_tests(&[Some("ISO-8859-1")]);
    window.request_selected_encoding_from_format_menu_for_tests();
    spin_until("ascii reopen switches encoding", || {
        window.selected_text_for_tests() == "plain"
            && window.status_format_summary_for_tests() == "ISO-8859-1 · LF"
    });

    window.choose_selected_line_ending_from_format_menu_for_tests(LineEndingMode::CrLf);
    spin_until("preferences choose crlf", || {
        window.status_format_summary_for_tests() == "ISO-8859-1 · CRLF"
            && window.line_ending_action_state_for_tests() == "crlf"
    });
    window.request_save();
    spin_until("status line ending saves crlf", || {
        fs::read(ascii_path).ok().as_deref() == Some(b"plain\r\n")
            && window.status_format_summary_for_tests() == "ISO-8859-1 · CRLF"
            && window.status_labels_for_tests().1.is_empty()
    });

    window.choose_selected_line_ending_from_format_menu_for_tests(LineEndingMode::Cr);
    spin_until("preferences choose cr", || {
        window.status_format_summary_for_tests() == "ISO-8859-1 · CR"
            && window.line_ending_action_state_for_tests() == "cr"
    });
    window.request_save();
    spin_until("status line ending saves cr", || {
        fs::read(ascii_path).ok().as_deref() == Some(b"plain\r")
            && window.status_format_summary_for_tests() == "ISO-8859-1 · CR"
            && window.status_labels_for_tests().1.is_empty()
    });

    window.choose_selected_line_ending_from_format_menu_for_tests(LineEndingMode::Lf);
    spin_until("preferences choose lf", || {
        window.status_format_summary_for_tests() == "ISO-8859-1 · LF"
            && window.line_ending_action_state_for_tests() == "lf"
    });
    window.request_save();
    spin_until("status line ending restores lf", || {
        fs::read(ascii_path).ok().as_deref() == Some(b"plain\n")
            && window.status_format_summary_for_tests() == "ISO-8859-1 · LF"
            && window.status_labels_for_tests().1.is_empty()
    });

    window.set_selected_text_for_tests("emoji 😀");
    queue_invalid_chars_save_responses_for_tests(&[InvalidCharsSaveResponse::ChooseEncoding]);
    queue_encoding_choices_for_tests(&[Some("UTF-8")]);
    window.request_save();
    spin_until("invalid chars save chooses utf8", || {
        fs::read_to_string(ascii_path).ok().as_deref() == Some("emoji 😀\n")
            && window.status_format_summary_for_tests() == "UTF-8 · LF"
    });

    let write_result = fs::write(ascii_path, "disk version\n");
    assert!(write_result.is_ok());
    window.inject_external_event_for_tests(ascii_uri, ExternalFileEvent::ContentPossiblyChanged);
    drain_events(12);
    window.set_selected_text_for_tests("local save");
    dialogs::queue_stale_save_responses_for_tests(&[StaleSaveResponse::SaveAnyway]);
    window.request_save();
    spin_until("stale save avoids double prompt", || {
        fs::read_to_string(ascii_path).ok().as_deref() == Some("local save\n")
    });
}

pub(crate) fn exercise_v5_format_io(test_app: &adw::Application) {
    let latin1_path = write_temp_file("riteed-v5-latin1.txt", b"h\xe9j\n");
    let latin1_uri = gio::File::for_path(&latin1_path).uri().to_string();
    let ascii_path = write_temp_file("riteed-v5-ascii.txt", b"plain\n");
    let ascii_uri = gio::File::for_path(&ascii_path).uri().to_string();

    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };

    exercise_encoding_dialog_test_hooks(&window);
    exercise_manual_open_and_reopen(&window, &latin1_path, &latin1_uri);
    exercise_save_and_stale(&window, &ascii_path, &ascii_uri);

    let _removed = fs::remove_file(latin1_path);
    let _removed = fs::remove_file(ascii_path);
}
