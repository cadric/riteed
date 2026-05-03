use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use gtk4::{gdk, gio, prelude::*};
use libadwaita as adw;

use crate::gtk_tests::{build_window, drain_events, spin_until, write_temp_file};
use crate::settings::SourceControlViewMode;
use crate::workspace::OpenSource;

pub(crate) fn exercise_v11_diff_surface(test_app: &adw::Application) {
    exercise_manual_compare_surface(test_app);
    exercise_navigation_copy_and_gutter_surface(test_app);
    exercise_asymmetric_gutter_width_surface(test_app);
    exercise_saved_reference_rebuild(test_app);
    exercise_git_compare_renderer_path(test_app);
}

fn exercise_manual_compare_surface(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let editable_text = "a0\nold\nleft only\nc0\nc1\nc2\nc3\nlast\n";
    let reference_text = "a0\nnew\nc0\nc1\nc2\nc3\nlast changed\nright tail\n";
    let expected_rows =
        crate::editor_tab::compare_row_count_for_texts_for_tests(editable_text, reference_text);
    let editable_path = write_temp_file("riteed-v11-editable.txt", editable_text.as_bytes());
    let reference_path = write_temp_file("riteed-v11-reference.txt", reference_text.as_bytes());

    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v11 editable file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v11-editable.txt")
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v11 compare renders row model", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_row_count_for_tests() == expected_rows
            && window.selected_compare_diff_count_for_tests() == 2
            && window.selected_compare_placeholder_count_for_tests() > 0
            && window.selected_compare_inline_range_count_for_tests() > 0
    });
    assert_eq!(
        window.selected_compare_line_counts_for_tests(),
        (usize_to_i32(expected_rows), usize_to_i32(expected_rows))
    );
    assert_eq!(
        window.selected_compare_views_editable_for_tests(),
        (false, false)
    );
    assert_eq!(
        window.selected_compare_line_numbers_for_tests(0),
        (Some(1), Some(1))
    );
    assert!(window.selected_compare_semantic_colors_for_tests());
    assert_eq!(
        window.selected_compare_wrap_modes_for_tests(),
        Some((gtk4::WrapMode::None, gtk4::WrapMode::None))
    );
    window.set_word_wrap_for_tests(true);
    drain_events(8);
    assert_eq!(
        window.selected_compare_wrap_modes_for_tests(),
        Some((gtk4::WrapMode::None, gtk4::WrapMode::None))
    );
    window.exit_compare_for_tests();
    drain_events(8);
    assert_eq!(
        window.selected_wrap_mode_for_tests(),
        Some(gtk4::WrapMode::WordChar)
    );

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn exercise_navigation_copy_and_gutter_surface(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let editable_text = numbered_compare_text("left eighty", "left one twenty");
    let reference_text = numbered_compare_text("right eighty", "right one twenty");
    let editable_path = write_temp_file("riteed-v11-gutter-editable.txt", editable_text.as_bytes());
    let reference_path =
        write_temp_file("riteed-v11-gutter-reference.txt", reference_text.as_bytes());

    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v11 gutter editable file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v11-gutter-editable.txt")
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v11 gutter compare starts", || {
        window.selected_compare_current_hunk_for_tests() == Some(0)
    });
    drain_events(16);
    assert_eq!(
        window.selected_compare_top_visible_row_for_tests(),
        79,
        "compare should open at first changed display row"
    );
    let gutter_widths = window.selected_compare_gutter_widths_for_tests();
    assert_eq!(
        gutter_widths.0, gutter_widths.1,
        "reference and current gutters should reserve identical width"
    );
    assert!(
        gutter_widths.0 >= 58,
        "gutter width should reserve three digits plus marker column, got {gutter_widths:?}"
    );

    window.scroll_selected_compare_to_row_for_tests(100);
    drain_events(8);
    window.next_diff_for_tests();
    spin_until("v11 next diff uses visible row", || {
        window.selected_compare_current_hunk_for_tests() == Some(1)
            && window.selected_compare_top_visible_row_for_tests() == 119
    });
    window.scroll_selected_compare_to_row_for_tests(100);
    drain_events(8);
    window.previous_diff_for_tests();
    spin_until("v11 previous diff uses visible row", || {
        window.selected_compare_current_hunk_for_tests() == Some(0)
            && window.selected_compare_top_visible_row_for_tests() == 79
    });

    let clipboard = gtk4::prelude::WidgetExt::display(window.widget()).clipboard();
    clipboard.set_text("sentinel");
    window.select_left_compare_range_for_tests(0, 8);
    assert!(window.copy_left_compare_selection_for_tests());
    assert_eq!(clipboard_text(&clipboard).as_deref(), Some("line 001"));
    clipboard.set_text("right-sentinel");
    window.select_right_compare_range_for_tests(0, 8);
    assert!(window.copy_right_compare_selection_for_tests());
    assert_eq!(clipboard_text(&clipboard).as_deref(), Some("line 001"));
    clipboard.set_text("unchanged");
    window.select_left_compare_range_for_tests(0, 0);
    assert!(!window.copy_left_compare_selection_for_tests());
    assert_eq!(clipboard_text(&clipboard).as_deref(), Some("unchanged"));

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn exercise_asymmetric_gutter_width_surface(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let editable_text = numbered_lines(120);
    let reference_text = "line 001\n";
    let editable_path = write_temp_file("riteed-v11-wide-current.txt", editable_text.as_bytes());
    let reference_path =
        write_temp_file("riteed-v11-narrow-reference.txt", reference_text.as_bytes());

    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v11 wide current file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v11-wide-current.txt")
    });
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v11 asymmetric gutter compare starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_placeholder_count_for_tests() > 0
    });
    drain_events(16);
    let gutter_widths = window.selected_compare_gutter_widths_for_tests();
    assert_eq!(
        gutter_widths.0, gutter_widths.1,
        "gutter widths should stay identical when only one side has three-digit current rows"
    );
    assert!(
        gutter_widths.0 >= 58,
        "gutter widths should include the marker column, got {gutter_widths:?}"
    );

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn exercise_saved_reference_rebuild(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let editable_path = write_temp_file("riteed-v11-save-sync.txt", b"before\n");

    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v11 save-sync file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v11-save-sync.txt")
    });
    window.set_selected_text_for_tests("after");
    window.compare_with_disk_for_tests();
    spin_until("v11 dirty disk compare starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() == 1
    });
    window.resolve_selected_external_for_tests();
    window.request_save();
    spin_until("v11 save writes dirty snapshot", || {
        fs::read_to_string(&editable_path).ok().as_deref() == Some("after\n")
    });
    spin_until("v11 save rebuilds raw reference", || {
        window.selected_compare_diff_count_for_tests() == 0
    });

    let _removed = fs::remove_file(editable_path);
}

