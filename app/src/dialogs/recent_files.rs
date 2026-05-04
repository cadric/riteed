use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialog_shell::build_dialog_shell;
use crate::document;
use crate::settings::AppSettings;

pub fn show_recent_files_dialog(
    parent: &adw::ApplicationWindow,
    settings: &AppSettings,
    on_open_uri: &Rc<dyn Fn(String)>,
) {
    let _dialog = present_recent_files_dialog(parent, settings, on_open_uri);
}

#[cfg(test)]
pub(crate) fn show_recent_files_dialog_for_tests(
    parent: &adw::ApplicationWindow,
    settings: &AppSettings,
) -> adw::Dialog {
    let on_open_uri: Rc<dyn Fn(String)> = Rc::new(|_uri| {});
    present_recent_files_dialog(parent, settings, &on_open_uri)
}

fn present_recent_files_dialog(
    parent: &adw::ApplicationWindow,
    settings: &AppSettings,
    on_open_uri: &Rc<dyn Fn(String)>,
) -> adw::Dialog {
    let shell = build_dialog_shell(
        &pgettext("recent files dialog title", "Recent Files"),
        620,
        Some(420),
        false,
    );
    let dialog = shell.dialog;

    let list_box = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .build();
    list_box.add_css_class("boxed-list");

    let list_holder = gtk4::ScrolledWindow::builder()
        .child(&list_box)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .build();
    list_holder.set_vexpand(true);

    let empty_page = adw::StatusPage::builder()
        .icon_name("document-open-symbolic")
        .title(gettext("No Recent Files"))
        .description(gettext("Files you open will appear here."))
        .build();

    let stack = gtk4::Stack::builder().build();
    stack.set_vexpand(true);
    stack.add_named(&list_holder, Some("list"));
    stack.add_named(&empty_page, Some("empty"));

    let content = shell.content;
    content.set_vexpand(true);
    content.append(&stack);

    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::End)
        .valign(gtk4::Align::End)
        .spacing(12)
        .build();
    let clear_all_button = gtk4::Button::with_label(&pgettext("dialog button", "Clear All"));
    button_box.append(&clear_all_button);
    content.append(&button_box);

    let state = Rc::new(RecentFilesDialogState {
        dialog: dialog.downgrade(),
        settings: settings.clone(),
        list_box,
        stack,
        on_open_uri: Rc::clone(on_open_uri),
        #[cfg(test)]
        _leak_canary: crate::dialogs::lifecycle::DialogLeakCanary::new(
            crate::dialogs::lifecycle::DialogLeakKind::RecentFiles,
        ),
    });

    let weak = Rc::downgrade(&state);
    clear_all_button.connect_clicked(move |_| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        state.confirm_clear_all();
    });

    rebuild_recent_files(&state);

    let state_for_closed = Rc::clone(&state);
    dialog.connect_closed(move |_| {
        let _state = &state_for_closed;
    });

    dialog.present(Some(parent));
    dialog
}

struct RecentFilesDialogState {
    dialog: gtk4::glib::WeakRef<adw::Dialog>,
    settings: AppSettings,
    list_box: gtk4::ListBox,
    stack: gtk4::Stack,
    on_open_uri: Rc<dyn Fn(String)>,
    #[cfg(test)]
    _leak_canary: crate::dialogs::lifecycle::DialogLeakCanary,
}

impl RecentFilesDialogState {
    fn dialog(&self) -> Option<adw::Dialog> {
        self.dialog.upgrade()
    }

    fn close_dialog(&self) {
        if let Some(dialog) = self.dialog() {
            let _closed = dialog.close();
        }
    }

    fn confirm_clear_all(self: &Rc<Self>) {
        if self.settings.recent_files().is_empty() {
            return;
        }
        let Some(dialog) = self.dialog() else {
            return;
        };
        let alert = adw::AlertDialog::builder()
            .heading(gettext("Clear Recent Files?"))
            .body(gettext("This removes all recent file entries."))
            .build();
        alert.add_responses(&[
            ("cancel", &pgettext("alert response", "Cancel")),
            ("clear", &pgettext("alert response", "Clear")),
        ]);
        alert.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");
        let weak = Rc::downgrade(self);
        alert.choose(Some(&dialog), None::<&gio::Cancellable>, move |response| {
            if response != "clear" {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            state.settings.set_recent_files(&[]);
            rebuild_recent_files(&state);
        });
    }
}

fn rebuild_recent_files(state: &Rc<RecentFilesDialogState>) {
    while let Some(child) = state.list_box.first_child() {
        state.list_box.remove(&child);
    }

    let recent_files = state.settings.recent_files();
    if recent_files.is_empty() {
        state.stack.set_visible_child_name("empty");
        return;
    }

    for uri in recent_files {
        let (title, subtitle) = recent_labels(&uri);
        let row = adw::ActionRow::builder()
            .title(&title)
            .subtitle(&subtitle)
            .activatable(true)
            .build();
        let uri_for_row = uri.clone();
        let weak = Rc::downgrade(state);
        row.connect_activated(move |_| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            (state.on_open_uri)(uri_for_row.clone());
            state.close_dialog();
        });
        state.list_box.append(&row);
    }
    state.stack.set_visible_child_name("list");
}

fn recent_labels(uri: &str) -> (String, String) {
    let file = gio::File::for_uri(uri);
    let Some(path) = file.path() else {
        return (String::from(uri), String::new());
    };
    let display_path = document::portal_host_display_path(&path).unwrap_or(path);
    let title = display_path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .map_or_else(
            || document::display_path(&display_path),
            ToString::to_string,
        );
    let subtitle = display_path
        .parent()
        .map_or_else(String::new, document::display_path);
    (title, subtitle)
}

#[cfg(test)]
mod tests {
    use gtk4::prelude::FileExt;

    use super::recent_labels;

    #[test]
    fn recent_labels_use_filename_and_parent_for_local_files() {
        let file = gtk4::gio::File::for_path("/tmp/riteed-recent-label.txt");
        let (title, subtitle) = recent_labels(file.uri().as_str());
        assert_eq!(title, "riteed-recent-label.txt");
        assert!(subtitle.ends_with("/tmp"));
    }

    #[test]
    fn recent_labels_keep_non_file_uris_visible() {
        let uri = "trash:///example.txt";
        let (title, subtitle) = recent_labels(uri);
        assert_eq!(title, uri);
        assert_eq!(subtitle, "");
    }
}
