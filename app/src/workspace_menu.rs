use gettextrs::pgettext;
use gtk4::gio;
use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;

pub(crate) fn build_primary_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&pgettext("menu item", "New Window")),
        Some("app.new-window"),
    );
    menu.append(Some(&pgettext("menu item", "Find")), Some("win.search"));

    let compare_label = ellipsis_label(pgettext("menu item", "Compare"));
    let compare_item = gio::MenuItem::new(Some(&compare_label), Some("win.compare"));
    compare_item.set_attribute_value("hidden-when", Some(&"action-missing".to_variant()));
    menu.append_item(&compare_item);

    menu.append(
        Some(&pgettext("menu item", "Keyboard Shortcuts")),
        Some("win.show-help-overlay"),
    );
    menu.append(
        Some(&pgettext("menu item", "Preferences")),
        Some("app.preferences"),
    );
    menu.append(
        Some(&pgettext("menu item", "Appearance")),
        Some("win.appearance"),
    );
    menu.append(Some(&pgettext("menu item", "Help")), Some("app.help"));
    menu.append(
        Some(&pgettext("menu item", "About Riteed")),
        Some("app.about"),
    );
    menu
}

pub(crate) fn build_open_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let open_label = ellipsis_label(pgettext("menu item", "Open Files"));
    menu.append_item(&open_menu_item(
        &open_label,
        "app.open",
        "document-open-symbolic",
    ));
    let folder_label = ellipsis_label(pgettext("menu item", "Open Folder"));
    menu.append_item(&open_menu_item(
        &folder_label,
        "app.open-folder",
        "folder-open-symbolic",
    ));
    let recent_label = ellipsis_label(pgettext("menu item", "Recent Files"));
    menu.append_item(&open_menu_item(
        &recent_label,
        "win.recent-files",
        "document-open-recent-symbolic",
    ));
    menu
}

fn open_menu_item(label: &str, action: &str, icon_name: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    item.set_attribute_value("verb-icon", Some(&icon_name.to_variant()));
    item
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

pub(crate) fn build_primary_popover() -> gtk4::PopoverMenu {
    let menu = build_primary_menu();
    let popover = gtk4::PopoverMenu::from_model(Some(&menu));
    popover.set_width_request(320);
    popover
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::MenuModelExt;

    use super::{build_open_menu, build_primary_menu};

    #[test]
    fn primary_menu_contains_core_actions() {
        let menu = build_primary_menu();
        assert_menu_labels(
            &menu,
            &[
                "New Window",
                "Find",
                "Compare…",
                "Keyboard Shortcuts",
                "Preferences",
                "Appearance",
                "Help",
                "About Riteed",
            ],
        );
        assert_menu_excludes(
            &menu,
            &[
                "Open...",
                "Open Folder...",
                "Recent Files...",
                "Search",
                "Open Text File",
                "Open Text File…",
                "Open Files",
                "Open Files…",
                "Open Folder",
                "Open Folder…",
                "Recent Files",
                "Recent Files…",
            ],
        );
    }

    #[test]
    fn open_menu_contains_icon_actions() {
        let menu = build_open_menu();
        assert_eq!(menu.n_items(), 3);
        assert_menu_item(
            &menu,
            0,
            "Open Files…",
            "app.open",
            "document-open-symbolic",
        );
        assert_menu_item(
            &menu,
            1,
            "Open Folder…",
            "app.open-folder",
            "folder-open-symbolic",
        );
        assert_menu_item(
            &menu,
            2,
            "Recent Files…",
            "win.recent-files",
            "document-open-recent-symbolic",
        );
    }

    fn assert_menu_labels(menu: &gtk4::gio::Menu, expected: &[&str]) {
        let labels = menu_labels(menu);
        for expected_label in expected {
            assert!(labels.iter().any(|label| label == expected_label));
        }
    }

    fn assert_menu_excludes(menu: &gtk4::gio::Menu, removed: &[&str]) {
        let labels = menu_labels(menu);
        for removed_label in removed {
            assert!(!labels.iter().any(|label| label == removed_label));
        }
    }

    fn assert_menu_item(
        menu: &gtk4::gio::Menu,
        index: i32,
        label: &str,
        action: &str,
        icon_name: &str,
    ) {
        assert_eq!(item_string(menu, index, "label").as_deref(), Some(label));
        assert_eq!(item_string(menu, index, "action").as_deref(), Some(action));
        assert_eq!(
            item_string(menu, index, "verb-icon").as_deref(),
            Some(icon_name)
        );
    }

    fn menu_labels(menu: &gtk4::gio::Menu) -> Vec<String> {
        (0..menu.n_items())
            .filter_map(|index| item_string(menu, index, "label"))
            .collect()
    }

    fn item_string(menu: &gtk4::gio::Menu, index: i32, attribute: &str) -> Option<String> {
        menu.item_attribute_value(index, attribute, None)
            .and_then(|value| value.get::<String>())
    }
}
