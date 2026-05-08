use gettextrs::pgettext;
use gtk4::prelude::*;

pub(super) struct ReplaceControls {
    pub(super) row: gtk4::Box,
    pub(super) entry: gtk4::Entry,
    pub(super) replace_button: gtk4::Button,
    pub(super) replace_all_button: gtk4::Button,
}

#[must_use]
pub(super) fn build_controls() -> ReplaceControls {
    let entry = gtk4::Entry::builder().hexpand(true).build();
    entry.set_placeholder_text(Some(&pgettext("search field placeholder", "Replace With")));

    let replace_button = gtk4::Button::with_label(&pgettext("search action", "Replace"));
    let replace_all_button = gtk4::Button::with_label(&pgettext("search action", "Replace All"));
    let row = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .visible(false)
        .build();
    row.append(&entry);
    row.append(&replace_button);
    row.append(&replace_all_button);

    ReplaceControls {
        row,
        entry,
        replace_button,
        replace_all_button,
    }
}

pub(super) fn set_replace_mode_visible(row: &gtk4::Box, visible: bool) {
    row.set_visible(visible);
}
