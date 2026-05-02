use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gtk4 as gtk;
use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::app::{AppState, RiteedApp, ensure_window_for_tests, install_for_tests};
use crate::dialogs::{self, UnsavedResponse};
use crate::editor_format::LineEndingMode;
use crate::error::AppError;
use crate::settings::{AppSettings, ThemePreference};
use crate::window::Window;
use crate::workspace::OpenSource;

pub(crate) fn spin_until(label: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        while glib::MainContext::default().iteration(false) {}
        if done() {
            return;
        }
        let _source = glib::timeout_add_local_once(Duration::from_millis(10), || {});
        let _dispatched = glib::MainContext::default().iteration(true);
    }
    assert!(done(), "{label}");
}

pub(crate) fn drain_events(rounds: usize) {
    for _ in 0..rounds {
        while glib::MainContext::default().iteration(false) {}
    }
}

pub(crate) fn wait_millis(label: &str, millis: u64) {
    let fired = std::rc::Rc::new(std::cell::Cell::new(false));
    let fired_for_callback = std::rc::Rc::clone(&fired);
    let _source = glib::timeout_add_local_once(Duration::from_millis(millis), move || {
        fired_for_callback.set(true);
    });
    spin_until(label, || fired.get());
}

pub(crate) fn build_window(app: &adw::Application) -> Option<std::rc::Rc<Window>> {
    Window::new_for_tests(app).ok()
}

pub(crate) fn build_window_with_settings(
    app: &adw::Application,
    settings: AppSettings,
) -> Option<std::rc::Rc<Window>> {
    Window::new_with_settings_for_tests(app, settings).ok()
}

pub(crate) fn write_temp_file(name: &str, contents: &[u8]) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(name);
    let _removed = fs::remove_file(&path);
    let write_result = fs::write(&path, contents);
    assert!(write_result.is_ok());
    path
}

pub(crate) fn atomic_replace_file(path: &std::path::Path, contents: &[u8]) {
    let replacement = path.with_extension("tmp-replace");
    let _removed = fs::remove_file(&replacement);
    assert!(fs::write(&replacement, contents).is_ok());
    assert!(fs::rename(&replacement, path).is_ok());
}

fn assert_settings_apply() {
    let settings = AppSettings::new_for_tests();
    let text_view = gtk::TextView::new();
    settings.apply_word_wrap(&text_view);
    assert_eq!(text_view.wrap_mode(), gtk::WrapMode::None);
    settings.set_word_wrap(true);
    settings.apply_word_wrap(&text_view);
    assert_eq!(text_view.wrap_mode(), gtk::WrapMode::WordChar);
    settings.set_theme(ThemePreference::Dark);
    settings.apply_theme();
    assert_eq!(
        format!("{:?}", adw::StyleManager::default().color_scheme()),
        "ForceDark"
    );
    assert!(!settings.show_line_numbers());
    settings.set_show_line_numbers(true);
    assert!(settings.show_line_numbers());

    let buffer = sourceview5::Buffer::builder().build();
    settings.apply_source_style_scheme(&buffer);
    let manager = sourceview5::StyleSchemeManager::default();
    if manager.scheme("Adwaita-dark").is_some() {
        assert_eq!(
            buffer.style_scheme().map(|scheme| scheme.id().to_string()),
            Some(String::from("Adwaita-dark"))
        );
    } else {
        assert!(buffer.style_scheme().is_some());
    }
    settings.set_theme(ThemePreference::Light);
    settings.apply_theme();
    assert_eq!(
        format!("{:?}", adw::StyleManager::default().color_scheme()),
        "ForceLight"
    );
}

fn assert_app_actions_exist() {
    let riteed_app = RiteedApp::new();
    let app = riteed_app.application();
    assert!(app.lookup_action("new").is_some());
    assert!(app.lookup_action("new-window").is_some());
    assert!(app.lookup_action("open").is_some());
    assert!(app.lookup_action("open-recent").is_some());
    assert!(app.lookup_action("preferences").is_some());
    assert!(app.lookup_action("help").is_some());
    assert!(app.lookup_action("about").is_some());
    assert!(app.lookup_action("quit").is_some());
}

