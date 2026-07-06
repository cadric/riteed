use std::fs;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::editor_zoom::effective_scroll_past_end_padding;
use crate::git_process::test_support::{
    FixtureRepoFile, FixtureRepoKind, init_modified_fixture_repo_for_tests,
};
use crate::gtk_tests::{
    TempFileFixture, build_window, build_window_with_settings, drain_events, spin_until,
    wait_millis, write_temp_file,
};
use crate::settings::{AppSettings, CompareViewMode, SourceControlViewMode};
use crate::workspace::OpenSource;

pub(crate) fn exercise_v7_compare(test_app: &adw::Application) {
    exercise_compare_minimap_wide_short_document_discriminant(test_app);
    exercise_compare_minimap_wide_long_document_attaches(test_app);
    exercise_compare_with_disk_and_file(test_app);
    exercise_compare_minimap_stays_visible_with_sidebar(test_app);
    exercise_compare_navigation(test_app);
    exercise_compare_two_files(test_app);
    exercise_compare_tab_actions(test_app);
    exercise_compare_exits_on_open(test_app);
}

fn positive_fitting_range((upper, page_size): (f64, f64)) -> bool {
    upper > 0.0 && page_size > 0.0 && (upper - page_size).abs() <= 0.5
}

fn scrollable_range((upper, page_size): (f64, f64)) -> bool {
    upper > page_size + 0.5
}

fn long_minimap_text(prefix: &str) -> String {
    let mut text = String::new();
    for line in 0..260 {
        text.push_str(prefix);
        text.push_str(" line ");
        text.push_str(&line.to_string());
        text.push('\n');
    }
    text
}

