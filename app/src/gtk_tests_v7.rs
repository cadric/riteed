use std::fs;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::gtk_tests::{build_window, drain_events, spin_until, write_temp_file};
use crate::workspace::OpenSource;

pub(crate) fn exercise_v7_compare(test_app: &adw::Application) {
    exercise_compare_with_disk_and_file(test_app);
    exercise_compare_navigation(test_app);
    exercise_compare_two_files(test_app);
    exercise_compare_dialog_entry(test_app);
    exercise_compare_exits_on_open(test_app);
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

fn collect_window_titles(root: &gtk4::Widget, title: &str) -> Vec<adw::WindowTitle> {
    let mut stack = vec![root.clone()];
    let mut matches = Vec::new();
    while let Some(widget) = stack.pop() {
        if let Ok(window_title) = widget.clone().downcast::<adw::WindowTitle>()
            && window_title.title().as_str() == title
        {
            matches.push(window_title);
        }

        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            stack.push(next);
        }
    }
    matches
}

fn click_first_button(root: &gtk4::Widget, label: &str) {
    if let Some(button) = collect_buttons(root, label).first() {
        button.emit_clicked();
    }
}

fn wait_for_button(root: &gtk4::Widget, label: &str, reason: &str) {
    spin_until(reason, || !collect_buttons(root, label).is_empty());
}

fn wait_for_sensitive_button(root: &gtk4::Widget, label: &str, reason: &str) {
    spin_until(reason, || {
        collect_buttons(root, label)
            .first()
            .is_some_and(gtk4::prelude::WidgetExt::is_sensitive)
    });
}

fn wait_for_window_title(root: &gtk4::Widget, title: &str, reason: &str) {
    spin_until(reason, || !collect_window_titles(root, title).is_empty());
}

fn exercise_compare_with_disk_and_file(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.set_minimap_for_tests(true);

    let editable_path = write_temp_file("riteed-v7-editable.txt", b"a\nb\nc\n");
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
        (true, false, false, false, false)
    );

    window.compare_with_disk_for_tests();
    spin_until("v7 compare with disk starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_status_for_tests() == "No differences were found."
    });
    assert!(!window.selected_minimap_visible_for_tests());
    assert_eq!(
        window.compare_action_states_for_tests(),
        (false, true, true, true, true)
    );

    window.set_selected_text_for_tests("a\nchanged\nc");
    spin_until("v7 compare updates after editing", || {
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

    let reference_path = write_temp_file("riteed-v7-reference.txt", b"x\nchanged\nc\n");
    window.compare_with_file_for_tests(&gio::File::for_path(&reference_path));
    spin_until("v7 compare with file starts", || {
        window.selected_compare_active_for_tests()
            && window.selected_compare_diff_count_for_tests() == 1
            && window.selected_compare_highlight_count_for_tests() > 0
    });
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
        "riteed-v7-nav.txt",
        b"0\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n",
    );
    let nav_ref = write_temp_file(
        "riteed-v7-nav-ref.txt",
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
    let left = write_temp_file("riteed-v7-two-left.txt", b"left\nsame\n");
    let right = write_temp_file("riteed-v7-two-right.txt", b"right\nsame\n");
    window.compare_two_files_for_tests(&gio::File::for_path(&left), &gio::File::for_path(&right));
    spin_until("v7 compare two files opens left and reference", || {
        window
            .selected_saved_uri_for_tests()
            .ends_with("riteed-v7-two-left.txt")
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
    let compare_path = write_temp_file("riteed-v7-exit-compare.txt", b"compare\n");
    let reference_path = write_temp_file("riteed-v7-exit-reference.txt", b"reference\n");
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

    let replacement_path = write_temp_file("riteed-v7-replacement.txt", b"replacement");
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

fn exercise_compare_dialog_entry(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let compare_path = write_temp_file("riteed-v7-dialog-entry.txt", b"before\n");
    let compare_uri = gio::File::for_path(&compare_path).uri().to_string();
    window.request_open_files(
        vec![gio::File::for_path(&compare_path)],
        OpenSource::AppOpen,
    );
    spin_until("v7 compare dialog file opened", || {
        window.selected_saved_uri_for_tests() == compare_uri
    });
    window.set_selected_text_for_tests("before\nchanged");
    window.present();
    assert!(
        gtk4::prelude::WidgetExt::activate_action(window.widget(), "win.compare", None).is_ok()
    );
    drain_events(16);

    let root_widget = window.widget().clone().upcast::<gtk4::Widget>();
    wait_for_window_title(&root_widget, "Compare", "v7 compare dialog title shown");
    wait_for_button(&root_widget, "Saved Version", "v7 compare dialog shown");
    assert!(collect_buttons(&root_widget, "Cancel").is_empty());

    let right_buttons_parent = collect_buttons(&root_widget, "Saved Version")
        .first()
        .and_then(gtk4::prelude::WidgetExt::parent);
    let left_buttons_parent = collect_buttons(&root_widget, "Current Document")
        .first()
        .and_then(gtk4::prelude::WidgetExt::parent);
    assert!(right_buttons_parent.is_some());
    assert!(left_buttons_parent.is_some());
    let Some(right_buttons_parent) = right_buttons_parent else {
        return;
    };
    let Some(left_buttons_parent) = left_buttons_parent else {
        return;
    };

    click_first_button(&right_buttons_parent, "Clear");
    drain_events(16);

    click_first_button(&left_buttons_parent, "Paste Text...");
    drain_events(16);

    wait_for_button(&root_widget, "Use Text", "v7 compare paste dialog opened");
    assert_eq!(collect_buttons(&root_widget, "Cancel").len(), 1);
    click_first_button(&root_widget, "Use Text");
    drain_events(16);

    click_first_button(&right_buttons_parent, "Paste Text...");
    drain_events(16);

    wait_for_button(
        &root_widget,
        "Use Text",
        "v7 compare paste dialog opened again",
    );
    click_first_button(&root_widget, "Use Text");
    drain_events(16);

    wait_for_sensitive_button(&root_widget, "Swap", "v7 compare dialog swap enabled");
    click_first_button(&root_widget, "Swap");
    drain_events(16);

    wait_for_sensitive_button(
        &root_widget,
        "Compare",
        "v7 compare dialog compare button enabled",
    );
    click_first_button(&root_widget, "Compare");
    spin_until("v7 compare started from dialog", || {
        window.selected_compare_active_for_tests()
    });
    window.exit_compare_for_tests();
    drain_events(16);

    let _removed = fs::remove_file(compare_path);
}
