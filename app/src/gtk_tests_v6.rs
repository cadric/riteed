use std::fs;

use gtk4::gio;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::gtk_tests::{build_window, spin_until, wait_millis, write_temp_file};
use crate::project_tree_model::ProjectTreeModel;
use crate::settings::AppSettings;
use crate::window::Window;
use crate::workspace::OpenSource;

#[cfg(unix)]
fn create_symlink(target: &std::path::Path, link: &std::path::Path) {
    let _removed = fs::remove_file(link);
    assert!(std::os::unix::fs::symlink(target, link).is_ok());
}

fn create_project_tree() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join("riteed-v6-project");
    let extra = std::env::temp_dir().join("riteed-v6-extra");
    let _removed = fs::remove_dir_all(&root);
    let _removed = fs::remove_dir_all(&extra);
    assert!(fs::create_dir_all(root.join("folder")).is_ok());
    assert!(fs::create_dir_all(&extra).is_ok());
    assert!(fs::write(root.join("b.txt"), b"bravo").is_ok());
    assert!(fs::write(root.join("A.txt"), b"alpha").is_ok());
    assert!(fs::write(root.join(".secret"), b"secret").is_ok());
    assert!(fs::write(root.join("folder").join("nested.txt"), b"nested").is_ok());
    (
        root,
        extra,
        write_temp_file("riteed-v6-open.txt", b"opened"),
    )
}

fn exercise_tree_model_expansion(root: &std::path::Path) {
    let model = ProjectTreeModel::new();
    model.set_root(Some(gio::File::for_path(root)));
    spin_until("tree model loads sorted visible entries", || {
        model.visible_entry_names_for_tests()
            == vec![
                String::from("folder"),
                String::from("A.txt"),
                String::from("b.txt"),
            ]
    });

    let row = model.model().row(0);
    assert!(row.is_some());
    let Some(row) = row else {
        return;
    };
    row.set_expanded(true);
    spin_until("tree model lazy-loads expanded directory", || {
        model
            .visible_entry_names_for_tests()
            .contains(&String::from("nested.txt"))
    });
    assert_eq!(model.snapshot_expanded_uris().len(), 1);
    model.set_show_hidden(true);
    spin_until("tree model show-hidden refreshes entries", || {
        model
            .visible_entry_names_for_tests()
            .contains(&String::from(".secret"))
    });
    model.set_root(None);
    assert!(model.visible_entry_names_for_tests().is_empty());
}

fn exercise_project_auto_refresh(window: &Window, root: &std::path::Path, project_uri: &str) {
    spin_until("project reveal selects opened file", || {
        window.selected_project_tree_uri_for_tests().as_deref() == Some(project_uri)
    });

    let auto_file = root.join("auto-created.txt");
    assert!(fs::write(&auto_file, b"auto").is_ok());
    window.trigger_project_auto_refresh_for_tests();
    spin_until("project auto refresh sees created root file", || {
        window
            .project_tree_entry_names_for_tests()
            .contains(&String::from("auto-created.txt"))
    });
    spin_until("project auto refresh preserves selected root file", || {
        window.selected_project_tree_uri_for_tests().as_deref() == Some(project_uri)
    });

    let renamed_file = root.join("auto-renamed.txt");
    assert!(fs::rename(&auto_file, &renamed_file).is_ok());
    window.trigger_project_auto_refresh_for_tests();
    spin_until("project auto refresh sees renamed root file", || {
        let names = window.project_tree_entry_names_for_tests();
        names.contains(&String::from("auto-renamed.txt"))
            && !names.contains(&String::from("auto-created.txt"))
    });
    assert!(fs::remove_file(&renamed_file).is_ok());
    window.trigger_project_auto_refresh_for_tests();
    spin_until("project auto refresh sees deleted root file", || {
        let names = window.project_tree_entry_names_for_tests();
        names.contains(&String::from("folder"))
            && !names.contains(&String::from("auto-renamed.txt"))
    });

    assert!(window.expand_project_tree_entry_for_tests("folder"));
    spin_until("project tree expands folder for auto refresh", || {
        window
            .project_tree_entry_names_for_tests()
            .contains(&String::from("nested.txt"))
    });
    let nested_auto = root.join("folder").join("auto-nested.txt");
    assert!(fs::write(&nested_auto, b"nested-auto").is_ok());
    window.trigger_project_auto_refresh_for_tests();
    spin_until("project auto refresh sees loaded subdir file", || {
        window
            .project_tree_entry_names_for_tests()
            .contains(&String::from("auto-nested.txt"))
    });
    assert!(window.project_monitor_count_for_tests() <= 2);
}

