use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::gio;
use libadwaita as adw;

use crate::dialogs;
use crate::error::AppError;

#[derive(Clone)]
pub(super) enum CompareSlot {
    SavedVersion,
    File(gio::File),
    Text(String),
}

pub(super) fn choose_file(
    parent: &adw::ApplicationWindow,
    title: &str,
    on_file: Rc<dyn Fn(Option<gio::File>)>,
) {
    let dialog = gtk4::FileDialog::builder()
        .title(title)
        .accept_label(pgettext("file dialog action", "Choose"))
        .modal(true)
        .build();
    apply_text_filters(&dialog);
    let parent_for_open = parent.clone();
    let parent_for_error = parent.clone();
    dialog.open(
        Some(&parent_for_open),
        None::<&gio::Cancellable>,
        move |result| match result {
            Ok(file) => on_file(Some(file)),
            Err(error) if error.matches(gtk4::DialogError::Dismissed) => on_file(None),
            Err(error) => {
                dialogs::present_error(&parent_for_error, &AppError::from(error));
                on_file(None);
            }
        },
    );
}

fn apply_text_filters(dialog: &gtk4::FileDialog) {
    let text_filter = gtk4::FileFilter::new();
    text_filter.set_name(Some(&pgettext("file filter", "Plain Text Files")));
    text_filter.add_mime_type("text/plain");
    text_filter.add_suffix("txt");

    let any_filter = gtk4::FileFilter::new();
    any_filter.set_name(Some(&pgettext("file filter", "All Files")));
    any_filter.add_pattern("*");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&text_filter);
    filters.append(&any_filter);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&text_filter));
}