fn exercise_compare_minimap_wide_short_document_discriminant(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_compare_view_mode(CompareViewMode::Unified);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    window.widget().set_default_size(1400, 1200);
    window.present();
    drain_events(20);
    window.ensure_default_tab();
    window.set_minimap_for_tests(true);

    let editable_path = write_temp_file(TempFileFixture::V7_MINIMAP_SHORT, b"current\n");
    let reference_path = write_temp_file(TempFileFixture::V7_MINIMAP_SHORT_REF, b"reference\n");
    let editable_uri = gio::File::for_path(&editable_path).uri().to_string();
    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 short minimap file opens", || {
        window.selected_saved_uri_for_tests() == editable_uri
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v7 wide short compare starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() == 1
    });
    spin_until("v7 wide short compare holder maps", || {
        window.selected_compare_minimap_holders_mapped_for_tests().2
    });
    spin_until("v7 wide short compare has positive fitting range", || {
        let (_, _, unified_range) = window.selected_compare_minimap_viewport_ranges_for_tests();
        positive_fitting_range(unified_range)
    });
    let (_, _, unified_range) = window.selected_compare_minimap_viewport_ranges_for_tests();
    assert!(positive_fitting_range(unified_range));
    assert!(window.selected_compare_minimaps_attached_for_tests().2);
    wait_millis("v7 wide short minimap tick probe", 120);

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn exercise_compare_minimap_wide_long_document_attaches(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_compare_view_mode(CompareViewMode::Unified);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    window.widget().set_default_size(1400, 900);
    window.present();
    drain_events(20);
    window.ensure_default_tab();
    window.set_minimap_for_tests(true);

    let editable_text = long_minimap_text("current");
    let reference_text = long_minimap_text("reference");
    let editable_path = write_temp_file(TempFileFixture::V7_MINIMAP_LONG, editable_text.as_bytes());
    let reference_path = write_temp_file(
        TempFileFixture::V7_MINIMAP_LONG_REF,
        reference_text.as_bytes(),
    );
    let editable_uri = gio::File::for_path(&editable_path).uri().to_string();
    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 long minimap file opens", || {
        window.selected_saved_uri_for_tests() == editable_uri
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v7 wide long compare starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() > 0
    });
    spin_until("v7 wide long compare holder maps", || {
        window.selected_compare_minimap_holders_mapped_for_tests().2
    });
    spin_until("v7 wide long compare has scroll range", || {
        let (_, _, unified_range) = window.selected_compare_minimap_viewport_ranges_for_tests();
        scrollable_range(unified_range)
    });
    assert!(window.selected_compare_minimaps_attached_for_tests().2);
    wait_millis("v7 wide long minimap tick probe", 120);
    window.set_minimap_for_tests(false);
    spin_until("v7 wide long minimap detaches when hidden", || {
        !window.selected_compare_minimaps_visible_for_tests().2
            && !window.selected_compare_minimaps_attached_for_tests().2
    });
    window.set_minimap_for_tests(true);
    spin_until("v7 wide long minimap reattaches when shown", || {
        window.selected_compare_minimaps_visible_for_tests().2
            && window.selected_compare_minimap_holders_mapped_for_tests().2
            && window.selected_compare_minimaps_attached_for_tests().2
    });

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn collect_buttons(root: &gtk4::Widget, label: &str) -> Vec<gtk4::Button> {
    let mut stack = vec![root.clone()];
    let mut matches = Vec::new();
    while let Some(widget) = stack.pop() {
        if let Ok(button) = widget.clone().downcast::<gtk4::Button>()
            && button.label().as_deref() == Some(label)
        {
            matches.push(button);
        }

        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
    matches
}

fn click_first_sensitive_button(root: &gtk4::Widget, label: &str) {
    if let Some(button) = collect_buttons(root, label)
        .into_iter()
        .find(gtk4::prelude::WidgetExt::is_sensitive)
    {
        button.emit_clicked();
    }
}

fn wait_for_sensitive_button(root: &gtk4::Widget, label: &str, reason: &str) {
    spin_until(reason, || {
        collect_buttons(root, label)
            .first()
            .is_some_and(gtk4::prelude::WidgetExt::is_sensitive)
    });
}

fn exercise_compare_viewport_scroll_past_end(window: &crate::window::Window, expected_floor: i32) {
    assert!(expected_floor > 12);
    window.set_selected_compare_viewport_page_sizes_for_tests(0.0);
    assert_eq!(
        window.selected_compare_scroll_past_end_padding_for_tests(),
        (expected_floor, expected_floor, expected_floor)
    );
    window.set_selected_compare_viewport_page_sizes_for_tests(800.0);
    let expected_padding = effective_scroll_past_end_padding(expected_floor, 800.0);
    assert_eq!(
        window.selected_compare_scroll_past_end_padding_for_tests(),
        (expected_padding, expected_padding, expected_padding)
    );
    window.set_selected_compare_viewport_page_sizes_for_tests(0.0);
    assert_eq!(
        window.selected_compare_scroll_past_end_padding_for_tests(),
        (expected_floor, expected_floor, expected_floor)
    );
}

fn zoom_source_scroll_floor_for_compare(window: &crate::window::Window) -> i32 {
    let scroll_floor_before = window
        .selected_scroll_past_end_floor_for_tests()
        .map_or(0, |floor| floor);
    window.activate_status_zoom_in_for_tests();
    window.activate_status_zoom_in_for_tests();
    spin_until("v7 zoom raises compare padding floor", || {
        window.zoom_percent_for_tests() == 120
            && window
                .selected_scroll_past_end_floor_for_tests()
                .is_some_and(|floor| floor > scroll_floor_before)
    });
    window
        .selected_scroll_past_end_floor_for_tests()
        .map_or(0, |floor| floor)
}

fn exercise_compare_minimap_stays_visible_with_sidebar(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_compare_view_mode(CompareViewMode::Unified);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    window.widget().set_default_size(700, 1600);
    window.present();
    drain_events(20);
    window.ensure_default_tab();
    window.set_minimap_for_tests(true);

    let tracked_file = FixtureRepoFile::SIDEBAR_MINIMAP;
    let tracked_name = tracked_file.name();
    let long_line = "pub fn compare_minimap_sidebar_width_regression() { let value = Some(\"this line is deliberately long enough to force horizontal scrolling in the unified diff surface while the project sidebar is open\"); println!(\"{value:?}\"); }\n";
    let Ok(repo) = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V7_SIDEBAR_MINIMAP,
        tracked_file,
        b"old\n",
        long_line.as_bytes(),
    ) else {
        return;
    };
    let editable_path = repo.file_path(tracked_file);
    window.handle_application_open(vec![gio::File::for_path(repo.path())]);
    window.set_source_control_view_mode_for_tests(SourceControlViewMode::List);
    spin_until("v7 sidebar minimap source control row appears", || {
        window.project_sidebar_visible_for_tests()
            && window
                .source_control_row_state_for_tests(tracked_name)
                .is_some()
    });
    window.set_project_sidebar_position_for_tests(280);
    spin_until("v7 sidebar minimap keeps visible sidebar", || {
        window.project_sidebar_position_for_tests() >= 220
    });
    assert!(window.source_control_activate_path_for_tests(tracked_name));
    let editable_uri = gio::File::for_path(&editable_path).uri().to_string();
    spin_until("v7 sidebar source control compare starts", || {
        window.selected_saved_uri_for_tests() == editable_uri
            && window.selected_compare_active_for_tests()
    });
    spin_until("v7 narrow compare suppresses minimap", || {
        !window.selected_compare_minimaps_visible_for_tests().2
    });
    assert!(!window.selected_compare_minimaps_attached_for_tests().2);
    assert_eq!(
        window
            .selected_compare_minimap_scrollbar_policies_for_tests()
            .2,
        gtk4::PolicyType::Automatic
    );
    window.set_selected_compare_minimap_width_suppressed_for_tests(false);
    spin_until("v7 width unapply restores minimap attachment", || {
        window.selected_compare_minimaps_visible_for_tests().2
            && window.selected_compare_minimap_holders_mapped_for_tests().2
            && window.selected_compare_minimaps_attached_for_tests().2
    });
}