pub(crate) fn exercise_v6_project_navigation(test_app: &adw::Application) {
    let (root, extra, open_file) = create_project_tree();
    exercise_tree_model_expansion(&root);
    let window = build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };

    window.ensure_default_tab();
    window.handle_application_open(vec![
        gio::File::for_path(&root),
        gio::File::for_path(&extra),
        gio::File::for_path(&open_file),
    ]);
    let root_uri = gio::File::for_path(&root).uri().to_string();
    spin_until("app open activates first project folder", || {
        window.project_root_uri_for_tests().as_deref() == Some(root_uri.as_str())
            && window.project_sidebar_visible_for_tests()
            && window.selected_saved_uri_for_tests() == gio::File::for_path(&open_file).uri()
    });
    assert_eq!(
        window.project_action_states_for_tests(),
        (true, true, true, true)
    );

    spin_until("project tree loads visible entries", || {
        window.project_tree_entry_names_for_tests()
            == vec![
                String::from("folder"),
                String::from("A.txt"),
                String::from("b.txt"),
            ]
    });
    window.set_project_show_hidden_for_tests(true);
    spin_until("show hidden refreshes project tree", || {
        window
            .project_tree_entry_names_for_tests()
            .contains(&String::from(".secret"))
    });

    window.refresh_project_for_tests();
    spin_until("manual project refresh preserves entries", || {
        window
            .project_tree_entry_names_for_tests()
            .contains(&String::from("A.txt"))
    });
    let project_file = root.join("A.txt");
    let project_uri = gio::File::for_path(&project_file).uri().to_string();
    window.request_open_files(
        vec![gio::File::for_path(&project_file)],
        OpenSource::AppOpen,
    );
    spin_until("project file opens and reveal sync runs", || {
        window.selected_text_for_tests() == "alpha"
    });
    assert_eq!(window.selected_saved_uri_for_tests(), project_uri);
    window.refresh_project_for_tests();
    wait_millis("project reveal timer drains", 80);
    exercise_project_auto_refresh(&window, &root, &project_uri);

    #[cfg(unix)]
    {
        let link = root.join("linked.txt");
        create_symlink(&root.join("b.txt"), &link);
        window.resolve_project_symlink_for_tests(&gio::File::for_path(&link));
        spin_until("project symlink opens target contents", || {
            window.selected_text_for_tests() == "bravo"
        });
        let folder_link = root.join("linked-folder");
        create_symlink(&root.join("folder"), &folder_link);
        window.resolve_project_symlink_for_tests(&gio::File::for_path(&folder_link));
        wait_millis("project folder symlink settles", 80);

        let device_link = root.join("linked-device");
        create_symlink(std::path::Path::new("/dev/null"), &device_link);
        window.resolve_project_symlink_for_tests(&gio::File::for_path(&device_link));
        wait_millis("project device symlink settles", 80);

        let broken_link = root.join("broken-link");
        create_symlink(&root.join("missing.txt"), &broken_link);
        window.resolve_project_symlink_for_tests(&gio::File::for_path(&broken_link));
        wait_millis("project broken symlink settles", 80);
        assert_eq!(window.selected_text_for_tests(), "bravo");
    }

    window.close_project_for_tests();
    assert_eq!(window.project_root_uri_for_tests(), None);
    assert_eq!(window.project_monitor_count_for_tests(), 0);
    assert_eq!(
        window.project_action_states_for_tests(),
        (false, false, false, false)
    );
    assert!(!window.project_sidebar_visible_for_tests());

    let _removed = fs::remove_dir_all(root);
    let _removed = fs::remove_dir_all(extra);
    let _removed = fs::remove_file(open_file);
}

pub(crate) fn exercise_v6_project_restore(test_app: &adw::Application) {
    let (root, _extra, remembered_file) = create_project_tree();
    let root_uri = gio::File::for_path(&root).uri().to_string();
    let settings = AppSettings::new_for_tests();
    settings.set_project_folder_uri(&root_uri);
    settings.set_project_sidebar_visible(false);
    let restored = Window::new_with_settings_for_tests(test_app, settings.clone()).ok();
    assert!(restored.is_some());
    let Some(restored) = restored else {
        return;
    };
    spin_until("restore activates remembered project", || {
        restored.project_root_uri_for_tests().as_deref() == Some(root_uri.as_str())
    });
    assert!(!restored.project_sidebar_visible_for_tests());
    assert_eq!(
        restored.project_action_states_for_tests(),
        (true, true, true, true)
    );

    let failed_settings = AppSettings::new_for_tests();
    let file_uri = gio::File::for_path(&remembered_file).uri().to_string();
    failed_settings.set_project_folder_uri(&file_uri);
    failed_settings.set_project_folder_display_name("Remembered");
    failed_settings.set_project_sidebar_visible(true);
    let failed = Window::new_with_settings_for_tests(test_app, failed_settings.clone()).ok();
    assert!(failed.is_some());
    let Some(failed) = failed else {
        return;
    };
    spin_until("restore failure leaves runtime without root", || {
        failed.project_root_uri_for_tests().is_none()
    });
    assert_eq!(failed_settings.project_folder_uri(), file_uri);
    assert_eq!(failed_settings.project_folder_display_name(), "Remembered");
    assert_eq!(
        failed.project_action_states_for_tests(),
        (false, false, false, false)
    );

    let _removed = fs::remove_dir_all(root);
    let _removed = fs::remove_file(remembered_file);
}