fn exercise_git_compare_renderer_path(test_app: &adw::Application) {
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let marker_name = "000-riteed-v11-git-compare-test.txt";
    let marker = repo.join(marker_name);
    let _removed = fs::remove_file(&marker);
    assert!(fs::write(&marker, b"git renderer path\n").is_ok());

    let Some(window) = build_window(test_app) else {
        let _removed = fs::remove_file(marker);
        return;
    };
    window.handle_application_open(vec![gio::File::for_path(&repo)]);
    window.set_source_control_view_mode_for_tests(SourceControlViewMode::List);
    spin_until("v11 source control lists marker", || {
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

    let _removed = fs::remove_file(marker);
}

fn numbered_lines(count: usize) -> String {
    let mut text = String::new();
    for line in 1..=count {
        push_numbered_line(&mut text, line);
    }
    text
}

fn numbered_compare_text(line_80: &str, line_120: &str) -> String {
    let mut text = String::new();
    for line in 1..=130 {
        let content = match line {
            80 => line_80.to_string(),
            120 => line_120.to_string(),
            _ => format!("line {line:03}"),
        };
        text.push_str(&content);
        text.push('\n');
    }
    text
}

fn push_numbered_line(text: &mut String, line: usize) {
    text.push_str("line ");
    if line < 10 {
        text.push_str("00");
    } else if line < 100 {
        text.push('0');
    }
    text.push_str(&line.to_string());
    text.push('\n');
}

fn clipboard_text(clipboard: &gdk::Clipboard) -> Option<String> {
    let result = Rc::new(RefCell::new(None));
    let result_for_callback = Rc::clone(&result);
    clipboard.read_text_async(None::<&gio::Cancellable>, move |text| {
        let text = text.ok().flatten().map(|text| text.as_str().to_string());
        *result_for_callback.borrow_mut() = Some(text);
    });
    spin_until("v11 clipboard text arrives", || result.borrow().is_some());
    result.borrow_mut().take().flatten()
}

fn usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).map_or(i32::MAX, |count| count)
}
