use std::fs;
use std::sync::{Arc, Mutex};

use gtk4 as gtk;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use libadwaita as adw;

use crate::app::{RiteedApp, ensure_window_for_tests, install_for_tests};
use crate::dialogs;
use crate::error::AppError;
use crate::settings::{AppSettings, ThemePreference};
use crate::window::Window;

fn spin_until(label: &str, done: impl Fn() -> bool) {
    for _ in 0..96 {
        while glib::MainContext::default().iteration(false) {}
        if done() {
            return;
        }
    }
    assert!(done(), "{label}");
}

fn drain_events(rounds: usize) {
    for _ in 0..rounds {
        while glib::MainContext::default().iteration(false) {}
    }
}

fn build_window(app: &adw::Application) -> Option<std::rc::Rc<Window>> {
    match Window::new_for_tests(app) {
        Ok(window) => Some(window),
        Err(error) => {
            let _body = error.body();
            None
        }
    }
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
        adw::StyleManager::default().color_scheme(),
        adw::ColorScheme::PreferDark
    );
}

fn assert_app_actions_exist() {
    let default_app = RiteedApp::default();
    assert!(default_app.application().lookup_action("new").is_some());
    let riteed_app = RiteedApp::new();
    let app = riteed_app.application();
    assert!(app.lookup_action("new").is_some());
    assert!(app.lookup_action("open").is_some());
    assert!(app.lookup_action("preferences").is_some());
    assert!(app.lookup_action("help").is_some());
    assert!(app.lookup_action("about").is_some());
    assert!(app.lookup_action("quit").is_some());
}

fn exercise_primary_window(
    test_app: &adw::Application,
    state: &std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<Window>>>>,
) {
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    *state.borrow_mut() = Some(window.clone());

    assert!(ensure_window_for_tests(test_app, state).is_some());
    assert_eq!(window.size_for_tests(), (840, 620));
    assert!(crate::window::builder_object_for_tests().is_err());
    assert!(crate::window::primary_menu_model_for_tests().n_items() >= 7);
    let _dialog = crate::window::text_file_dialog_for_tests("Open a Text File", "Open");
    assert!(matches!(
        crate::window::local_path_for_tests(&gio::File::for_uri("https://example.com/test.txt")),
        Err(AppError::NonLocalFile)
    ));

    let path = std::env::temp_dir().join("riteed-gtk-test.txt");
    let _removed = fs::remove_file(&path);
    assert!(fs::write(&path, "alpha").is_ok());

    window.request_open_file(gio::File::for_path(&path));
    spin_until("open existing file", || {
        window.buffer_text_for_tests() == "alpha"
    });
    assert!(!window.is_dirty_for_tests());

    window.set_text_for_tests("beta");
    assert!(window.is_dirty_for_tests());
    window.request_save();
    spin_until("save current file", || {
        fs::read_to_string(&path).ok().as_deref() == Some("beta")
    });
    spin_until("clear dirty after save", || !window.is_dirty_for_tests());
    assert!(!window.is_dirty_for_tests());
    window.request_new();
    spin_until("new document resets buffer", || {
        window.buffer_text_for_tests().is_empty()
    });
    assert!(!window.is_dirty_for_tests());
    assert_eq!(window.close_request_for_tests(), glib::Propagation::Proceed);

    let invalid_utf8_path = std::env::temp_dir().join("riteed-invalid-utf8.txt");
    let _removed = fs::remove_file(&invalid_utf8_path);
    assert!(fs::write(&invalid_utf8_path, [0xff, 0xfe, 0xfd]).is_ok());
    window.request_open_file(gio::File::for_path(&invalid_utf8_path));
    drain_events(8);

    let missing_path = std::env::temp_dir().join("riteed-missing-file.txt");
    let _removed = fs::remove_file(&missing_path);
    window.request_open_file(gio::File::for_path(&missing_path));
    drain_events(8);
    window.request_open_file(gio::File::for_uri("https://example.com/test.txt"));
    drain_events(4);

    window.set_text_for_tests("gamma");
    window.save_to_path_for_tests(std::env::temp_dir());
    drain_events(8);

    window.present();
    window.show_preferences();
    window.show_about();
    test_app.activate();
    drain_events(4);
    test_app.activate_action("preferences", None);
    test_app.activate_action("about", None);
    test_app.activate_action("help", None);
    dialogs::present_error(window.widget(), &AppError::Internal(String::from("error")));
    dialogs::present_error(window.widget(), &AppError::Cancelled);
    let response = Arc::new(Mutex::new(None));
    let response_clone = Arc::clone(&response);
    dialogs::confirm_unsaved_changes(window.widget(), |_response| {});
    dialogs::confirm_unsaved_changes(window.widget(), move |choice| {
        let lock = response_clone.lock();
        match lock {
            Ok(mut guard) => *guard = Some(choice),
            Err(poisoned) => *poisoned.into_inner() = Some(choice),
        }
    });
    dialogs::launch_help(window.widget(), |_error| {});

    let _removed = fs::remove_file(path);
    let _removed = fs::remove_file(invalid_utf8_path);
}

fn exercise_dialog_window(
    test_app: &adw::Application,
    state: &std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<Window>>>>,
) {
    let dialog_window = build_window(test_app);
    assert!(dialog_window.is_some());
    let Some(dialog_window) = dialog_window else {
        return;
    };
    *state.borrow_mut() = Some(dialog_window.clone());

    let signal_path = std::env::temp_dir().join("riteed-open-signal.txt");
    let _removed = fs::remove_file(&signal_path);
    assert!(fs::write(&signal_path, "omega").is_ok());
    test_app.open(&[gio::File::for_path(&signal_path)], "");
    spin_until("app open signal loads file", || {
        dialog_window.buffer_text_for_tests() == "omega"
    });
    dialog_window.set_text_for_tests("sigma");
    gtk::prelude::ActionGroupExt::activate_action(dialog_window.widget(), "save", None);
    spin_until("window save action writes file", || {
        fs::read_to_string(&signal_path).ok().as_deref() == Some("sigma")
    });

    dialog_window.set_text_for_tests("dirty");
    assert_eq!(
        dialog_window.close_request_for_tests(),
        glib::Propagation::Stop
    );
    test_app.activate_action("preferences", None);
    test_app.activate_action("about", None);
    test_app.activate_action("new", None);
    test_app.activate_action("open", None);
    test_app.activate_action("help", None);
    dialog_window.request_save_as();
    dialog_window.request_open_dialog();
    gtk::prelude::ActionGroupExt::activate_action(dialog_window.widget(), "close", None);
    drain_events(8);
    let _removed = fs::remove_file(signal_path);
}

#[test]
fn gtk_surfaces_and_editor_flow_work() {
    crate::bootstrap_runtime();
    let _adw = adw::init();
    assert_settings_apply();
    assert_app_actions_exist();

    let test_app = adw::Application::builder()
        .application_id("io.github.cadric.Riteed.Test")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let _registered = test_app.register(None::<&gio::Cancellable>);
    let state = std::rc::Rc::new(std::cell::RefCell::new(None));
    install_for_tests(&test_app, &state);
    exercise_primary_window(&test_app, &state);
    exercise_dialog_window(&test_app, &state);
}
