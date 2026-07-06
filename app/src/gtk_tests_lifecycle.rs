use libadwaita as adw;

use crate::gtk_tests::{build_window, build_window_with_settings, drain_events, spin_until};
use crate::settings::AppSettings;
use gtk4::prelude::GtkWindowExt;

pub(crate) fn exercise_window_lifecycle_release(test_app: &adw::Application) {
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.present();
    drain_events(4);
    let workspace_weak = window.workspace_weak_for_tests();
    let source_control_weak = window.source_control_state_weak_for_tests();
    window.widget().close();
    drop(window);
    spin_until("closed window releases its workspace", || {
        workspace_weak.upgrade().is_none()
    });
    spin_until("closed window releases its source-control state", || {
        source_control_weak.upgrade().is_none()
    });
    assert!(workspace_weak.upgrade().is_none());
    assert!(source_control_weak.upgrade().is_none());
}

pub(crate) fn exercise_clean_close_persists_window_size(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    let window = build_window_with_settings(test_app, settings.clone());
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.present();
    drain_events(4);
    assert!(
        !settings
            .write_log_for_tests()
            .iter()
            .any(|key| key == "window-width"),
        "no size write is expected before close"
    );
    window.widget().close();
    drain_events(8);
    assert!(
        settings
            .write_log_for_tests()
            .iter()
            .any(|key| key == "window-width"),
        "a clean close must persist the window size"
    );
}
