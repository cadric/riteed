use std::fs;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::editor_tab::Writability;
use crate::gtk_tests::{build_window_with_settings, drain_events, spin_until, write_temp_file};
use crate::settings::{AppSettings, EditorPalette, ThemePreference};
use crate::workspace::OpenSource;

pub(crate) fn exercise_v8_polish_and_safety(test_app: &adw::Application) {
    exercise_presentation_preferences(test_app);
    exercise_recent_files_dialog(test_app);
    exercise_autosave_is_silent_and_gsettings_clean(test_app);
}

fn exercise_presentation_preferences(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    window.ensure_default_tab();
    let writes_before_sync = window.preferences_write_log_for_tests();
    window.sync_appearance_for_tests();
    assert_eq!(window.preferences_write_log_for_tests(), writes_before_sync);
    window.present_appearance_for_tests();
    assert_eq!(window.preferences_write_log_for_tests(), writes_before_sync);

    window.set_app_appearance_for_tests(ThemePreference::Light);
    window.select_editor_palette_for_tests(2);
    window.set_current_line_highlight_for_tests(false);
    window.set_autosave_for_tests(true);
    window.set_fullscreen_for_tests(true);
    window.set_fullscreen_for_tests(false);
    window.persist_window_size_for_tests();

    let writes = window.preferences_write_log_for_tests();
    assert!(writes.contains(&String::from("theme")));
    assert!(writes.contains(&String::from("editor-palette")));
    assert!(writes.contains(&String::from("highlight-current-line")));
    assert!(writes.contains(&String::from("autosave-enabled")));
    assert!(writes.contains(&String::from("window-width")));
    assert!(writes.contains(&String::from("window-height")));
    assert_eq!(
        window.selected_appearance_palette_for_tests(),
        EditorPalette::AdwaitaDark
    );
}

fn exercise_autosave_is_silent_and_gsettings_clean(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_autosave_enabled(true);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    let path = write_temp_file("riteed-v8-autosave.txt", b"before");
    let uri = gio::File::for_path(&path).uri().to_string();
    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    spin_until("v8 autosave file opened", || {
        window.selected_saved_uri_for_tests() == uri
    });
    spin_until("v8 autosave writability resolved", || {
        window.selected_writability_for_tests() == Some(Writability::Writable)
    });

    let writes_before = window.preferences_write_log_for_tests();
    window.resolve_selected_external_for_tests();
    window.set_selected_text_for_tests("after autosave");
    window.resolve_selected_external_for_tests();
    window.request_selected_autosave_for_tests();
    spin_until("v8 autosave writes without manual save", || {
        fs::read_to_string(&path).ok().as_deref() == Some("after autosave")
    });
    assert_eq!(window.preferences_write_log_for_tests(), writes_before);

    let _removed = fs::remove_file(path);
}

fn exercise_recent_files_dialog(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    let first = write_temp_file("riteed-v8-recent-first.txt", b"first");
    let second = write_temp_file("riteed-v8-recent-second.txt", b"second");
    let first_uri = gio::File::for_path(&first).uri().to_string();
    let second_uri = gio::File::for_path(&second).uri().to_string();
    settings.set_recent_files(&[first_uri, second_uri]);

    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    window.ensure_default_tab();
    window.present();
    assert!(
        gtk4::prelude::WidgetExt::activate_action(window.widget(), "win.recent-files", None)
            .is_ok()
    );
    drain_events(16);

    let _removed = fs::remove_file(first);
    let _removed = fs::remove_file(second);
}
