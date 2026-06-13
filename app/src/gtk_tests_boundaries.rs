use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use std::{io::Write, time::Duration};

// GTK boundary smoke runs inside the single GTK integration test; that keeps
// gtk4-rs initialization on one Rust test thread while preserving per-flow CI
// visibility through progress markers.
use crate::document_limits::{MIB, OPEN_FILE_LIMIT_BYTES, SEARCH_CHAR_LIMIT};
use crate::gtk_tests::{
    TempFileFixture, build_window, build_window_with_settings, drain_events, spin_until,
    write_temp_file,
};
use crate::settings::{AppSettings, LargeFileLimitValues};
use crate::workspace::OpenSource;

const OPEN_SEED: &[u8] = include_bytes!("../../stress/corpus/seeds/open-boundary.txt");
const SEARCH_SEED: &str = include_str!("../../stress/corpus/seeds/search-boundary.txt");

pub(crate) fn exercise_boundary_smokes(test_app: &adw::Application) {
    exercise_open_file_at_cap(test_app);
    exercise_search_at_cap(test_app);
    exercise_chunked_apply_completes_with_full_content(test_app);
    exercise_chunked_apply_close_during_apply(test_app);
    exercise_session_restore_survives_close_during_apply(test_app);
    exercise_long_line_file_routes_to_viewer(test_app);
    exercise_medium_file_disables_minimap_after_load(test_app);
    exercise_large_file_threshold_preferences_reapply_open_tab(test_app);
    exercise_large_file_viewer_rendering(test_app);
    exercise_large_file_viewer_close_releases_tab(test_app);
    exercise_large_file_edit_anyway_flow(test_app);
    exercise_large_file_edit_failure_keeps_viewer(test_app);
    exercise_large_file_viewer_refresh_updates_size(test_app);
    exercise_large_file_restore_placeholder(test_app);
    exercise_large_file_restore_placeholder_remove_button(test_app);
    exercise_large_file_placeholder_close_releases_tab(test_app);
}

fn exercise_medium_file_disables_minimap_after_load(test_app: &adw::Application) {
    let settings = large_file_settings(false);
    settings.set_show_minimap(true);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    let medium_len = usize::try_from(MIB.saturating_add(1024)).unwrap_or(0);
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_MEDIUM_MINIMAP,
        &repeat_seed(b"medium-minimap\n", medium_len),
    );
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("medium file opens in editor", || {
        window.selected_large_file_surface_for_tests() == Some("editor")
            && window.selected_saved_uri_for_tests() == uri
            && window
                .selected_text_for_tests()
                .starts_with("medium-minimap")
    });
    assert!(!window.selected_minimap_visible_for_tests());
    assert_eq!(
        window.selected_source_control_minimap_tag_counts_for_tests(),
        (0, 0, 0)
    );

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_threshold_preferences_reapply_open_tab(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_show_minimap(true);
    settings.set_large_file_limit_values(LargeFileLimitValues {
        full_feature: 4,
        editor: 5,
        strong_warning: 6,
        viewer_only: 7,
    });
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    let medium_len = usize::try_from(2 * MIB).unwrap_or(0);
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_THRESHOLD_REAPPLY,
        &repeat_seed(b"fn main() {}\n", medium_len),
    );
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("threshold test starts fully featured", || {
        window.selected_large_file_surface_for_tests() == Some("editor")
            && window.selected_saved_uri_for_tests() == uri
            && window.selected_minimap_visible_for_tests()
            && window.selected_language_id_for_tests().as_deref() == Some("rust")
    });
    window.set_large_file_full_feature_limit_for_tests(1.0);
    spin_until("threshold change reapplies heavy gates", || {
        !window.selected_minimap_visible_for_tests()
            && window.selected_language_id_for_tests().is_none()
    });
    assert_eq!(
        window.selected_source_control_minimap_tag_counts_for_tests(),
        (0, 0, 0)
    );

    let _removed = std::fs::remove_file(path);
}

#[test]
fn routed_open_uses_preflight_size_and_explicit_unknown_size_error() {
    let source = include_str!("workspace_open.rs");

    assert!(source.contains("OpenPlanQueryResult::SizeUnavailable"));
    assert!(source.contains("file_size_unavailable_error(&file)"));
    assert!(source.contains("load_file_with_open_support"));
}

#[test]
fn gtk_tests_boundaries_seed_corpus_is_small() {
    assert!(OPEN_SEED.len().saturating_add(SEARCH_SEED.len()) < 100_000);
}