fn exercise_window_tab_flow(test_app: &adw::Application) {
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };

    assert!(crate::window_shell::builder_object_for_tests().is_err());
    window.ensure_default_tab();
    assert_eq!(window.close_request_for_tests(), glib::Propagation::Proceed);
    assert_eq!(window.tab_count_for_tests(), 1);
    assert_eq!(window.size_for_tests(), (840, 620));
    assert!(window.shortcuts_enabled_for_tests());

    let first_path = write_temp_file("riteed-v2-first.txt", b"alpha");
    let second_path = write_temp_file("riteed-v2-second.txt", b"beta");
    let third_path = write_temp_file("riteed-v2-third.txt", b"gamma");

    window.request_open_files(
        vec![
            gio::File::for_path(&first_path),
            gio::File::for_path(&second_path),
        ],
        OpenSource::AppOpen,
    );
    spin_until("open multiple files", || {
        window.tab_count_for_tests() == 2 && window.session_files_for_tests().len() == 2
    });
    window.request_open_files(vec![gio::File::for_path(&first_path)], OpenSource::AppOpen);
    drain_events(8);
    assert_eq!(window.tab_count_for_tests(), 2);

    window.set_selected_text_for_tests("beta-updated");
    window.request_save();
    spin_until("save selected tab", || {
        fs::read_to_string(&first_path).ok().as_deref() == Some("beta-updated")
            || fs::read_to_string(&second_path).ok().as_deref() == Some("beta-updated")
    });

    window.request_new();
    spin_until("new tab", || window.tab_count_for_tests() == 3);
    assert!(window.selected_saved_uri_for_tests().is_empty());

    let first_uri = gio::File::for_path(&first_path).uri().to_string();
    window.request_open_recent(&first_uri);
    drain_events(8);
    assert_eq!(window.tab_count_for_tests(), 3);
    assert!(window.recent_files_for_tests().contains(&first_uri));
    assert!(window.selected_title_for_tests().contains("first.txt"));

    let third_uri = gio::File::for_path(&third_path).uri().to_string();
    window.request_open_files(vec![gio::File::for_path(&third_path)], OpenSource::Drop);
    spin_until("drop open", || {
        window.tab_count_for_tests() == 4 && window.recent_files_for_tests().contains(&third_uri)
    });
    assert!(window.recent_files_for_tests().contains(&third_uri));

    window.request_open_recent(&third_uri);
    drain_events(8);
    assert_eq!(window.selected_saved_uri_for_tests(), third_uri);
    assert!(window.reorder_selected_to_first_for_tests());
    assert_eq!(
        window.session_files_for_tests().first().map(String::as_str),
        Some(third_uri.as_str())
    );

    window.request_new();
    spin_until("second untitled tab", || window.tab_count_for_tests() == 5);
    window.set_selected_text_for_tests("dirty");
    assert_eq!(window.selected_text_for_tests(), "dirty");
    dialogs::queue_unsaved_responses_for_tests(&[]);
    window.request_close_current_tab();
    drain_events(8);
    assert_eq!(window.tab_count_for_tests(), 5);
    assert_eq!(window.close_request_for_tests(), glib::Propagation::Stop);

    let response = Arc::new(Mutex::new(None));
    let response_clone = Arc::clone(&response);
    dialogs::confirm_unsaved_changes(window.widget(), "Dirty Tab", move |choice| {
        let lock = response_clone.lock();
        match lock {
            Ok(mut guard) => *guard = Some(choice),
            Err(poisoned) => *poisoned.into_inner() = Some(choice),
        }
    });
    dialogs::present_error(window.widget(), &AppError::Internal(String::from("error")));
    dialogs::present_error(window.widget(), &AppError::Cancelled);
    window.show_help();

    let _removed = fs::remove_file(first_path);
    let _removed = fs::remove_file(second_path);
    let _removed = fs::remove_file(third_path);
}

