use gtk4::prelude::*;
use libadwaita as adw;

use crate::gtk_tests::{build_window, drain_events};

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
    let Some(window) = build_window(test_app) else {
        return;
    };
    let workspace = window.workspace();
    let tab = workspace.add_empty_tab(true);
    let buffer = tab.text_buffer();
    buffer.set_text("alpha\nbeta");
    buffer.place_cursor(&buffer.start_iter());
    buffer.set_modified(false);
    drain_events(4);
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