fn exercise_open_file_at_cap(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let cap = usize::try_from(OPEN_FILE_LIMIT_BYTES).unwrap_or(0);
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_OPEN_CAP,
        &repeat_seed(OPEN_SEED, cap),
    );
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open file exactly at byte cap", || {
        window
            .session_files_for_tests()
            .iter()
            .any(|session_uri| session_uri == &uri)
    });

    let _removed = std::fs::remove_file(path);
}

fn exercise_search_at_cap(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    let query = SEARCH_SEED.trim();
    let search_limit = usize::try_from(SEARCH_CHAR_LIMIT).unwrap_or(0);
    let filler_len = search_limit.saturating_sub(query.len());
    let text = format!("{query}{}", "x".repeat(filler_len));
    window.set_selected_text_for_tests(&text);
    window.select_offsets_for_tests(0, i32::try_from(query.len()).unwrap_or(0));
    window.open_search(false);

    drain_events(12);
    assert!(window.search_visible_for_tests());
    assert_ne!(
        window.search_result_for_tests(),
        "Search is disabled for very large files."
    );
}

fn exercise_chunked_apply_completes_with_full_content(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("keep this tab");
    window.set_minimap_for_tests(true);
    let mut contents = repeat_seed(b"chunked-apply line\n", 3 * 1024 * 1024);
    if contents.last().is_some_and(|byte| *byte == b'\n') {
        contents.push(b'x');
    }
    let expected_text = String::from_utf8_lossy(&contents).into_owned();
    let expected_chars = i32::try_from(contents.len()).unwrap_or(i32::MAX);
    let path = write_temp_file(TempFileFixture::BOUNDARY_CHUNKED_FULL, &contents);

    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    spin_until_next_event("chunked apply hides the minimap", || {
        window.tab_count_for_tests() == 2
            && window.selected_loading_for_tests()
            && window.selected_char_count_for_tests() > 0
            && !window.selected_minimap_visible_for_tests()
    });
    spin_until("chunked apply fills the buffer", || {
        window.tab_count_for_tests() == 2
            && !window.selected_loading_for_tests()
            && window.selected_char_count_for_tests() == expected_chars
    });
    assert!(!window.selected_dirty_for_tests());
    assert!(window.selected_minimap_visible_for_tests());
    let text = window.selected_text_for_tests();
    assert_eq!(text, expected_text);

    let _removed = std::fs::remove_file(path);
}

fn exercise_chunked_apply_close_during_apply(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("keep this tab");
    let contents = repeat_seed(b"chunked-close line\n", 16 * 1024 * 1024);
    let path = write_temp_file(TempFileFixture::BOUNDARY_CHUNKED_CLOSE, &contents);

    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    spin_until_next_event("chunked apply is in progress", || {
        window.tab_count_for_tests() == 2
            && window.selected_loading_for_tests()
            && window.selected_char_count_for_tests() > 0
    });
    assert!(window.close_selected_page_for_tests());
    spin_until("loading tab closes promptly", || {
        window.tab_count_for_tests() == 1
    });
    drain_events(8);
    assert_eq!(window.selected_text_for_tests(), "keep this tab");

    let _removed = std::fs::remove_file(path);
}

fn exercise_session_restore_survives_close_during_apply(test_app: &adw::Application) {
    let small = write_temp_file(TempFileFixture::BOUNDARY_RESTORE_SMALL, b"small restore\n");
    let big = write_temp_file(
        TempFileFixture::BOUNDARY_RESTORE_BIG,
        &repeat_seed(b"restore-close line\n", 8 * 1024 * 1024),
    );
    let extra = write_temp_file(TempFileFixture::BOUNDARY_RESTORE_EXTRA, b"extra restore\n");
    let small_uri = gio::File::for_path(&small).uri().to_string();
    let big_uri = gio::File::for_path(&big).uri().to_string();
    let extra_uri = gio::File::for_path(&extra).uri().to_string();

    let settings = AppSettings::new_for_tests();
    settings.set_session_files(&[small_uri.clone(), big_uri.clone()]);
    settings.set_session_selected_file(&big_uri);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    window.restore_session();
    spin_until_next_event("session restore reaches chunked apply", || {
        window.tab_count_for_tests() == 2
            && window.selected_loading_for_tests()
            && window.selected_char_count_for_tests() > 0
    });
    assert!(window.close_selected_page_for_tests());
    spin_until("restore continues after cancelled apply", || {
        window.tab_count_for_tests() == 1
    });
    drain_events(8);

    // Session persistence must be unfrozen: a later open rewrites the session
    // without the file that was closed while its apply was still running.
    window.request_open_files(vec![gio::File::for_path(&extra)], OpenSource::AppOpen);
    spin_until("session persists without the closed file", || {
        let session = window.session_files_for_tests();
        session.contains(&extra_uri) && !session.contains(&big_uri) && session.contains(&small_uri)
    });

    let _removed = std::fs::remove_file(small);
    let _removed = std::fs::remove_file(big);
    let _removed = std::fs::remove_file(extra);
}