fn exercise_restore_and_recent_pruning(test_app: &adw::Application) {
    let first_path = write_temp_file("riteed-restore-one.txt", b"one");
    let second_path = write_temp_file("riteed-restore-two.txt", b"two");
    let missing_path = std::env::temp_dir().join("riteed-missing-recent.txt");
    let _removed = fs::remove_file(&missing_path);

    let first_uri = gio::File::for_path(&first_path).uri().to_string();
    let second_uri = gio::File::for_path(&second_path).uri().to_string();
    let missing_uri = gio::File::for_path(&missing_path).uri().to_string();

    let restore_settings = AppSettings::new_for_tests();
    restore_settings.set_recent_files(std::slice::from_ref(&first_uri));
    restore_settings.set_session_files(&[
        first_uri.clone(),
        missing_uri.clone(),
        second_uri.clone(),
    ]);
    restore_settings.set_session_selected_file(&second_uri);

    let restore_window = build_window_with_settings(test_app, restore_settings);
    assert!(restore_window.is_some());
    let Some(restore_window) = restore_window else {
        return;
    };
    restore_window.restore_session();
    spin_until("restore session", || {
        restore_window.tab_count_for_tests() == 2
            && restore_window.session_files_for_tests().len() == 2
            && restore_window.selected_saved_uri_for_tests() == second_uri
    });
    assert_eq!(restore_window.selected_saved_uri_for_tests(), second_uri);
    assert_eq!(
        restore_window.recent_files_for_tests(),
        vec![first_uri.clone()]
    );
    assert!(
        restore_window
            .selected_title_for_tests()
            .contains("two.txt")
    );

    let prune_settings = AppSettings::new_for_tests();
    prune_settings.set_recent_files(std::slice::from_ref(&missing_uri));
    let prune_window = build_window_with_settings(test_app, prune_settings);
    assert!(prune_window.is_some());
    let Some(prune_window) = prune_window else {
        return;
    };
    prune_window.ensure_default_tab();
    prune_window.request_open_recent(&missing_uri);
    drain_events(12);
    assert!(prune_window.recent_files_for_tests().is_empty());
    prune_window.request_open_files(vec![gio::File::for_path(&missing_path)], OpenSource::Drop);
    drain_events(12);

    let _removed = fs::remove_file(first_path);
    let _removed = fs::remove_file(second_path);
}

fn exercise_close_flows(test_app: &adw::Application) {
    let save_path = write_temp_file("riteed-close-save.txt", b"saved");
    let save_window = build_window(test_app);
    assert!(save_window.is_some());
    let Some(save_window) = save_window else {
        return;
    };
    save_window.request_open_files(vec![gio::File::for_path(&save_path)], OpenSource::AppOpen);
    spin_until("open saved file for close flow", || {
        save_window.tab_count_for_tests() == 1
            && !save_window.selected_saved_uri_for_tests().is_empty()
    });
    save_window.set_selected_text_for_tests("save-before-close");
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Cancel]);
    save_window.request_close_current_tab();
    drain_events(8);
    assert_eq!(save_window.tab_count_for_tests(), 1);
    assert_eq!(
        fs::read_to_string(&save_path).ok().as_deref(),
        Some("saved")
    );

    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Save]);
    save_window.request_close_current_tab();
    spin_until("save dirty tab on close", || {
        fs::read_to_string(&save_path).ok().as_deref() == Some("save-before-close")
    });
    drain_events(12);

    let discard_window = build_window(test_app);
    assert!(discard_window.is_some());
    let Some(discard_window) = discard_window else {
        return;
    };
    discard_window.ensure_default_tab();
    discard_window.set_selected_text_for_tests("discard-me");
    dialogs::queue_unsaved_responses_for_tests(&[UnsavedResponse::Discard]);
    discard_window.request_close_current_tab();
    drain_events(12);

    let first_path = write_temp_file("riteed-window-close-a.txt", b"one");
    let second_path = write_temp_file("riteed-window-close-b.txt", b"two");
    let first_uri = gio::File::for_path(&first_path).uri().to_string();
    let second_uri = gio::File::for_path(&second_path).uri().to_string();
    let window_close = build_window(test_app);
    assert!(window_close.is_some());
    let Some(window_close) = window_close else {
        return;
    };
    window_close.request_open_files(
        vec![
            gio::File::for_path(&first_path),
            gio::File::for_path(&second_path),
        ],
        OpenSource::AppOpen,
    );
    spin_until("open files for window close", || {
        window_close.tab_count_for_tests() == 2 && window_close.session_files_for_tests().len() == 2
    });
    window_close.request_open_recent(&first_uri);
    drain_events(8);
    window_close.set_selected_text_for_tests("one-dirty");
    window_close.request_open_recent(&second_uri);
    drain_events(8);
    window_close.set_selected_text_for_tests("two-dirty");
    dialogs::queue_unsaved_responses_for_tests(&[
        UnsavedResponse::Discard,
        UnsavedResponse::Discard,
    ]);
    assert_eq!(
        window_close.close_request_for_tests(),
        glib::Propagation::Stop
    );
    drain_events(12);

    let _removed = fs::remove_file(save_path);
    let _removed = fs::remove_file(first_path);
    let _removed = fs::remove_file(second_path);
}

