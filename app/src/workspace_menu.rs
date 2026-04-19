use gettextrs::pgettext;
use gtk4::gio;
use gtk4::glib::variant::ToVariant;
use gtk4::prelude::*;

pub(crate) fn build_primary_menu(recent_files: &[String]) -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some(&pgettext("menu item", "New Tab")), Some("app.new"));
    menu.append(Some(&pgettext("menu item", "Open")), Some("app.open"));
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
    use super::recent_label;

    #[test]
    fn recent_label_prefers_name_and_parent() {
        let label = recent_label("file:///tmp/example.txt");
        assert!(label.contains("example.txt"));
    }
}