fn exercise_long_line_file_routes_to_viewer(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    let mut contents = vec![b'y'; 128 * 1024];
    contents.push(b'\n');
    let path = write_temp_file(TempFileFixture::BOUNDARY_LONG_LINE, &contents);

    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    spin_until("long-line file opens in the viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
    });

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_viewer_rendering(test_app: &adw::Application) {
    let Some(window) = build_window_with_settings(test_app, large_file_settings(false)) else {
        return;
    };
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_VIEWER_OPEN,
        &repeat_seed(b"viewer-start\nviewer-line\n", large_viewer_test_len()),
    );
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open large file in viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
            && window
                .session_files_for_tests()
                .iter()
                .any(|session_uri| session_uri == &uri)
            && window
                .selected_large_file_viewer_text_for_tests()
                .contains("viewer-start")
            && window
                .selected_large_file_viewer_status_for_tests()
                .contains("Viewing bytes")
    });
    assert!(window.selected_saved_uri_for_tests().is_empty());

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_viewer_close_releases_tab(test_app: &adw::Application) {
    let Some(window) = build_window_with_settings(test_app, large_file_settings(false)) else {
        return;
    };
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_VIEWER_CLOSE,
        &repeat_seed(b"viewer-close\nviewer-line\n", large_viewer_test_len()),
    );
    let file = gio::File::for_path(&path);

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open close-test large file in viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
            && window
                .selected_large_file_viewer_text_for_tests()
                .contains("viewer-close")
    });
    let weak = window.selected_tab_weak_for_tests();
    assert!(weak.is_some());
    let Some(weak) = weak else {
        let _removed = std::fs::remove_file(path);
        return;
    };
    window.request_close_current_tab();
    spin_until("closed viewer tab is released", || weak.upgrade().is_none());
    assert!(weak.upgrade().is_none());

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_edit_anyway_flow(test_app: &adw::Application) {
    let Some(window) = build_window_with_settings(test_app, large_file_settings(true)) else {
        return;
    };
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_VIEWER_EDIT,
        &repeat_seed(b"edit-start\nedit-line\n", large_viewer_test_len()),
    );
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open editable large file in viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
            && window
                .selected_large_file_viewer_text_for_tests()
                .contains("edit-start")
    });
    assert!(window.activate_selected_large_file_edit_for_tests());
    spin_until("edit anyway loads editor path", || {
        window.selected_large_file_surface_for_tests() == Some("editor")
            && window.selected_saved_uri_for_tests() == uri
            && window.selected_text_for_tests().starts_with("edit-start")
    });

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_edit_failure_keeps_viewer(test_app: &adw::Application) {
    let Some(window) = build_window_with_settings(test_app, large_file_settings(true)) else {
        return;
    };
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_VIEWER_EDIT_FAIL,
        &repeat_seed(
            b"edit-fail-start\nedit-fail-line\n",
            large_viewer_test_len(),
        ),
    );
    let file = gio::File::for_path(&path);

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open edit-fail large file in viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
            && window
                .selected_large_file_viewer_text_for_tests()
                .contains("edit-fail-start")
    });
    let over_cap = usize::try_from(OPEN_FILE_LIMIT_BYTES.saturating_add(1)).unwrap_or(0);
    assert!(std::fs::write(&path, repeat_seed(b"grown-past-cap\n", over_cap)).is_ok());
    assert!(window.activate_selected_large_file_edit_for_tests());
    spin_until("failed edit opt-in keeps viewer surface", || {
        !window.selected_loading_for_tests()
            && window.selected_large_file_surface_for_tests() == Some("viewer")
    });
    assert!(
        window
            .selected_large_file_viewer_text_for_tests()
            .contains("edit-fail-start")
    );

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_viewer_refresh_updates_size(test_app: &adw::Application) {
    let Some(window) = build_window_with_settings(test_app, large_file_settings(false)) else {
        return;
    };
    let initial = repeat_seed(b"refresh-start\nrefresh-line\n", large_viewer_test_len());
    let initial_size = initial.len();
    let path = write_temp_file(TempFileFixture::BOUNDARY_LARGE_VIEWER_REFRESH, &initial);
    let file = gio::File::for_path(&path);

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open refresh-test large file in viewer", || {
        window.selected_large_file_surface_for_tests() == Some("viewer")
            && window
                .selected_large_file_viewer_status_for_tests()
                .contains(&initial_size.to_string())
    });
    let append = repeat_seed(b"appended-tail\n", 64 * 1024);
    let new_size = initial_size.saturating_add(append.len());
    let appended = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(&append));
    assert!(appended.is_ok());
    assert!(window.activate_selected_large_file_refresh_for_tests());
    spin_until("viewer refresh picks up appended bytes", || {
        window
            .selected_large_file_viewer_status_for_tests()
            .contains(&new_size.to_string())
    });

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_restore_placeholder(test_app: &adw::Application) {
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_VIEWER_RESTORE,
        &repeat_seed(b"restore-start\nrestore-line\n", large_viewer_test_len()),
    );
    let uri = gio::File::for_path(&path).uri().to_string();
    let settings = large_file_settings(false);
    settings.set_session_files(std::slice::from_ref(&uri));
    settings.set_session_selected_file(&uri);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        let _removed = std::fs::remove_file(path);
        return;
    };

    window.restore_session();
    spin_until("restore large file placeholder", || {
        window.tab_count_for_tests() == 1
            && window.selected_large_file_surface_for_tests() == Some("restore-placeholder")
            && window
                .session_files_for_tests()
                .iter()
                .any(|session_uri| session_uri == &uri)
    });
    assert!(window.selected_text_for_tests().is_empty());

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_restore_placeholder_remove_button(test_app: &adw::Application) {
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_PLACEHOLDER_REMOVE,
        &repeat_seed(b"placeholder-remove\n", large_viewer_test_len()),
    );
    let uri = gio::File::for_path(&path).uri().to_string();
    let settings = large_file_settings(false);
    settings.set_session_files(std::slice::from_ref(&uri));
    settings.set_session_selected_file(&uri);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        let _removed = std::fs::remove_file(path);
        return;
    };

    window.restore_session();
    spin_until("restore removable large file placeholder", || {
        window.selected_large_file_surface_for_tests() == Some("restore-placeholder")
    });
    assert!(window.activate_selected_large_file_placeholder_remove_for_tests());
    spin_until("placeholder remove updates persisted session", || {
        !window
            .session_files_for_tests()
            .iter()
            .any(|session_uri| session_uri == &uri)
    });

    let _removed = std::fs::remove_file(path);
}