fn exercise_app_open_actions() {
    let test_app = adw::Application::builder()
        .application_id("io.github.cadric.Riteed.Test")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let _registered = test_app.register(None::<&gio::Cancellable>);
    let state = std::rc::Rc::new(std::cell::RefCell::new(AppState {
        windows: Vec::new(),
        last_focused_window: None,
        session_restore_attempted: false,
    }));
    install_for_tests(&test_app, &state);

    let first_path = write_temp_file("riteed-open-a.txt", b"uno");
    let second_path = write_temp_file("riteed-open-b.txt", b"dos");
    let first_uri = gio::File::for_path(&first_path).uri().to_string();

    test_app.open(
        &[
            gio::File::for_path(&first_path),
            gio::File::for_path(&second_path),
        ],
        "",
    );
    let window = ensure_window_for_tests(&test_app, &state);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    spin_until("app open loads multiple files", || {
        window.tab_count_for_tests() == 2 && window.session_files_for_tests().len() == 2
    });

    test_app.activate_action("open-recent", Some(&first_uri.to_variant()));
    drain_events(8);
    assert_eq!(window.tab_count_for_tests(), 2);

    let _removed = fs::remove_file(first_path);
    let _removed = fs::remove_file(second_path);
}

fn exercise_app_actions_more() {
    let test_app = adw::Application::builder()
        .application_id("io.github.cadric.Riteed.Actions")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let _registered = test_app.register(None::<&gio::Cancellable>);
    let state = std::rc::Rc::new(std::cell::RefCell::new(AppState {
        windows: Vec::new(),
        last_focused_window: None,
        session_restore_attempted: false,
    }));
    install_for_tests(&test_app, &state);

    test_app.activate();
    let window = ensure_window_for_tests(&test_app, &state);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    spin_until("first activation creates a tab", || {
        window.tab_count_for_tests() == 1
    });

    test_app.activate_action("new", None);
    spin_until("new action adds a tab", || {
        window.tab_count_for_tests() == 2
    });
    test_app.activate_action("preferences", None);
    test_app.activate_action("about", None);
    test_app.activate_action("help", None);
    test_app.activate_action("open", None);
    drain_events(12);

    test_app.activate();
    drain_events(8);
    test_app.activate_action("quit", None);
    drain_events(12);
}

