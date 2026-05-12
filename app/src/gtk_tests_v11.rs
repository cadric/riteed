use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use gtk4::{gdk, gio, prelude::*};
use libadwaita as adw;

use crate::editor_tab::{SaveResult, Writability};
use crate::gtk_tests::{build_window_with_settings, drain_events, spin_until, write_temp_file};
use crate::settings::AppSettings;
use crate::workspace::OpenSource;

const SCROLL_OFFSET_TOLERANCE: f64 = 2.0;

pub(crate) fn exercise_v11_diff_surface(test_app: &adw::Application) {
    exercise_manual_compare_surface(test_app);
    exercise_navigation_copy_and_gutter_surface(test_app);
    exercise_asymmetric_gutter_width_surface(test_app);
    exercise_saved_reference_rebuild(test_app);
    exercise_compare_pauses_guarded_autosave(test_app);
}

fn exercise_manual_compare_surface(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_compare_collapse_unchanged(false);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    let long_reference_tail = format!("right tail {}\n", "x".repeat(240));
    let editable_text = "a0\nold\nleft only\nc0\nc1\nc2\nc3\nlast\n";
    let reference_text = format!("a0\nnew\nc0\nc1\nc2\nc3\nlast changed\n{long_reference_tail}");
    let expected_rows =
        crate::editor_tab::compare_row_count_for_texts_for_tests(editable_text, &reference_text);
    let editable_path = write_temp_file("riteed-v11-editable.rs", editable_text.as_bytes());
    let reference_path = write_temp_file("riteed-v11-reference.rs", reference_text.as_bytes());

    window.request_open_files(
        vec![gio::File::for_path(&editable_path)],
        OpenSource::AppOpen,
    );
    spin_until("v11 editable file opened", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v11-editable.rs")
    });
    spin_until("v11 editable syntax detected", || {
        window.selected_language_id_for_tests().as_deref() == Some("rust")
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
    assert!(window.selected_compare_uses_full_row_backgrounds_for_tests());
    assert_eq!(
        window.selected_compare_syntax_highlight_for_tests(),
        (true, true)
    );
    assert_compare_hatches_target_placeholders(&window);
    let (_, right_markers) = window.selected_compare_placeholder_markers_for_tests();
    let right_marker_text = window.selected_right_compare_line_text_for_tests(right_markers[0].0);
    let right_marker_start = usize_to_i32("a0\nold\nleft only\nc0\nc1\nc2\nc3\nlast\n".len());
    let right_real_start = usize_to_i32("a0\nold\nleft only\nc0\nc1\nc2\nc3\n".len());
    let clipboard = gtk4::prelude::WidgetExt::display(window.widget()).clipboard();
    window.select_right_compare_range_for_tests(
        right_real_start,
        right_marker_start + usize_to_i32(right_marker_text.len()),
    );
    assert!(window.copy_right_compare_selection_for_tests());
    assert_eq!(clipboard_text(&clipboard).as_deref(), Some("last\n"));
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
    let before_hscroll = window.selected_compare_hatch_viewports_for_tests();
    window.set_left_compare_horizontal_scroll_value_for_tests(24.0);
    spin_until(
        "v11 hatch left horizontal scroll moves only left pane",
        || {
            let after = window.selected_compare_hatch_viewports_for_tests();
            after.0.0 > before_hscroll.0.0 && after.1.0 == before_hscroll.1.0
        },
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
    let settings = AppSettings::new_for_tests();
    settings.set_compare_collapse_unchanged(false);
    let Some(window) = build_window_with_settings(test_app, settings) else {
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
    spin_until("v11 compare opens at first changed display row", || {
        compare_positions_match(&window, 79, 0.0)
    });
    drain_layout_events(16);
    assert_compare_positions(&window, 79, 0.0);
    drain_layout_events(16);
    assert_compare_positions(&window, 79, 0.0);
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
    drain_layout_events(8);
    assert_compare_positions(&window, 100, 0.0);
    window.reset_compare_scroll_event_counts_for_tests();
    assert!(window.scroll_left_compare_to_row_offset_for_tests(100, 7.0));
    drain_layout_events(8);
    assert_compare_positions(&window, 100, 7.0);
    assert_eq!(
        window.compare_scroll_event_counts_for_tests().1,
        0,
        "left-origin scroll should block the right feedback handler"
    );
    window.reset_compare_scroll_event_counts_for_tests();
    assert!(window.scroll_right_compare_to_row_offset_for_tests(108, 5.0));
    drain_layout_events(8);
    assert_compare_positions(&window, 108, 5.0);
    assert_eq!(
        window.compare_scroll_event_counts_for_tests().0,
        0,
        "right-origin scroll should block the left feedback handler"
    );
    exercise_smooth_scroll_burst(&window);
    window.next_diff_for_tests();
    spin_until("v11 next diff uses visible row", || {
        window.selected_compare_current_hunk_for_tests() == Some(1)
            && compare_positions_match(&window, 119, 0.0)
    });
    window.scroll_selected_compare_to_row_for_tests(100);
    drain_layout_events(8);
    window.previous_diff_for_tests();
    spin_until("v11 previous diff uses visible row", || {
        window.selected_compare_current_hunk_for_tests() == Some(0)
            && compare_positions_match(&window, 79, 0.0)
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
    let settings = AppSettings::new_for_tests();
    settings.set_compare_collapse_unchanged(false);
    let Some(window) = build_window_with_settings(test_app, settings) else {
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
    let (left_markers, right_markers) = window.selected_compare_placeholder_markers_for_tests();
    assert_eq!(right_markers.len(), 0);
    assert_eq!(left_markers.len(), 1);
    let (marker_row, marker_len) = left_markers[0];
    assert!(marker_len >= 100);
    let marker_text = window.selected_left_compare_line_text_for_tests(marker_row);
    assert!(marker_text.contains("lines only in current"));
    assert_eq!(
        window.selected_left_compare_line_text_for_tests(marker_row + 1),
        ""
    );
    let clipboard = gtk4::prelude::WidgetExt::display(window.widget()).clipboard();
    clipboard.set_text("marker-sentinel");
    assert!(window.select_left_compare_line_offsets_for_tests(marker_row, 1, 5));
    assert!(window.copy_left_compare_selection_for_tests());
    assert_eq!(clipboard_text(&clipboard).as_deref(), Some(""));
    clipboard.set_text("newline-sentinel");
    let marker_start = usize_to_i32("line 001\n".len());
    let marker_end = marker_start + usize_to_i32(marker_text.len()) + 2;
    window.select_left_compare_range_for_tests(marker_start, marker_end);
    assert!(window.copy_left_compare_selection_for_tests());
    assert_eq!(clipboard_text(&clipboard).as_deref(), Some("\n\n"));
    window.select_left_compare_range_for_tests(0, marker_end);
    assert!(window.copy_left_compare_selection_for_tests());
    assert_eq!(
        clipboard_text(&clipboard).as_deref(),
        Some("line 001\n\n\n")
    );

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
}

fn exercise_saved_reference_rebuild(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_compare_collapse_unchanged(false);
    let Some(window) = build_window_with_settings(test_app, settings) else {
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

fn exercise_compare_pauses_guarded_autosave(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    settings.set_autosave_enabled(true);
    settings.set_compare_collapse_unchanged(false);
    let Some(window) = build_window_with_settings(test_app, settings) else {
        return;
    };
    let editable_path = write_temp_file("riteed-v11-autosave-compare.txt", b"before\n");
    let reference_path = write_temp_file("riteed-v11-autosave-reference.txt", b"reference\n");
    let editable_file = gio::File::for_path(&editable_path);
    let reference_file = gio::File::for_path(&reference_path);
    let editable_uri = editable_file.uri().to_string();

    window.request_open_files(vec![editable_file], OpenSource::AppOpen);
    spin_until("v11 autosave compare file opened", || {
        window.selected_saved_uri_for_tests() == editable_uri
    });
    spin_until("v11 autosave compare writability resolved", || {
        window.selected_writability_for_tests() == Some(Writability::Writable)
    });
    window.resolve_selected_external_for_tests();
    window.set_selected_text_for_tests("after compare autosave");
    window.resolve_selected_external_for_tests();
    assert_eq!(
        fs::read_to_string(&editable_path).ok().as_deref(),
        Some("before\n")
    );

    window.compare_with_file_for_tests(&reference_file);
    spin_until("v11 guarded autosave compare starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() > 0
    });
    let compare_autosave = window.request_selected_guarded_autosave_for_tests();
    drain_events(12);
    assert!(
        !compare_autosave.requested && compare_autosave.result.borrow().is_none(),
        "compare mode should block the guarded autosave entry point"
    );
    assert_eq!(
        fs::read_to_string(&editable_path).ok().as_deref(),
        Some("before\n")
    );
    assert!(window.selected_compare_active_for_tests());

    window.exit_compare_for_tests();
    drain_events(8);
    assert!(!window.selected_compare_active_for_tests());
    let exit_autosave = window.request_selected_guarded_autosave_for_tests();
    assert!(
        exit_autosave.requested,
        "compare exit should leave the selected tab autosave-eligible"
    );
    spin_until("v11 guarded autosave returns after compare exits", || {
        exit_autosave.result.borrow().is_some()
    });
    let save_result = exit_autosave.result.borrow().clone();
    assert!(
        matches!(save_result, Some(SaveResult::Saved(_))),
        "guarded autosave after compare returned {save_result:?}"
    );
    assert_eq!(
        fs::read_to_string(&editable_path).ok().as_deref(),
        Some("after compare autosave\n")
    );

    let _removed = fs::remove_file(editable_path);
    let _removed = fs::remove_file(reference_path);
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

fn assert_compare_hatches_target_placeholders(window: &crate::window::Window) {
    assert_eq!(
        window.selected_compare_hatch_overlay_states_for_tests(),
        ((false, false), (false, false))
    );
    let row_count = window.selected_compare_row_count_for_tests();
    let left_hatch_row = (0..row_count).find(|row| {
        let (reference, current) = window.selected_compare_line_numbers_for_tests(*row);
        reference.is_none() && current.is_some()
    });
    let right_hatch_row = (0..row_count).find(|row| {
        let (reference, current) = window.selected_compare_line_numbers_for_tests(*row);
        reference.is_some() && current.is_none()
    });
    assert!(
        left_hatch_row.is_some(),
        "compare fixture should include a current-only filler row"
    );
    assert!(
        right_hatch_row.is_some(),
        "compare fixture should include a reference-only filler row"
    );
    if let Some(row) = left_hatch_row {
        let (left_markers, _) = window.selected_compare_placeholder_markers_for_tests();
        assert!(
            left_markers
                .iter()
                .any(|(marker_row, _len)| *marker_row == row)
        );
        assert!(
            !window
                .selected_left_compare_line_text_for_tests(row)
                .is_empty()
        );
        window.scroll_selected_compare_to_row_for_tests(row);
        drain_layout_events(8);
        spin_until("v11 left hatch region becomes visible", || {
            let (left_regions, _) = window.selected_compare_hatch_regions_for_tests();
            left_regions
                .iter()
                .any(|(region_row, _x, _y, _width, height)| *region_row == row && *height > 0)
        });
    }
    if let Some(row) = right_hatch_row {
        let (_, right_markers) = window.selected_compare_placeholder_markers_for_tests();
        assert!(
            right_markers
                .iter()
                .any(|(marker_row, _len)| *marker_row == row)
        );
        assert!(
            !window
                .selected_right_compare_line_text_for_tests(row)
                .is_empty()
        );
        window.scroll_selected_compare_to_row_for_tests(row);
        drain_layout_events(8);
        spin_until("v11 right hatch region becomes visible", || {
            let (_, right_regions) = window.selected_compare_hatch_regions_for_tests();
            right_regions
                .iter()
                .any(|(region_row, _x, _y, _width, height)| *region_row == row && *height > 0)
        });
    }
}

fn exercise_smooth_scroll_burst(window: &crate::window::Window) {
    assert!(window.scroll_left_compare_to_row_offset_for_tests(100, 0.0));
    drain_layout_events(8);
    let start = window.left_compare_scroll_value_for_tests();
    window.reset_compare_scroll_event_counts_for_tests();
    for step in 0..64 {
        window.set_left_compare_scroll_value_for_tests(start + f64::from(step) * 0.7);
        drain_layout_events(1);
    }
    let (left_events, right_events) = window.compare_scroll_event_counts_for_tests();
    assert!(
        left_events > 0,
        "smooth-scroll burst should exercise the source handler"
    );
    assert_eq!(
        right_events, 0,
        "smooth-scroll burst should not bounce through the peer handler"
    );
    assert_compare_panes_aligned(window);
}

fn assert_compare_positions(window: &crate::window::Window, row: usize, offset: f64) {
    assert_eq!(
        window.selected_compare_top_visible_rows_for_tests(),
        (row, row)
    );
    assert!(
        compare_positions_match(window, row, offset),
        "expected compare panes at row {row} offset {offset}, got {:?}",
        window.selected_compare_top_visible_positions_for_tests()
    );
}

fn compare_positions_match(window: &crate::window::Window, row: usize, offset: f64) -> bool {
    let (left, right) = window.selected_compare_top_visible_positions_for_tests();
    position_matches(left, row, offset) && position_matches(right, row, offset)
}

fn position_matches(position: Option<(usize, f64)>, row: usize, offset: f64) -> bool {
    position.is_some_and(|(actual_row, actual_offset)| {
        actual_row == row && (actual_offset - offset).abs() <= SCROLL_OFFSET_TOLERANCE
    })
}

fn assert_compare_panes_aligned(window: &crate::window::Window) {
    let (left, right) = window.selected_compare_top_visible_positions_for_tests();
    if let (Some((left_row, left_offset)), Some((right_row, right_offset))) = (left, right) {
        assert_eq!(left_row, right_row);
        assert!(
            (left_offset - right_offset).abs() <= SCROLL_OFFSET_TOLERANCE,
            "compare offsets drifted: left {left_offset}, right {right_offset}"
        );
    } else {
        assert!(
            left.is_some() && right.is_some(),
            "expected compare viewport positions, got {left:?} and {right:?}"
        );
    }
}

fn drain_layout_events(rounds: usize) {
    for _ in 0..rounds {
        while gtk4::glib::MainContext::default().iteration(false) {}
    }
}

fn usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).map_or(i32::MAX, |count| count)
}
