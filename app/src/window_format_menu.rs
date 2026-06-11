use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, glib, prelude::*};

use crate::editor_format::LineEndingMode;
use crate::editor_tab::EditorTab;
use crate::workspace::Workspace;

pub fn create_actions() -> (gio::SimpleAction, gio::SimpleAction) {
    let change_encoding = gio::SimpleAction::new("change-encoding", None);
    let line_ending = gio::SimpleAction::new_stateful(
        "line-ending",
        Some(glib::VariantTy::STRING),
        &line_ending_nick(LineEndingMode::Lf).to_variant(),
    );
    (change_encoding, line_ending)
}

pub fn build_menu() -> gio::Menu {
    let menu = gio::Menu::new();
    let encoding_section = gio::Menu::new();
    encoding_section.append(
        Some(&pgettext("format menu", "Change Encoding…")),
        Some("win.change-encoding"),
    );
    menu.append_section(None, &encoding_section);

    let line_section = gio::Menu::new();
    for mode in [LineEndingMode::Lf, LineEndingMode::CrLf, LineEndingMode::Cr] {
        let item = gio::MenuItem::new(Some(&mode.menu_label()), None);
        item.set_action_and_target_value(
            Some("win.line-ending"),
            Some(&line_ending_nick(mode).to_variant()),
        );
        line_section.append_item(&item);
    }
    menu.append_section(Some(&pgettext("format menu", "Line Endings")), &line_section);
    menu
}

pub fn install(
    change_encoding: &gio::SimpleAction,
    line_ending: &gio::SimpleAction,
    workspace: &Rc<Workspace>,
) {
    let weak = Rc::downgrade(workspace);
    change_encoding.connect_activate(move |_, _| {
        if let Some(workspace) = weak.upgrade() {
            workspace.request_selected_encoding_action();
        }
    });

    let weak = Rc::downgrade(workspace);
    line_ending.connect_activate(move |_, parameter| {
        let Some(workspace) = weak.upgrade() else {
            return;
        };
        let Some(nick) = parameter.and_then(glib::Variant::str) else {
            return;
        };
        let Some(mode) = mode_from_nick(nick) else {
            return;
        };
        workspace.set_selected_line_ending_mode(mode);
    });

    let sync_change_encoding = change_encoding.clone();
    let sync_line_ending = line_ending.clone();
    workspace.set_format_preferences_handler(Rc::new(move |tab| {
        sync_actions(&sync_change_encoding, &sync_line_ending, tab.as_deref());
    }));
    sync_actions(
        change_encoding,
        line_ending,
        workspace.selected_tab().as_deref(),
    );
}

fn sync_actions(
    change_encoding: &gio::SimpleAction,
    line_ending: &gio::SimpleAction,
    tab: Option<&EditorTab>,
) {
    if let Some(tab) = tab {
        let format = tab.current_format();
        change_encoding.set_enabled(
            tab.is_document() && (tab.document_uri().is_none() || tab.can_reopen_with_encoding()),
        );
        line_ending.set_enabled(tab.is_document());
        line_ending.set_state(&line_ending_nick(format.line_ending_mode()).to_variant());
    } else {
        change_encoding.set_enabled(false);
        line_ending.set_enabled(false);
        line_ending.set_state(&line_ending_nick(LineEndingMode::Lf).to_variant());
    }
}

fn line_ending_nick(mode: LineEndingMode) -> &'static str {
    match mode {
        LineEndingMode::Lf => "lf",
        LineEndingMode::CrLf => "crlf",
        LineEndingMode::Cr => "cr",
    }
}

fn mode_from_nick(nick: &str) -> Option<LineEndingMode> {
    match nick {
        "lf" => Some(LineEndingMode::Lf),
        "crlf" => Some(LineEndingMode::CrLf),
        "cr" => Some(LineEndingMode::Cr),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::*;

    use super::{build_menu, line_ending_nick, mode_from_nick};
    use crate::editor_format::LineEndingMode;

    #[test]
    fn line_ending_nicks_round_trip() {
        for mode in [LineEndingMode::Lf, LineEndingMode::CrLf, LineEndingMode::Cr] {
            assert_eq!(mode_from_nick(line_ending_nick(mode)), Some(mode));
        }
    }

    #[test]
    fn unknown_nick_is_rejected() {
        assert_eq!(mode_from_nick("mixed"), None);
    }

    #[test]
    fn format_menu_exposes_encoding_and_line_ending_items() {
        let menu = build_menu();
        assert_eq!(menu.n_items(), 2);

        let encoding = section(&menu, 0);
        assert_eq!(encoding.n_items(), 1);
        assert_eq!(
            item_string(&encoding, 0, "label").as_deref(),
            Some("Change Encoding…")
        );
        assert_eq!(
            item_string(&encoding, 0, "action").as_deref(),
            Some("win.change-encoding")
        );

        assert_eq!(item_string(&menu, 1, "label").as_deref(), Some("Line Endings"));
        let line_endings = section(&menu, 1);
        assert_eq!(line_endings.n_items(), 3);
        let expected = [
            (0, "Unix (LF)", "lf"),
            (1, "Windows (CRLF)", "crlf"),
            (2, "Classic Mac (CR)", "cr"),
        ];
        for (index, label, nick) in expected {
            assert_eq!(
                item_string(&line_endings, index, "label").as_deref(),
                Some(label)
            );
            assert_eq!(
                item_string(&line_endings, index, "action").as_deref(),
                Some("win.line-ending")
            );
            assert_eq!(
                item_string(&line_endings, index, "target").as_deref(),
                Some(nick)
            );
        }
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
}