fn exercise_search_and_status(test_app: &adw::Application) {
    let search_window = build_window(test_app);
    assert!(search_window.is_some());
    let Some(search_window) = search_window else {
        return;
    };
    search_window.ensure_default_tab();
    assert!(!search_window.selected_line_numbers_visible_for_tests());
    assert_eq!(
        search_window.status_labels_for_tests(),
        (
            String::from("Untitled"),
            String::new(),
            String::from("Ln 1, Col 1")
        )
    );
    assert_eq!(
        search_window.status_format_summary_for_tests(),
        "UTF-8 · LF"
    );

    search_window.choose_selected_line_ending_from_preferences_for_tests(LineEndingMode::CrLf);
    assert_eq!(
        search_window.status_format_summary_for_tests(),
        "UTF-8 · CRLF"
    );
    assert_eq!(
        search_window.status_labels_for_tests(),
        (
            String::from("Untitled"),
            String::from("Modified"),
            String::from("Ln 1, Col 1")
        )
    );
    search_window.choose_selected_line_ending_from_preferences_for_tests(LineEndingMode::Lf);
    assert_eq!(
        search_window.status_format_summary_for_tests(),
        "UTF-8 · LF"
    );
    assert_eq!(
        search_window.status_labels_for_tests(),
        (
            String::from("Untitled"),
            String::new(),
            String::from("Ln 1, Col 1")
        )
    );

    search_window.set_selected_text_for_tests("alpha beta alpha");
    search_window.select_offsets_for_tests(0, 5);
    search_window.open_search(false);
    spin_until("search opens with prefill", || {
        search_window.search_visible_for_tests()
            && search_window.search_query_for_tests() == "alpha"
    });
    assert!(!search_window.replace_visible_for_tests());
    spin_until("search count becomes known", || {
        !search_window.search_result_for_tests().is_empty()
    });
    assert_eq!(search_window.search_result_for_tests(), "2 matches");

    search_window.open_search(true);
    drain_events(8);
    search_window.set_replace_text_for_tests("omega");
    search_window.replace_current_for_tests();
    drain_events(8);
    assert_eq!(search_window.selected_text_for_tests(), "omega beta alpha");

    search_window.set_replace_text_for_tests("z");
    search_window.replace_all_for_tests();
    spin_until("replace all result appears", || {
        search_window.selected_text_for_tests() == "omega beta z"
            && search_window.search_result_for_tests() == "Replaced 1 match"
    });
    search_window.undo_selected_for_tests();
    drain_events(8);
    assert_eq!(search_window.selected_text_for_tests(), "omega beta alpha");

    let multiline_window = build_window(test_app);
    assert!(multiline_window.is_some());
    let Some(multiline_window) = multiline_window else {
        return;
    };
    multiline_window.ensure_default_tab();
    multiline_window.set_selected_text_for_tests("line one\nline two");
    multiline_window.select_offsets_for_tests(0, 10);
    multiline_window.open_search(false);
    drain_events(8);
    assert!(multiline_window.search_query_for_tests().is_empty());

    let line_window = build_window(test_app);
    assert!(line_window.is_some());
    let Some(line_window) = line_window else {
        return;
    };
    line_window.ensure_default_tab();
    line_window.set_line_numbers_for_tests(true);
    assert!(line_window.selected_line_numbers_visible_for_tests());
    line_window.request_new();
    spin_until("new tab keeps line numbers enabled", || {
        line_window.tab_count_for_tests() == 2
    });
    assert!(line_window.selected_line_numbers_visible_for_tests());
}

#[test]
fn gtk_surfaces_and_editor_flow_work() {
    let _guard = crate::test_support::init_gtk_for_tests();
    if gtk::gdk::Display::default().is_none() {
        return;
    }
    assert_settings_apply();
    assert_app_actions_exist();
    crate::source_control::tree_model::exercise_state_restore_for_tests();

    let test_app = adw::Application::builder()
        .application_id("io.github.cadric.Riteed.WindowTabTests")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let _registered = test_app.register(None::<&gio::Cancellable>);

    exercise_window_tab_flow(&test_app);
    exercise_restore_and_recent_pruning(&test_app);
    exercise_close_flows(&test_app);
    exercise_app_open_actions();
    exercise_app_actions_more();
    exercise_search_and_status(&test_app);
    crate::gtk_tests_tabs::exercise_tab_context_actions();
    crate::gtk_tests_v4::exercise_v4_editor_features(&test_app);
    crate::gtk_tests_v5::exercise_v5_format_io(&test_app);
    crate::gtk_tests_v5b::exercise_v5b_editor_controls(&test_app);
    crate::gtk_tests_v6::exercise_v6_project_navigation(&test_app);
    crate::gtk_tests_v6::exercise_v6_project_restore(&test_app);
    crate::gtk_tests_v7::exercise_v7_compare(&test_app);
    crate::gtk_tests_v8::exercise_v8_polish_and_safety(&test_app);
    crate::gtk_tests_v9::exercise_v9_source_control(&test_app);
    crate::gtk_tests_v10::exercise_chrome_palette(&test_app);
}
