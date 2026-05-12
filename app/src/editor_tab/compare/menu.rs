use gettextrs::pgettext;
use gtk4::gio;
use gtk4::glib::variant::ToVariant;

use crate::settings::CompareViewMode;

pub(super) fn build_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let mode = gio::Menu::new();
    for view_mode in CompareViewMode::ALL {
        mode.append_item(&string_item(
            &mode_label(view_mode),
            "win.compare-view-mode",
            view_mode.nick(),
        ));
    }
    menu.append_section(Some(&pgettext("compare option", "Compare Mode")), &mode);

    let display = gio::Menu::new();
    display.append(
        Some(&pgettext("compare option", "Collapse Unchanged Lines")),
        Some("win.compare-collapse-unchanged"),
    );
    display.append(
        Some(&pgettext("compare option", "Wrap Unified Diff Lines")),
        Some("win.compare-word-wrap"),
    );
    menu.append_section(Some(&pgettext("compare menu section", "Display")), &display);

    let comparison = gio::Menu::new();
    comparison.append(
        Some(&pgettext(
            "compare option",
            "Ignore Leading/Trailing Whitespace",
        )),
        Some("win.compare-ignore-leading-trailing-whitespace"),
    );
    let context = gio::Menu::new();
    for lines in 1..=10 {
        context.append_item(&int_item(
            &lines.to_string(),
            "win.compare-context-lines",
            lines,
        ));
    }
    comparison.append_submenu(Some(&pgettext("compare option", "Context Lines")), &context);
    menu.append_section(
        Some(&pgettext("compare menu section", "Comparison")),
        &comparison,
    );
    menu
}

fn string_item(label: &str, action: &str, target: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    item.set_action_and_target_value(Some(action), Some(&target.to_variant()));
    item
}

fn int_item(label: &str, action: &str, target: i32) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), Some(action));
    item.set_action_and_target_value(Some(action), Some(&target.to_variant()));
    item
}

fn mode_label(mode: CompareViewMode) -> String {
    match mode {
        CompareViewMode::Adaptive => pgettext("compare mode", "Adaptive"),
        CompareViewMode::Split => pgettext("compare mode", "Split"),
        CompareViewMode::Unified => pgettext("compare mode", "Unified"),
    }
}
