use std::fs;

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::gtk_tests::{build_window, drain_events, spin_until};
use crate::settings::SourceControlViewMode;
use crate::sidebar_host::SOURCE_CONTROL_ICON;

pub(crate) fn exercise_v9_source_control(test_app: &adw::Application) {
    exercise_non_git_folder(test_app);

    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let marker_name = "000-riteed-v9-source-control-test.txt";
    let marker = repo.join(marker_name);
    let _removed = fs::remove_file(&marker);
    assert!(fs::write(&marker, b"source control test").is_ok());

    let Some(window) = build_window(test_app) else {
        let _removed = fs::remove_file(&marker);
        return;
    };
    assert_eq!(
        window.source_control_icon_for_tests().as_deref(),
        Some(SOURCE_CONTROL_ICON)
    );
    window.handle_application_open(vec![gio::File::for_path(&repo)]);
    let repo_uri = gio::File::for_path(&repo).uri().to_string();
    spin_until("v9 project root opens", || {
        window.project_root_uri_for_tests().as_deref() == Some(repo_uri.as_str())
    });
    spin_until("v9 source control lists changed files", || {
        window.source_control_row_count_for_tests() > 0
            && window.source_control_status_for_tests() == "Changed files"
    });
    assert_eq!(
        window.source_control_row_state_for_tests(marker_name),
        Some((String::from("U"), true, false))
    );
    spin_until("v10 source control recent commits load", || {
        window.source_control_recent_commit_count_for_tests() > 0
    });
    window.set_source_control_view_mode_for_tests(SourceControlViewMode::List);
    spin_until("v10 source control list view lists changed files", || {
        window.source_control_row_count_for_tests() > 0
    });
    assert!(window.source_control_activate_path_for_tests(marker_name));
    let marker_uri = gio::File::for_path(&marker).uri().to_string();
    spin_until("v9 source control row activation opens compare", || {
        window.selected_saved_uri_for_tests() == marker_uri
            && window.selected_compare_active_for_tests()
    });
    drain_events(12);

    let _removed = fs::remove_file(marker);
}

fn exercise_non_git_folder(test_app: &adw::Application) {
    let folder = std::env::temp_dir().join("riteed-v9-non-git-folder");
    let _removed = fs::remove_dir_all(&folder);
    assert!(fs::create_dir_all(&folder).is_ok());
    let Some(window) = build_window(test_app) else {
        let _removed = fs::remove_dir_all(folder);
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(&folder)]);
    let folder_uri = gio::File::for_path(&folder).uri().to_string();
    spin_until("v9 non-git project opens", || {
        window.project_root_uri_for_tests().as_deref() == Some(folder_uri.as_str())
    });
    spin_until("v9 non-git source control state settles", || {
        window.source_control_status_for_tests() == "This folder is not a Git repository."
    });
    assert_eq!(window.source_control_row_count_for_tests(), 0);
    let _removed = fs::remove_dir_all(folder);
}
