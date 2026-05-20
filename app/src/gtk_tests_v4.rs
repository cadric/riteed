use std::fs;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::dialogs::{self, ExternalReloadResponse, StaleSaveResponse};
use crate::editor_monitor::ExternalFileEvent;
use crate::gtk_tests::{
    atomic_replace_file, build_window, build_window_with_settings, drain_events, spin_until,
    write_temp_file,
};
use crate::settings::AppSettings;
use crate::workspace::OpenSource;

fn exercise_external_banner(test_app: &adw::Application) {
    let banner_path = write_temp_file("riteed-v4-banner.rs", b"fn main() {}\n");
    let banner_uri = gio::File::for_path(&banner_path).uri().to_string();
    let banner_window = build_window(test_app);
    assert!(banner_window.is_some());
    let Some(banner_window) = banner_window else {
        return;
    };
    banner_window.present();
    drain_events(8);
    banner_window.request_open_files(vec![gio::File::for_path(&banner_path)], OpenSource::AppOpen);
    spin_until("selected clean file opened", || {
        banner_window.selected_saved_uri_for_tests() == banner_uri
    });
    let _written = fs::write(&banner_path, b"fn main() { println!(\"changed\"); }\n");
    banner_window
        .inject_external_event_for_tests(&banner_uri, ExternalFileEvent::ContentPossiblyChanged);
    banner_window.sync_selected_banner_for_tests(true);
    spin_until("selected banner or reload appears", || {
        banner_window.selected_banner_visible_for_tests()
            || banner_window.selected_text_for_tests().contains("changed")
    });
    if banner_window.selected_text_for_tests() == "fn main() {}" {
        banner_window.trigger_selected_external_action_for_tests();
        spin_until("selected banner reload applies", || {
            banner_window.selected_text_for_tests().contains("changed")
        });
    } else {
        assert!(banner_window.selected_text_for_tests().contains("changed"));
    }
    let _removed = fs::remove_file(banner_path);
}

pub(crate) fn exercise_v4_editor_features(test_app: &adw::Application) {
    let startup_settings = AppSettings::new_for_tests();
    startup_settings.set_show_minimap(true);
    let rust_window = build_window_with_settings(test_app, startup_settings);
    assert!(rust_window.is_some());
    let Some(rust_window) = rust_window else {
        return;
    };
    rust_window.ensure_default_tab();
    assert!(rust_window.selected_minimap_visible_for_tests());
    assert_eq!(
        rust_window.selected_minimap_scrollbar_policy_for_tests(),
        Some(gtk4::PolicyType::External)
    );
    rust_window.set_minimap_for_tests(false);
    assert!(!rust_window.selected_minimap_visible_for_tests());
    assert_eq!(
        rust_window.selected_minimap_scrollbar_policy_for_tests(),
        Some(gtk4::PolicyType::Automatic)
    );
    rust_window.set_minimap_for_tests(true);
    assert!(rust_window.selected_minimap_visible_for_tests());
    assert_eq!(
        rust_window.selected_minimap_scrollbar_policy_for_tests(),
        Some(gtk4::PolicyType::External)
    );

    let rust_path = write_temp_file("riteed-v4-syntax.rs", b"fn main() {}\n");
    rust_window.request_open_files(vec![gio::File::for_path(&rust_path)], OpenSource::AppOpen);
    spin_until("rust syntax detected", || {
        rust_window.selected_language_id_for_tests().as_deref() == Some("rust")
    });

    exercise_external_banner(test_app);

    let first_path = write_temp_file("riteed-v4-auto-a.txt", b"one");
    let second_path = write_temp_file("riteed-v4-auto-b.txt", b"two");
    let first_uri = gio::File::for_path(&first_path).uri().to_string();
    let second_uri = gio::File::for_path(&second_path).uri().to_string();
    let auto_window = build_window(test_app);
    assert!(auto_window.is_some());
    let Some(auto_window) = auto_window else {
        return;
    };
    auto_window.request_open_files(
        vec![
            gio::File::for_path(&first_path),
            gio::File::for_path(&second_path),
        ],
        OpenSource::AppOpen,
    );
    spin_until("two files for auto reload", || {
        auto_window.tab_count_for_tests() == 2
    });
    auto_window.request_open_recent(&second_uri);
    drain_events(8);
    atomic_replace_file(&first_path, b"one updated");
    spin_until(
        "background tab monitor reloads after atomic replace",
        || auto_window.text_for_uri_for_tests(&first_uri).as_deref() == Some("one updated"),
    );
    auto_window.request_open_recent(&first_uri);
    spin_until("background tab was reloaded", || {
        auto_window.selected_text_for_tests() == "one updated"
    });

    let stale_path = write_temp_file("riteed-v4-stale.txt", b"disk version");
    let stale_uri = gio::File::for_path(&stale_path).uri().to_string();
    let stale_window = build_window(test_app);
    assert!(stale_window.is_some());
    let Some(stale_window) = stale_window else {
        return;
    };
    stale_window.request_open_files(vec![gio::File::for_path(&stale_path)], OpenSource::AppOpen);
    spin_until("stale file open", || {
        stale_window.selected_saved_uri_for_tests() == stale_uri
    });
    stale_window.set_selected_text_for_tests("local edits");
    dialogs::queue_external_reload_responses_for_tests(&[ExternalReloadResponse::KeepCurrent]);
    stale_window
        .inject_external_event_for_tests(&stale_uri, ExternalFileEvent::ContentPossiblyChanged);
    drain_events(12);
    dialogs::queue_stale_save_responses_for_tests(&[StaleSaveResponse::Cancel]);
    stale_window.request_save();
    drain_events(12);
    assert_eq!(
        fs::read_to_string(&stale_path).ok().as_deref(),
        Some("disk version")
    );

    let _removed = fs::remove_file(rust_path);
    let _removed = fs::remove_file(first_path);
    let _removed = fs::remove_file(second_path);
    let _removed = fs::remove_file(stale_path);
}