fn exercise_compare_with_disk_and_file(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_minimap_for_tests(true);

    let editable_path = write_temp_file(TempFileFixture::V7_EDITABLE, b"a\nb\nc\n");
    let editable_uri = gio::File::for_path(&editable_path).uri().to_string();
    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 editable file opened", || {
        window.selected_saved_uri_for_tests() == editable_uri
    });
    assert_eq!(
        window.compare_action_states_for_tests(),
        (false, false, false, false)
    );
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (true, false, true)
    );
    let zoomed_floor = zoom_source_scroll_floor_for_compare(&window);

    window.compare_with_disk_for_tests();
    spin_until("v7 compare with disk starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_status_for_tests() == "No differences were found."
    });
    assert!(!window.selected_minimap_visible_for_tests());
    assert_eq!(
        window.selected_compare_minimaps_visible_for_tests(),
        (true, true, true)
    );
    assert_eq!(
        window.selected_compare_minimaps_attached_for_tests(),
        (false, false, false)
    );
    assert_eq!(
        window.selected_compare_minimap_scrollbar_policies_for_tests(),
        (
            gtk4::PolicyType::Automatic,
            gtk4::PolicyType::Automatic,
            gtk4::PolicyType::Automatic
        )
    );
    exercise_compare_viewport_scroll_past_end(&window, zoomed_floor);
    assert_eq!(
        window.compare_action_states_for_tests(),
        (true, true, true, true)
    );
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (false, false, false)
    );
    assert_eq!(
        window.selected_compare_views_editable_for_tests(),
        (false, false)
    );

    window.exit_compare_for_tests();
    drain_events(8);
    window.set_selected_text_for_tests("a\nchanged\nc");
    window.compare_with_disk_for_tests();
    spin_until("v7 dirty disk compare starts read-only", || {
        window.selected_compare_diff_count_for_tests() == 1
    });
    window.request_save();
    spin_until("v7 save refreshes disk reference", || {
        fs::read_to_string(&editable_path).ok().as_deref() == Some("a\nchanged\nc\n")
            && window.selected_compare_diff_count_for_tests() == 0
    });

    let _written = fs::write(&editable_path, b"a\nexternal\nc\n");
    drain_events(8);
    assert_eq!(window.selected_compare_diff_count_for_tests(), 0);
    window.refresh_compare_reference_for_tests();
    spin_until("v7 manual reference refresh sees disk change", || {
        window.selected_compare_diff_count_for_tests() == 1
    });

    let reference_path = write_temp_file(TempFileFixture::V7_REFERENCE, b"x\nchanged\nc\n");
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v7 compare with file starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() > 0
    });
    assert_eq!(
        window.selected_compare_minimaps_attached_for_tests(),
        (false, false, false)
    );
    assert_eq!(window.selected_compare_diff_count_for_tests(), 1);
    assert!(window.selected_compare_highlight_count_for_tests() > 0);
    window.exit_compare_for_tests();
    drain_events(8);
    assert!(!window.selected_compare_active_for_tests());
    assert_eq!(window.selected_compare_highlight_count_for_tests(), 0);
    assert!(window.selected_minimap_visible_for_tests());

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn exercise_compare_navigation(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let nav_path = write_temp_file(
        TempFileFixture::V7_NAV,
        b"0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n",
    );
    let nav_ref = write_temp_file(
        TempFileFixture::V7_NAV_REF,
        b"x\n1\n2\n3\n4\n5\n6\n7\n8\n9\ny\n11\n",
    );
    window.request_open_files(vec![gio::File::for_path(&nav_path)], OpenSource::AppOpen);
    spin_until("v7 nav file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v7-nav.txt")
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&nav_ref));
    for _ in 0..600 {
        while glib::MainContext::default().iteration(false) {}
        let diff_count = window.selected_compare_diff_count_for_tests();
        let current = window.selected_compare_current_hunk_for_tests();
        if diff_count == 2 && current == Some(0) {
            break;
        }
        let _source = glib::timeout_add_local_once(Duration::from_millis(10), || {});
        let _dispatched = glib::MainContext::default().iteration(true);
    }
    let diff_count = window.selected_compare_diff_count_for_tests();
    let current = window.selected_compare_current_hunk_for_tests();
    let status = window.selected_compare_status_for_tests();
    assert!(
        diff_count == 2 && current == Some(0),
        "v7 compare has two hunks (diff_count={diff_count}, current={current:?}, status={status})"
    );
    window.next_diff_for_tests();
    spin_until("v7 next diff moves current hunk", || {
        window.selected_compare_current_hunk_for_tests() == Some(1)
    });
    window.previous_diff_for_tests();
    spin_until("v7 previous diff moves current hunk", || {
        window.selected_compare_current_hunk_for_tests() == Some(0)
    });

    let _removed = fs::remove_file(nav_path);
    let _removed = fs::remove_file(nav_ref);
}

