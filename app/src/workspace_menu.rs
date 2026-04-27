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
    menu.append(Some(&pgettext("menu item", "Open...")), Some("app.open"));
    menu.append(
        Some(&pgettext("menu item", "Open Folder...")),
        Some("app.open-folder"),
    );
    menu.append(
        Some(&pgettext("menu item", "Recent Files...")),
        Some("win.recent-files"),
    );
    menu.append(Some(&pgettext("menu item", "Search")), Some("win.search"));

    let compare_item = gio::MenuItem::new(
        Some(&pgettext("menu item", "Compare...")),
        Some("win.compare"),
    );
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

pub(crate) fn build_primary_popover() -> gtk4::PopoverMenu {
    let menu = build_primary_menu();
    let popover = gtk4::PopoverMenu::from_model(Some(&menu));
    popover.set_width_request(320);
    popover
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::MenuModelExt;

    use super::build_primary_menu;

    #[test]
    fn primary_menu_contains_core_actions() {
        let menu = build_primary_menu();
        assert!(menu.n_items() >= 10);
    }
}
