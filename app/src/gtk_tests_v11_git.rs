use gtk4::gio;
use libadwaita as adw;

use crate::git_process::test_support::{
    FixtureRepoFile, FixtureRepoKind, init_modified_fixture_repo_for_tests,
};
use crate::gtk_tests::{build_window, spin_until};
use crate::settings::SourceControlViewMode;

pub(crate) fn exercise_v11_git_compare_renderer_path(test_app: &adw::Application) {
    let marker_file = FixtureRepoFile::MARKER;
    let marker_name = marker_file.name();
    let Ok(repo) = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V11_GIT_COMPARE,
        marker_file,
        b"old\n",
        b"old\nnew current\n",
    ) else {
        return;
    };

    let Some(window) = build_window(test_app) else {
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
    window.set_source_control_view_mode_for_tests(SourceControlViewMode::List);
    spin_until("v11 source control lists temp marker", || {
        window
            .source_control_row_state_for_tests(marker_name)
            .is_some()
    });
    assert!(window.source_control_activate_path_for_tests(marker_name));
    spin_until("v11 git compare uses row renderer", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_row_count_for_tests() > 0
            && window.selected_compare_placeholder_count_for_tests() > 0
    });
}
