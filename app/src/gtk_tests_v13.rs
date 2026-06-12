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
