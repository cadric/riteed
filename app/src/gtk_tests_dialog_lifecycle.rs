use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs::lifecycle::{
    assert_dialog_leak_counters_clear_for_tests, dialog_leak_counters_clear_for_tests,
    reset_dialog_leak_counters_for_tests,
};
use crate::gtk_tests::{build_window, drain_events, spin_until};
use crate::settings::AppSettings;

pub(crate) fn exercise_dialog_lifecycle(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    window.present();
    reset_dialog_leak_counters_for_tests();

    for _ in 0..10 {
        close_dialog(window.present_compare_dialog_for_tests());
        wait_for_clear("compare dialog state drops after close");
    }

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
        close_dialog(crate::dialogs::encoding::choose_encoding_dialog_for_tests(
            window.widget(),
        ));
        wait_for_clear("encoding dialog state drops after close");
    }

    assert_dialog_leak_counters_clear_for_tests();
}

fn close_dialog(dialog: adw::Dialog) {
    let _closed = dialog.close();
    drop(dialog);
    drain_events(16);
}

fn wait_for_clear(label: &str) {
    spin_until(label, dialog_leak_counters_clear_for_tests);
}
