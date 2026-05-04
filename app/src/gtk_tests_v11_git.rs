use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk4::gio;
use libadwaita as adw;

use crate::git_process::test_support::init_modified_fixture_repo_for_tests;
use crate::gtk_tests::{build_window, spin_until};
use crate::settings::SourceControlViewMode;

const MARKER_NAME: &str = "marker.txt";

pub(crate) fn exercise_v11_git_compare_renderer_path(test_app: &adw::Application) {
    let repo = unique_temp_repo();
    let Some(_cleanup) = CleanupDir::create(repo.clone()) else {
        return;
    };
    if init_modified_fixture_repo_for_tests(&repo, MARKER_NAME, b"old\n", b"old\nnew current\n")
        .is_err()
    {
        return;
    }

    let Some(window) = build_window(test_app) else {
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(&repo)]);
    window.set_source_control_view_mode_for_tests(SourceControlViewMode::List);
    spin_until("v11 source control lists temp marker", || {
        window
            .source_control_row_state_for_tests(MARKER_NAME)
            .is_some()
    });
    assert!(window.source_control_activate_path_for_tests(MARKER_NAME));
    spin_until("v11 git compare uses row renderer", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_row_count_for_tests() > 0
            && window.selected_compare_placeholder_count_for_tests() > 0
    });
}

struct CleanupDir(PathBuf);

impl CleanupDir {
    fn create(path: PathBuf) -> Option<Self> {
        let _removed = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).ok()?;
        Some(Self(path))
    }
}

impl Drop for CleanupDir {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

fn unique_temp_repo() -> PathBuf {
    let base =
        std::env::var_os("CARGO_TARGET_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    base.join(format!(
        "riteed-v11-git-compare-{}-{nanos}",
        std::process::id()
    ))
}
