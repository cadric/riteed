use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::git_process::test_support::init_modified_fixture_repo_for_tests;
use crate::gtk_tests::{build_window, drain_events, spin_until};
use crate::settings::SourceControlViewMode;
use crate::sidebar_host::SOURCE_CONTROL_ICON;
use crate::workspace::OpenSource;

pub(crate) fn exercise_v9_source_control(test_app: &adw::Application) {
    exercise_non_git_folder(test_app);
    exercise_portal_like_root_detect_starts_without_preflight(test_app);
    exercise_tracked_source_control_compare_after_open(test_app);
    exercise_editor_source_control_minimap_bands(test_app);

    let repo = unique_temp_repo("source-control-untracked");
    let Some(_cleanup) = CleanupDir::create(repo.clone()) else {
        return;
    };
    if init_modified_fixture_repo_for_tests(&repo, "baseline.txt", b"baseline\n", b"baseline\n")
        .is_err()
    {
        return;
    }
    let marker_name = "untracked.txt";
    let marker = repo.join(marker_name);
    assert!(fs::write(&marker, b"source control test").is_ok());

    let Some(window) = build_window(test_app) else {
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
    assert!(!window.source_control_commit_controls_visible_for_tests());
    assert!(window.source_control_history_split_resizable_for_tests());
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
    assert!(!window.source_control_commit_controls_visible_for_tests());
    assert!(window.source_control_activate_path_for_tests(marker_name));
    let marker_uri = gio::File::for_path(&marker).uri().to_string();
    spin_until("v9 source control row activation opens compare", || {
        window.selected_saved_uri_for_tests() == marker_uri
            && window.selected_compare_active_for_tests()
    });
    drain_events(12);
}

fn exercise_portal_like_root_detect_starts_without_preflight(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let calls = Rc::new(Cell::new(0));
    let captured_path = Rc::new(RefCell::new(String::new()));
    let calls_for_detect = calls.clone();
    let path_for_detect = captured_path.clone();
    window.set_source_control_detect_repo_for_tests(Rc::new(
        move |path, _cancellable, _callback| {
            calls_for_detect.set(calls_for_detect.get() + 1);
            *path_for_detect.borrow_mut() = path.to_string_lossy().to_string();
        },
    ));
    let portal_path = "/run/user/1000/doc/abcdef/test";
    window.set_source_control_project_root_for_tests(gio::File::for_path(portal_path));
    assert_eq!(calls.get(), 1);
    assert_eq!(captured_path.borrow().as_str(), portal_path);
    assert!(
        window
            .source_control_status_for_tests()
            .starts_with("Refreshing Git status")
    );
}

fn exercise_tracked_source_control_compare_after_open(test_app: &adw::Application) {
    let repo = unique_temp_repo("source-control-tracked");
    let Some(_cleanup) = CleanupDir::create(repo.clone()) else {
        return;
    };
    let tracked_name = "tracked.txt";
    let tracked_path = repo.join(tracked_name);
    if init_modified_fixture_repo_for_tests(
        &repo,
        tracked_name,
        b"baseline\n",
        b"baseline\nchanged\n",
    )
    .is_err()
    {
        return;
    }

    let Some(window) = build_window(test_app) else {
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(&repo)]);
    window.set_source_control_view_mode_for_tests(SourceControlViewMode::List);
    spin_until("v9 source control lists tracked modified file", || {
        window
            .source_control_row_state_for_tests(tracked_name)
            .is_some()
    });
    assert!(!window.source_control_commit_controls_visible_for_tests());
    assert!(window.source_control_activate_path_for_tests(tracked_name));
    let tracked_uri = gio::File::for_path(&tracked_path).uri().to_string();
    spin_until("v9 tracked source control activation opens compare", || {
        window.selected_saved_uri_for_tests() == tracked_uri
            && window.selected_compare_active_for_tests()
    });
    drain_events(16);
}

fn exercise_editor_source_control_minimap_bands(test_app: &adw::Application) {
    let repo = unique_temp_repo("source-control-minimap");
    let Some(_cleanup) = CleanupDir::create(repo.clone()) else {
        return;
    };
    let tracked_name = "minimap.txt";
    let tracked_path = repo.join(tracked_name);
    let current_text = "same\nnew\nlast\nadded\n";
    if init_modified_fixture_repo_for_tests(
        &repo,
        tracked_name,
        b"same\nold\nlast\n",
        current_text.as_bytes(),
    )
    .is_err()
    {
        return;
    }

    let Some(window) = build_window(test_app) else {
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(&repo)]);
    window.request_open_files(
        vec![gio::File::for_path(&tracked_path)],
        OpenSource::AppOpen,
    );
    let tracked_uri = gio::File::for_path(&tracked_path).uri().to_string();
    spin_until("v14.5 source file opened for minimap bands", || {
        window.selected_saved_uri_for_tests() == tracked_uri
    });
    spin_until("v14.5 source control minimap bands apply", || {
        window
            .source_control_row_state_for_tests(tracked_name)
            .is_some()
            && window.selected_source_control_minimap_tag_counts_for_tests() == (1, 1, 0)
    });
    assert!(!window.selected_dirty_for_tests());
    assert!(window.selected_source_control_minimap_tags_compose_for_tests());
    let decorated_text = window.selected_text_for_tests();

    window.set_selected_text_for_tests("same\nlocal\nlast\nadded\n");
    spin_until("v14.5 dirty buffer dims minimap bands", || {
        window.selected_source_control_minimap_stale_for_tests()
    });
    window.set_selected_text_for_tests(&decorated_text);
    spin_until("v14.5 restored text clears stale minimap state", || {
        !window.selected_source_control_minimap_stale_for_tests()
    });
    window.request_save();
    spin_until("v14.5 restored minimap buffer saves cleanly", || {
        !window.selected_dirty_for_tests()
    });

    assert!(fs::write(&tracked_path, b"same\nold\nlast\n").is_ok());
    spin_until(
        "v14.5 clean source control state clears minimap bands",
        || {
            window
                .source_control_row_state_for_tests(tracked_name)
                .is_none()
                && window.selected_source_control_minimap_tag_counts_for_tests() == (0, 0, 0)
        },
    );
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

fn unique_temp_repo(label: &str) -> PathBuf {
    let base =
        std::env::var_os("CARGO_TARGET_TMPDIR").map_or_else(std::env::temp_dir, PathBuf::from);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    base.join(format!("riteed-v9-{label}-{}-{nanos}", std::process::id()))
}
