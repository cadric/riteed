use std::fs;

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::gtk_tests::{TempFileFixture, build_window, drain_events, spin_until, write_temp_file};
use crate::workspace::OpenSource;

pub(crate) fn exercise_v13_review_change_list(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let workspace = window.workspace();
    let spec = crate::editor_tab::ReviewTabSpec::new(
        crate::editor_tab::ReviewKind::Staged,
        std::path::PathBuf::from("/repo"),
        9,
        crate::editor_tab::ReviewSnapshotFingerprint::new("fingerprint"),
        vec![crate::editor_tab::ReviewFileSpec::new(
            b"src/main.rs".to_vec(),
        )],
        workspace.settings.compare_review_settings_snapshot(),
    );
    let tab = crate::editor_tab::EditorTab::new_git_review(&workspace.settings, spec.clone());
    tab.populate_review_session_with_spec(
        &spec,
        vec![crate::editor_tab::ReviewFileInput::file(
            crate::editor_tab::ReviewFileId::new(
                crate::editor_tab::ReviewKind::Staged,
                b"src/main.rs".to_vec(),
            ),
            crate::git_status::GitFileStatus::Modified,
            Some(String::from("old\nsame\n")),
            Some(String::from("new\nsame\n")),
        )],
    );
    workspace.add_tab(tab.clone(), true);
    tab.present_change_list();
    drain_events(4);
}

pub(crate) fn exercise_v13_status_bar_label_reuse(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let workspace = window.workspace();
    let _tab = workspace.add_empty_tab(true);
    drain_events(4);
    workspace.refresh_selected_state();
    let updates_before = workspace.status_bar.format_button_label_updates_for_tests();
    workspace.refresh_selected_state();
    let updates_after = workspace.status_bar.format_button_label_updates_for_tests();
    assert_eq!(updates_before, updates_after);
}

pub(crate) fn exercise_v13_status_refresh_coalescing(test_app: &adw::Application) {
    let path = write_temp_file(TempFileFixture::V13_STATUS_PRESENTATION, b"alpha\nbeta");
    let uri = gio::File::for_path(&path).uri().to_string();
    let window =
        build_window(test_app).unwrap_or_else(|| unreachable!("v13 status refresh GTK window"));
    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    spin_until("v13 status presentation file opens", || {
        window.selected_saved_uri_for_tests() == uri
    });
    let workspace = window.workspace();
    let tab = workspace
        .selected_tab()
        .unwrap_or_else(|| unreachable!("v13 status presentation tab"));
    let buffer = tab.text_buffer();
    buffer.place_cursor(&buffer.start_iter());
    drain_events(4);
    assert_eq!(tab.dirty_indicator_visible_for_tests(), Some(false));
    let (_, clean_modified, start_position) = workspace.status_bar.labels_for_tests();
    assert!(clean_modified.is_empty());
    assert!(start_position.contains('1'));

    let line_two = buffer.iter_at_line(1).unwrap_or_else(|| buffer.end_iter());
    buffer.place_cursor(&line_two);
    let mut end = buffer.end_iter();
    buffer.insert(&mut end, "!");
    let (_, pending_modified, pending_position) = workspace.status_bar.labels_for_tests();
    assert!(pending_modified.is_empty());
    assert_eq!(pending_position, start_position);

    drain_events(4);
    let (_, dirty_modified, moved_position) = workspace.status_bar.labels_for_tests();
    assert!(!dirty_modified.is_empty());
    assert_ne!(moved_position, start_position);
    assert_eq!(tab.dirty_indicator_visible_for_tests(), Some(true));
    assert_eq!(tab.title(), "riteed-v13-status-presentation.txt");

    tab.reset_presentation_sync_count_for_tests();
    for text in ["1", "2", "3", "4", "5", "6"] {
        let mut end = buffer.end_iter();
        buffer.insert(&mut end, text);
    }
    assert_eq!(
        tab.presentation_sync_count_for_tests(),
        0,
        "steady-state edits must not rebuild tab presentation per keystroke"
    );
    assert_eq!(tab.dirty_indicator_visible_for_tests(), Some(true));

    window.request_save();
    spin_until("v13 status presentation saves cleanly", || {
        fs::read_to_string(&path).ok().as_deref() == Some("alpha\nbeta!123456")
            && !window.selected_dirty_for_tests()
    });
    assert_eq!(tab.dirty_indicator_visible_for_tests(), Some(false));
    assert_eq!(tab.title(), "riteed-v13-status-presentation.txt");
    assert!(tab.presentation_sync_count_for_tests() > 0);

    let _removed = fs::remove_file(path);
}

pub(crate) fn exercise_v13_minimap_palette_cache(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let workspace = window.workspace();
    let tab = workspace.add_empty_tab(true);
    drain_events(2);
    crate::editor_tab::minimap_palette::clear_probe_cache_for_tests();
    tab.refresh_source_control_minimap_colors();
    assert_eq!(
        crate::editor_tab::minimap_palette::probe_miss_count_for_tests(),
        1
    );
    assert_eq!(
        crate::editor_tab::minimap_palette::probe_cache_len_for_tests(),
        1
    );
    tab.refresh_source_control_minimap_colors();
    assert_eq!(
        crate::editor_tab::minimap_palette::probe_miss_count_for_tests(),
        1
    );
}

pub(crate) fn exercise_v13_preview_search_active_tag_move(_test_app: &adw::Application) {
    crate::editor_search::exercise_preview_search_navigation_for_tests();
}
