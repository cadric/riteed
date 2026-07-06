use libadwaita as adw;
use std::fs;

use crate::gtk_tests::{
    build_window, build_window_with_settings, drain_events, spin_until, test_tmp_dir,
};
use crate::settings::AppSettings;
use gtk4::gio;
use gtk4::prelude::{FileExt, GtkWindowExt};

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

pub(crate) fn exercise_sidebar_restore_preference(test_app: &adw::Application) {
    let root = test_tmp_dir().join("riteed-lifecycle-project");
    let _removed = fs::remove_dir_all(&root);
    assert!(fs::create_dir_all(&root).is_ok());
    assert!(fs::write(root.join("A.txt"), b"a\n").is_ok());
    let root_uri = gio::File::for_path(&root).uri().to_string();
    let settings = AppSettings::new_for_tests();
    settings.set_project_folder_uri(&root_uri);
    settings.set_project_sidebar_visible(true);
    let writes_before = settings.write_log_for_tests().len();

    let window = build_window_with_settings(test_app, settings.clone());
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.present();
    spin_until("sidebar restore finds the remembered root", || {
        window.project_root_uri_for_tests().as_deref() == Some(root_uri.as_str())
    });
    let writes = settings.write_log_for_tests();
    assert!(
        !writes[writes_before..]
            .iter()
            .any(|key| key == "project-sidebar-visible"),
        "construction must not write project-sidebar-visible before restore"
    );
    spin_until("sidebar restores as visible", || {
        window.project_sidebar_visible_for_tests()
    });
    window.widget().close();
    drain_events(4);
    let _removed = fs::remove_dir_all(&root);
}
