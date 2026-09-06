use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs::lifecycle::{
    assert_dialog_leak_counters_clear_for_tests, dialog_leak_counters_clear_for_tests,
    reset_dialog_leak_counters_for_tests,
};
use crate::gtk_tests::{build_window, drain_events, spin_until};
use crate::settings::AppSettings;

pub(crate) fn exercise_dialog_lifecycle(test_app: &adw::Application) {
    let window =
        build_window(test_app).unwrap_or_else(|| unreachable!("dialog lifecycle GTK window"));
    window.ensure_default_tab();
    window.present();
    reset_dialog_leak_counters_for_tests();

    for _ in 0..10 {
        close_dialog(window.present_compare_paste_text_dialog_for_tests());
        wait_for_clear("paste text dialog state drops after close");
    }

    let recent_settings = AppSettings::new_for_tests();
    for _ in 0..10 {
        close_dialog(
            crate::dialogs::recent_files::show_recent_files_dialog_for_tests(
                window.widget(),
                &recent_settings,
            ),
        );
        wait_for_clear("recent files dialog state drops after close");
    }

    for _ in 0..10 {
        let dialog = crate::dialogs::encoding::choose_encoding_dialog_for_tests(window.widget());
        drain_events(4);
        assert_encoding_dialog_chrome(&dialog);
        close_dialog(dialog);
        wait_for_clear("encoding dialog state drops after close");
    }

    assert_dialog_leak_counters_clear_for_tests();
}

fn assert_encoding_dialog_chrome(dialog: &adw::Dialog) {
    let root = dialog.clone().upcast::<gtk4::Widget>();
    let widgets = descendant_widgets(&root);
    let has_visible_title = widgets.iter().any(|widget| {
        widget
            .clone()
            .downcast::<adw::WindowTitle>()
            .is_ok_and(|title| title.is_visible() && title.title().as_str() == "Choose")
    });
    let has_visible_close_button = widgets.iter().any(visible_close_button);
    assert_eq!(
        (has_visible_title, has_visible_close_button),
        (true, true),
        "encoding dialog must expose its visible title and real close control"
    );
}

fn visible_close_button(widget: &gtk4::Widget) -> bool {
    let Ok(button) = widget.clone().downcast::<gtk4::Button>() else {
        return false;
    };
    button.is_visible()
        && button.accessible_role() == gtk4::AccessibleRole::Button
        && descendant_widgets(button.upcast_ref()).iter().any(|child| {
            child
                .clone()
                .downcast::<gtk4::Image>()
                .is_ok_and(|image| image.icon_name().as_deref() == Some("window-close-symbolic"))
        })
}

fn descendant_widgets(root: &gtk4::Widget) -> Vec<gtk4::Widget> {
    let mut pending = vec![root.clone()];
    let mut widgets = Vec::new();
    while let Some(widget) = pending.pop() {
        let mut child = widget.first_child();
        while let Some(next) = child {
            child = next.next_sibling();
            pending.push(next);
        }
        widgets.push(widget);
    }
    widgets
}

fn close_dialog(dialog: adw::Dialog) {
    let _closed = dialog.close();
    drop(dialog);
    drain_events(16);
}

fn wait_for_clear(label: &str) {
    spin_until(label, dialog_leak_counters_clear_for_tests);
}