fn exercise_compare_two_files(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let left = write_temp_file(TempFileFixture::V7_TWO_LEFT, b"left\nsame\n");
    let right = write_temp_file(TempFileFixture::V7_TWO_RIGHT, b"right\nsame\n");
    window.compare_two_files_for_tests(&gio::File::for_path(&left), &gio::File::for_path(&right));
    spin_until("v7 compare two files opens current and reference", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v7-two-right.txt")
            && window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() == 1
    });

    let _removed = fs::remove_file(left);
    let _removed = fs::remove_file(right);
}

fn exercise_compare_exits_on_open(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let compare_path = write_temp_file(TempFileFixture::V7_EXIT_COMPARE, b"compare\n");
    let reference_path = write_temp_file(TempFileFixture::V7_EXIT_REFERENCE, b"reference\n");
    window.request_open_files(
        vec![gio::File::for_path(&compare_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 exit compare file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v7-exit-compare.txt")
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v7 compare before replacement", || {
        window.selected_compare_active_for_tests()
    });

    let replacement_path = write_temp_file(TempFileFixture::V7_REPLACEMENT, b"replacement");
    window.request_open_files(
        vec![gio::File::for_path(&replacement_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 opening a file exits compare", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v7-replacement.txt")
            && !window.selected_compare_active_for_tests()
    });

    let _removed = fs::remove_file(compare_path);
    let _removed = fs::remove_file(reference_path);
    let _removed = fs::remove_file(replacement_path);
}

fn exercise_compare_tab_actions(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let compare_path = write_temp_file(TempFileFixture::V7_TAB_ACTIONS, b"before\n");
    let compare_uri = gio::File::for_path(&compare_path).uri().to_string();
    window.request_open_files(
        vec![gio::File::for_path(&compare_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 compare tab action file opened", || {
        window.selected_saved_uri_for_tests() == compare_uri
    });
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (true, false, true)
    );
    window.set_selected_text_for_tests("before\nchanged");
    drain_events(16);
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (true, true, true)
    );
    window.set_autosave_for_tests(true);
    drain_events(16);
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (true, false, true)
    );
    window.set_autosave_for_tests(false);
    window.set_selected_text_for_tests("before\nchanged again");
    drain_events(16);
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (true, true, true)
    );

    let activated = gtk4::prelude::WidgetExt::activate_action(
        window.widget(),
        "win.tab-compare-with-saved-version",
        None,
    )
    .is_ok();
    assert!(activated);
    spin_until("v7 compare starts from saved tab action", || {
        window.selected_compare_active_for_tests()
    });
    window.exit_compare_for_tests();
    drain_events(16);
    assert_eq!(
        window.tab_compare_action_states_for_tests(),
        (true, true, true)
    );

    let root_widget = window.widget().clone().upcast::<gtk4::Widget>();
    let activated = gtk4::prelude::WidgetExt::activate_action(
        window.widget(),
        "win.tab-compare-with-pasted-text",
        None,
    )
    .is_ok();
    assert!(activated);
    drain_events(16);
    wait_for_sensitive_button(
        &root_widget,
        "Compare",
        "v7 compare pasted text action opens paste dialog",
    );
    click_first_sensitive_button(&root_widget, "Compare");
    spin_until("v7 compare starts from pasted text tab action", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() > 0
    });
    window.exit_compare_for_tests();

    let reference_path = write_temp_file(TempFileFixture::V7_TAB_ACTION_REFERENCE, b"reference\n");
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v7 compare with file helper still starts", || {
        window.selected_compare_active_for_tests()
    });

    let _removed = fs::remove_file(compare_path);
    let _removed = fs::remove_file(reference_path);
}
