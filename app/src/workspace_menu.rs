use gettextrs::pgettext;
use gtk4::gio;
use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;

const THEME_CUSTOM_ID: &str = "theme";

pub(crate) fn build_primary_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append_section(None, &theme_section());
    menu.append_section(None, &workflow_section());
    menu.append_section(None, &standard_section());
    menu
}

fn workflow_section() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(
        Some(&pgettext("menu item", "New Window")),
        Some("app.new-window"),
    );
    section.append(Some(&pgettext("menu item", "Find")), Some("win.search"));
    section.append(
        Some(&pgettext("menu item", "Find and Replace")),
        Some("win.replace"),
    );
    section.append(
        Some(&pgettext("menu item", "Find in Files")),
        Some("win.find-in-files"),
    );
    section.append(
        Some(&pgettext("menu item", "Document Statistics")),
        Some("win.document-statistics"),
    );
    section.append(Some(&pgettext("menu item", "Print")), Some("win.print"));
    section
}

fn standard_section() -> gio::Menu {
    let section = gio::Menu::new();
    section.append(
        Some(&pgettext("menu item", "Preferences")),
        Some("app.preferences"),
    );
    section.append(
        Some(&pgettext("menu item", "Keyboard Shortcuts")),
        Some("win.show-help-overlay"),
    );
    section.append(Some(&pgettext("menu item", "Help")), Some("app.help"));
    section.append(
        Some(&pgettext("menu item", "About Riteed")),
        Some("app.about"),
    );
    section
}

fn theme_section() -> gio::Menu {
    let section = gio::Menu::new();
    let item = gio::MenuItem::new(None, None);
    item.set_attribute_value("custom", Some(&THEME_CUSTOM_ID.to_variant()));
    section.append_item(&item);
    section
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
    let _added = popover.add_child(&crate::window_theme::build_selector(), THEME_CUSTOM_ID);
    popover.set_width_request(320);
    popover
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::*;

    use super::{build_open_menu, build_primary_menu};

    #[test]
    fn primary_menu_uses_hig_sections() {
        let menu = build_primary_menu();
        assert_eq!(menu.n_items(), 3);

        let theme = section(&menu, 0);
        assert_eq!(theme.n_items(), 1);
        assert_eq!(item_string(&theme, 0, "custom").as_deref(), Some("theme"));
        assert_eq!(item_string(&theme, 0, "action"), None);

        let workflow = section(&menu, 1);
        assert_menu_labels(
            &workflow,
            &[
                "New Window",
                "Find",
                "Find and Replace",
                "Find in Files",
                "Document Statistics",
                "Print",
            ],
        );
        assert_menu_actions(
            &workflow,
            &[
                "app.new-window",
                "win.search",
                "win.replace",
                "win.find-in-files",
                "win.document-statistics",
                "win.print",
            ],
        );

        let standard = section(&menu, 2);
        assert_menu_labels(
            &standard,
            &["Preferences", "Keyboard Shortcuts", "Help", "About Riteed"],
        );
        assert_menu_actions(
            &standard,
            &[
                "app.preferences",
                "win.show-help-overlay",
                "app.help",
                "app.about",
            ],
        );
        assert_menu_excludes_actions(&menu, &["app.open", "app.open-folder", "win.recent-files"]);
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

    fn assert_menu_labels(menu: &impl MenuModelExt, expected: &[&str]) {
        let labels = menu_labels(menu);
        let expected = expected_strings(expected);
        assert_eq!(labels, expected);
    }

    fn assert_menu_item(
        menu: &impl MenuModelExt,
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

    fn assert_menu_actions(menu: &impl MenuModelExt, expected: &[&str]) {
        let actions = menu_actions(menu);
        let expected = expected_strings(expected);
        assert_eq!(actions, expected);
    }

    fn assert_menu_excludes_actions(menu: &impl MenuModelExt, removed: &[&str]) {
        let actions = menu_actions(menu);
        for removed_action in removed {
            assert!(!actions.iter().any(|action| action == removed_action));
        }
    }

    fn menu_labels(menu: &impl MenuModelExt) -> Vec<String> {
        (0..menu.n_items())
            .filter_map(|index| item_string(menu, index, "label"))
            .collect()
    }

    fn menu_actions(menu: &impl MenuModelExt) -> Vec<String> {
        let mut actions = Vec::new();
        for index in 0..menu.n_items() {
            if let Some(action) = item_string(menu, index, "action") {
                actions.push(action);
            }
            if let Some(section) = menu.item_link(index, "section") {
                actions.extend(menu_actions(&section));
            }
        }
        actions
    }

    fn section(menu: &impl MenuModelExt, index: i32) -> gtk4::gio::MenuModel {
        let section = menu.item_link(index, "section");
        assert!(section.is_some());
        section.unwrap_or_else(|| gtk4::gio::Menu::new().upcast())
    }

    fn item_string(menu: &impl MenuModelExt, index: i32, attribute: &str) -> Option<String> {
        menu.item_attribute_value(index, attribute, None)
            .and_then(|value| value.get::<String>())
    }

    fn expected_strings(expected: &[&str]) -> Vec<String> {
        expected.iter().map(|item| String::from(*item)).collect()
    }
}
