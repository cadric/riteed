use std::cell::{Cell, RefCell};
use std::fs;
use std::rc::Rc;

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::editor_tab::EditorTab;
use crate::git_process::test_support::{
    FixtureRepoFile, FixtureRepoKind, init_modified_fixture_repo_for_tests,
};
use crate::gtk_tests::{build_window, drain_events, spin_until, test_tmp_dir};
use crate::large_file::reader::install_read_test_hooks;
use crate::settings::SourceControlViewMode;
use crate::sidebar_host::SOURCE_CONTROL_ICON;
use crate::window::Window;
use crate::workspace::{OpenSource, Workspace};

struct WindowCleanup(Rc<Window>);

impl Drop for WindowCleanup {
    fn drop(&mut self) {
        self.0.widget().destroy();
    }
}

pub(crate) fn exercise_v9_source_control(test_app: &adw::Application) {
    exercise_non_git_folder(test_app);
    exercise_portal_like_root_detect_starts_without_preflight(test_app);
    exercise_source_control_history_expands_after_collapsed_open(test_app);
    exercise_tracked_source_control_compare_after_open(test_app);
    exercise_overlapping_opens_do_not_duplicate_tabs(test_app);
    exercise_editor_source_control_minimap_bands(test_app);
    exercise_review_close_reopen_during_worktree_read(test_app);

    let Ok(repo) = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_UNTRACKED,
        FixtureRepoFile::BASELINE,
        b"baseline\n",
        b"baseline\n",
    ) else {
        return;
    };
    let marker_name = FixtureRepoFile::UNTRACKED.name();
    let marker = repo.file_path(FixtureRepoFile::UNTRACKED);
    assert!(fs::write(&marker, b"source control test").is_ok());

    let Some(window) = build_window(test_app) else {
        return;
    };
    assert_eq!(
        window.source_control_icon_for_tests().as_deref(),
        Some(SOURCE_CONTROL_ICON)
    );
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
    let repo_uri = gio::File::for_path(repo.path()).uri().to_string();
    spin_until("v9 project root opens", || {
        window.project_root_uri_for_tests().as_deref() == Some(repo_uri.as_str())
    });
    spin_until("v9 source control lists changed files", || {
        window.source_control_row_count_for_tests() > 0
            && window.source_control_status_for_tests() == "Changed files"
    });
    assert!(!window.source_control_commit_controls_visible_for_tests());
    assert!(window.source_control_history_split_resizable_for_tests());
    assert!(window.source_control_history_expanded_for_tests());
    assert!(window.source_control_history_content_revealed_for_tests());
    window.set_source_control_history_split_position_for_tests(420);
    assert_eq!(
        window.source_control_history_split_position_for_tests(),
        420
    );
    assert!(window.toggle_source_control_history_for_tests());
    assert!(!window.source_control_history_expanded_for_tests());
    assert!(!window.source_control_history_content_revealed_for_tests());
    assert!(window.source_control_history_split_position_for_tests() >= 420);
    assert!(window.toggle_source_control_history_for_tests());
    assert!(window.source_control_history_expanded_for_tests());
    assert!(window.source_control_history_content_revealed_for_tests());
    assert_eq!(
        window.source_control_history_split_position_for_tests(),
        420
    );
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

