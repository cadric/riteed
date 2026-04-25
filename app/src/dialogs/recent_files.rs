use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::document;
use crate::settings::AppSettings;

pub fn show_recent_files_dialog(
    parent: &adw::ApplicationWindow,
    settings: &AppSettings,
    on_open_uri: &Rc<dyn Fn(String)>,
) {
    let dialog = adw::Dialog::builder()
        .title(pgettext("recent files dialog title", "Recent Files"))
        .content_width(620)
        .content_height(520)
        .follows_content_size(false)
        .can_close(true)
        .build();

    let list_box = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::None)
        .build();
    list_box.add_css_class("boxed-list");

    let list_holder = gtk4::ScrolledWindow::builder()
        .child(&list_box)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .min_content_height(360)
        .build();

    let empty_page = adw::StatusPage::builder()
        .icon_name("document-open-symbolic")
        .title(gettext("No Recent Files"))
        .description(gettext("Files you open will appear here."))
        .build();

    let stack = gtk4::Stack::builder().build();
    stack.add_named(&list_holder, Some("list"));
    stack.add_named(&empty_page, Some("empty"));

    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(18)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&stack);

    let button_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .halign(gtk4::Align::End)
        .spacing(12)
        .build();
    let clear_all_button = gtk4::Button::with_label(&pgettext("dialog button", "Clear All"));
    let close_button = gtk4::Button::with_label(&pgettext("dialog button", "Close"));
    button_box.append(&clear_all_button);
    button_box.append(&close_button);
    content.append(&button_box);

    dialog.set_child(Some(&content));

    {
        let dialog = dialog.clone();
        close_button.connect_clicked(move |_| {
            let _closed = dialog.close();
        });
    }

    {
        let dialog = dialog.clone();
        let settings = settings.clone();
        let list_box = list_box.clone();
        let stack = stack.clone();
        clear_all_button.connect_clicked(move |_| {
            if settings.recent_files().is_empty() {
                return;
            }
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
            let settings_for_choice = settings.clone();
            let list_box_for_choice = list_box.clone();
            let stack_for_choice = stack.clone();
            let dialog_for_parent = dialog.clone();
            let dialog_for_choice = dialog.clone();
            alert.choose(
                Some(&dialog_for_parent),
                None::<&gio::Cancellable>,
                move |response| {
                    if response != "clear" {
                        return;
                    }
                    settings_for_choice.set_recent_files(&[]);
                    rebuild_recent_files(
                        &dialog_for_choice,
                        &settings_for_choice,
                        &list_box_for_choice,
                        &stack_for_choice,
                        None,
                    );
                },
            );
        });
    }

    rebuild_recent_files(&dialog, settings, &list_box, &stack, Some(on_open_uri));

    dialog.present(Some(parent));
}

fn rebuild_recent_files(
    dialog: &adw::Dialog,
    settings: &AppSettings,
    list_box: &gtk4::ListBox,
    stack: &gtk4::Stack,
    on_open_uri: Option<&Rc<dyn Fn(String)>>,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let recent_files = settings.recent_files();
    if recent_files.is_empty() {
        stack.set_visible_child_name("empty");
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
        if let Some(on_open_uri) = on_open_uri {
            let on_open_uri = Rc::clone(on_open_uri);
            let dialog = dialog.clone();
            row.connect_activated(move |_| {
                on_open_uri(uri_for_row.clone());
                let _closed = dialog.close();
            });
        }
        list_box.append(&row);
    }
    stack.set_visible_child_name("list");
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
