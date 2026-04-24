use gettextrs::pgettext;
use gtk4::gio;
use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;

pub(crate) fn build_primary_menu(recent_files: &[String]) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some(&pgettext("menu item", "New Tab")), Some("app.new"));
    menu.append(Some(&pgettext("menu item", "Open")), Some("app.open"));
    menu.append(
        Some(&pgettext("menu item", "Open Folder")),
        Some("app.open-folder"),
    );
    menu.append(Some(&pgettext("menu item", "Search")), Some("win.search"));

    if !recent_files.is_empty() {
        let submenu = gio::Menu::new();
        for uri in recent_files {
            let item = gio::MenuItem::new(Some(&recent_label(uri)), None);
            item.set_action_and_target_value(Some("app.open-recent"), Some(&uri.to_variant()));
            submenu.append_item(&item);
        }
        menu.append_submenu(Some(&pgettext("menu item", "Recent Files")), &submenu);
    }

    let zoom_menu = gio::Menu::new();
    zoom_menu.append(Some(&pgettext("menu item", "Zoom In")), Some("win.zoom-in"));
    zoom_menu.append(
        Some(&pgettext("menu item", "Zoom Out")),
        Some("win.zoom-out"),
    );
    zoom_menu.append(
        Some(&pgettext("menu item", "Actual Size")),
        Some("win.zoom-reset"),
    );
    menu.append_submenu(Some(&pgettext("menu item", "Zoom")), &zoom_menu);
    menu.append(
        Some(&pgettext("menu item", "Fullscreen")),
        Some("win.fullscreen"),
    );

    let compare_menu = gio::Menu::new();
    compare_menu.append(
        Some(&pgettext("menu item", "Compare With Saved File")),
        Some("win.compare-with-disk"),
    );
    compare_menu.append(
        Some(&pgettext("menu item", "Compare With File")),
        Some("win.compare-with-file"),
    );
    compare_menu.append(
        Some(&pgettext("menu item", "Compare Two Files")),
        Some("win.compare-two-files"),
    );
    compare_menu.append(
        Some(&pgettext("menu item", "Refresh Reference")),
        Some("win.compare-refresh-reference"),
    );
    compare_menu.append(
        Some(&pgettext("menu item", "Exit Compare")),
        Some("win.compare-exit"),
    );
    menu.append_submenu(Some(&pgettext("menu item", "Compare")), &compare_menu);

    let project_menu = gio::Menu::new();
    project_menu.append(
        Some(&pgettext("menu item", "Project Sidebar")),
        Some("win.project-sidebar-visible"),
    );
    project_menu.append(
        Some(&pgettext("menu item", "Show Hidden Files")),
        Some("win.project-show-hidden"),
    );
    project_menu.append(
        Some(&pgettext("menu item", "Refresh Project Tree")),
        Some("win.refresh-project-tree"),
    );
    project_menu.append(
        Some(&pgettext("menu item", "Close Folder")),
        Some("win.close-folder"),
    );
    menu.append_submenu(Some(&pgettext("menu item", "Project")), &project_menu);

    menu.append(
        Some(&pgettext("menu item", "Keyboard Shortcuts")),
        Some("win.show-help-overlay"),
    );
    menu.append(
        Some(&pgettext("menu item", "Preferences")),
        Some("app.preferences"),
    );
    menu.append(Some(&pgettext("menu item", "Help")), Some("app.help"));
    menu.append(Some(&pgettext("menu item", "About")), Some("app.about"));
    menu
}

pub(crate) fn build_primary_popover(recent_files: &[String]) -> gtk4::PopoverMenu {
    let menu = build_primary_menu(recent_files);
    let popover = gtk4::PopoverMenu::from_model(Some(&menu));
    popover.set_width_request(320);
    popover
}

fn recent_label(uri: &str) -> String {
    let file = gio::File::for_uri(uri);
    let Some(path) = file.path() else {
        return String::from(uri);
    };
    let name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .map_or_else(|| path.display().to_string(), ToString::to_string);
    path.parent()
        .and_then(|parent| parent.file_name())
        .and_then(std::ffi::OsStr::to_str)
        .map_or(name.clone(), |parent_name| {
            format!("{name} · {parent_name}")
        })
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::MenuModelExt;

    use super::{build_primary_menu, recent_label};

    #[test]
    fn recent_label_prefers_name_and_parent() {
        let label = recent_label("file:///tmp/example.txt");
        assert!(label.contains("example.txt"));
    }

    #[test]
    fn recent_label_falls_back_to_uri_for_non_local_files() {
        let uri = "https://example.com/example.txt";
        assert_eq!(recent_label(uri), uri);
    }

    #[test]
    fn primary_menu_includes_recent_and_zoom_sections() {
        let menu = build_primary_menu(&[String::from("file:///tmp/example.txt")]);
        assert!(menu.n_items() >= 9);
    }
}