fn exercise_large_file_placeholder_close_releases_tab(test_app: &adw::Application) {
    let path = write_temp_file(
        TempFileFixture::BOUNDARY_LARGE_PLACEHOLDER_CLOSE,
        &repeat_seed(
            b"placeholder-close\nplaceholder-line\n",
            large_viewer_test_len(),
        ),
    );
    let uri = gio::File::for_path(&path).uri().to_string();
    let settings = large_file_settings(false);
    settings.set_session_files(std::slice::from_ref(&uri));
    settings.set_session_selected_file(&uri);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        let _removed = std::fs::remove_file(path);
        return;
    };

    window.restore_session();
    spin_until("restore close-test large file placeholder", || {
        window.selected_large_file_surface_for_tests() == Some("restore-placeholder")
    });
    let weak = window.selected_tab_weak_for_tests();
    assert!(weak.is_some());
    let Some(weak) = weak else {
        let _removed = std::fs::remove_file(path);
        return;
    };
    window.request_close_current_tab();
    spin_until("closed placeholder tab is released", || {
        weak.upgrade().is_none()
    });
    assert!(weak.upgrade().is_none());

    let _removed = std::fs::remove_file(path);
}

fn large_file_settings(always_edit: bool) -> AppSettings {
    let settings = AppSettings::new_for_tests();
    settings.set_large_file_limit_values(LargeFileLimitValues {
        full_feature: 1,
        editor: 2,
        strong_warning: 3,
        viewer_only: 4,
    });
    settings.set_always_allow_large_file_edit(always_edit);
    settings
}

fn large_viewer_test_len() -> usize {
    usize::try_from(2 * MIB).unwrap_or(0)
}

fn repeat_seed(seed: &[u8], target_len: usize) -> Vec<u8> {
    if seed.is_empty() {
        return vec![b'x'; target_len];
    }
    let mut contents = Vec::with_capacity(target_len);
    while contents.len().saturating_add(seed.len()) <= target_len {
        contents.extend_from_slice(seed);
    }
    let remaining = target_len.saturating_sub(contents.len());
    contents.extend_from_slice(&seed[..remaining]);
    contents
}

fn spin_until_next_event(label: &str, done: impl Fn() -> bool) {
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        if done() {
            return;
        }
        let _source = glib::timeout_add_local_once(Duration::from_millis(10), || {});
        let _dispatched = glib::MainContext::default().iteration(true);
    }
    assert!(done(), "{label}");
}