fn exercise_review_close_reopen_during_worktree_read(test_app: &adw::Application) {
    let repo = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_UNTRACKED,
        FixtureRepoFile::BASELINE,
        b"baseline\n",
        b"baseline\n",
    );
    assert!(repo.is_ok(), "task12a Git fixture must initialize");
    let Ok(repo) = repo else {
        return;
    };
    let marker = repo.file_path(FixtureRepoFile::UNTRACKED);
    assert!(fs::write(&marker, b"abcdef").is_ok());
    let window = build_window(test_app);
    assert!(window.is_some(), "task12a GTK window must initialize");
    let Some(window) = window else {
        return;
    };
    let _cleanup = WindowCleanup(Rc::clone(&window));
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
    spin_until("task12a review fixture is ready", || {
        window.source_control_row_count_for_tests() > 0
    });

    let reopened = Rc::new(Cell::new(false));
    let reopened_for_read = Rc::clone(&reopened);
    let window_for_read = Rc::clone(&window);
    let closes = Rc::new(Cell::new(0_usize));
    let closes_for_observer = Rc::clone(&closes);
    let _hooks = install_read_test_hooks(
        Some(2),
        Some(Rc::new(move |count| {
            if count == 0 || reopened_for_read.replace(true) {
                return;
            }
            assert!(window_for_read.close_selected_page_for_tests());
            assert!(
                gtk4::prelude::WidgetExt::activate_action(
                    window_for_read.widget(),
                    "win.git-review-unstaged",
                    None,
                )
                .is_ok()
            );
        })),
        Some(Rc::new(move |closed| {
            assert!(closed);
            closes_for_observer.set(closes_for_observer.get().saturating_add(1));
        })),
    );

    assert!(
        gtk4::prelude::WidgetExt::activate_action(
            window.widget(),
            "win.git-review-unstaged",
            None,
        )
        .is_ok()
    );
    spin_until("task12a successor review completes", || {
        reopened.get()
            && closes.get() == 2
            && window.selected_review_file_count_for_tests() == 1
            && window.selected_text_for_tests().contains("abcdef")
    });
    assert_eq!(window.tab_count_for_tests(), 1);
    assert_eq!(closes.get(), 2);
    assert!(window.selected_text_for_tests().contains("abcdef"));
}

fn exercise_source_control_history_expands_after_collapsed_open(test_app: &adw::Application) {
    let Ok(repo) = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_UNTRACKED,
        FixtureRepoFile::BASELINE,
        b"baseline\n",
        b"baseline\n",
    ) else {
        return;
    };
    let marker = repo.file_path(FixtureRepoFile::UNTRACKED);
    assert!(fs::write(&marker, b"source control collapsed history test").is_ok());

    let Some(window) = build_window(test_app) else {
        return;
    };
    assert!(window.toggle_source_control_history_for_tests());
    assert!(!window.source_control_history_expanded_for_tests());

    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
    let repo_uri = gio::File::for_path(repo.path()).uri().to_string();
    spin_until("v9 collapsed-history project root opens", || {
        window.project_root_uri_for_tests().as_deref() == Some(repo_uri.as_str())
    });
    spin_until("v9 collapsed-history lists changed files", || {
        window.source_control_row_count_for_tests() > 0
            && window.source_control_status_for_tests() == "Changed files"
    });
    assert!(window.source_control_history_root_visible_for_tests());
    assert!(!window.source_control_history_content_revealed_for_tests());
    assert_eq!(window.source_control_recent_commit_count_for_tests(), 0);

    assert!(window.toggle_source_control_history_for_tests());
    assert!(window.source_control_history_content_revealed_for_tests());
    spin_until("v9 collapsed-history loads after expand", || {
        window.source_control_recent_commit_count_for_tests() > 0
    });
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
    let tracked_file = FixtureRepoFile::TRACKED;
    let tracked_name = tracked_file.name();
    let Ok(repo) = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_TRACKED,
        tracked_file,
        b"baseline\n",
        b"baseline\nchanged\n",
    ) else {
        return;
    };
    let tracked_path = repo.file_path(tracked_file);

    let Some(window) = build_window(test_app) else {
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
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
    assert_eq!(
        window.source_control_active_row_path_for_tests().as_deref(),
        Some(tracked_name)
    );
    assert!(window.compare_fonts_match_editor_for_tests());
    window.apply_editor_font_for_tests("Monospace 19");
    drain_events(16);
    assert!(window.compare_fonts_match_editor_for_tests());
}

fn exercise_overlapping_opens_do_not_duplicate_tabs(test_app: &adw::Application) {
    let file_kind = FixtureRepoFile::BASELINE;
    let file_name = file_kind.name();
    let Ok(repo) = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_UNTRACKED,
        file_kind,
        b"baseline\n",
        b"baseline changed\n",
    ) else {
        return;
    };
    let file_path = repo.file_path(file_kind);

    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("keep this tab");
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
    spin_until("v9 dedupe lists changed files", || {
        window.source_control_row_count_for_tests() > 0
    });

    let file = gio::File::for_path(&file_path);
    window.request_open_files(vec![file.clone()], OpenSource::AppOpen);
    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("v9 dedupe overlapping opens settle", || {
        window.selected_char_count_for_tests() > 0
    });
    assert_eq!(window.tab_count_for_tests(), 2);

    assert!(window.source_control_activate_path_for_tests(file_name));
    drain_events(12);
    assert_eq!(window.tab_count_for_tests(), 2);
}

fn exercise_editor_source_control_minimap_bands(test_app: &adw::Application) {
    let tracked_file = FixtureRepoFile::MINIMAP;
    let tracked_name = tracked_file.name();
    let current_text = "same\nnew\nlast\nadded\n";
    let repo = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_MINIMAP,
        tracked_file,
        b"same\nold\nlast\n",
        current_text.as_bytes(),
    )
    .unwrap_or_else(|error| unreachable!("real minimap repository fixture: {error:?}"));
    let tracked_path = repo.file_path(tracked_file);

    let window = build_window(test_app).unwrap_or_else(|| unreachable!("GTK minimap window"));
    let _cleanup = WindowCleanup(Rc::clone(&window));
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
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
    let workspace = window
        .workspace_weak_for_tests()
        .upgrade()
        .unwrap_or_else(|| unreachable!("minimap window owns its workspace"));
    let tab = workspace
        .selected_tab()
        .unwrap_or_else(|| unreachable!("minimap document tab is selected"));
    spin_until(
        "v14.5 source control history settles before spawn baseline",
        || {
            window.source_control_recent_commit_count_for_tests() > 0
                && !workspace.selected_state_refresh_queued_for_tests()
        },
    );
    exercise_minimap_cursor_spawn_dedupe(&window, &workspace, &tab);
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

fn exercise_minimap_cursor_spawn_dedupe(window: &Window, workspace: &Workspace, tab: &EditorTab) {
    let cursor_spawn_baseline = crate::git_process::test_hooks::spawn_count_for_tests();
    let buffer = tab.text_buffer();
    for offset in [buffer.char_count(), 0, buffer.char_count(), 0] {
        buffer.place_cursor(&buffer.iter_at_offset(offset));
        assert!(workspace.selected_state_refresh_queued_for_tests());
        spin_until("v14.5 cursor minimap refresh idle completes", || {
            !workspace.selected_state_refresh_queued_for_tests()
        });
    }
    assert_eq!(
        crate::git_process::test_hooks::spawn_count_for_tests(),
        cursor_spawn_baseline,
        "clean cursor moves must not spawn Git children"
    );

    let fast_clean_spawn_baseline = crate::git_process::test_hooks::spawn_count_for_tests();
    buffer.set_text("same\nold\nlast\nextra-a\nextra-b\n");
    buffer.set_modified(false);
    assert!(!tab.source_control_minimap_stale_for_tests());
    buffer.place_cursor(&buffer.start_iter());
    assert!(workspace.selected_state_refresh_queued_for_tests());
    spin_until("v14.5 fast clean text refreshes minimap bands", || {
        !workspace.selected_state_refresh_queued_for_tests()
            && window.selected_source_control_minimap_tag_counts_for_tests() == (2, 0, 0)
    });
    assert_eq!(
        crate::git_process::test_hooks::spawn_count_for_tests(),
        fast_clean_spawn_baseline + 1,
        "changed clean text must load one fresh reference blob"
    );
}

fn exercise_non_git_folder(test_app: &adw::Application) {
    let folder = test_tmp_dir().join("riteed-v9-non-git-folder");
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
