use gettextrs::pgettext;
use gtk4::prelude::*;

use super::history::SourceControlHistory;

pub(super) const HISTORY_SPLIT_DEFAULT_POSITION: i32 = 360;

pub(super) fn build_commit_controls() -> (gtk4::Revealer, gtk4::Entry, gtk4::Button) {
    let commit_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let commit_entry = gtk4::Entry::builder()
        .placeholder_text(pgettext("git commit", "Commit Message"))
        .build();
    commit_box.append(&commit_entry);

    let commit_button = gtk4::Button::with_label(&pgettext("git commit", "Commit"));
    commit_button.add_css_class("suggested-action");
    commit_button.set_sensitive(false);
    commit_box.append(&commit_button);

    let commit_revealer = gtk4::Revealer::builder()
        .transition_type(gtk4::RevealerTransitionType::SlideDown)
        .child(&commit_box)
        .build();
    (commit_revealer, commit_entry, commit_button)
}

pub(super) fn build_history_split(
    changes_pane: &gtk4::Box,
    history: &SourceControlHistory,
) -> gtk4::Paned {
    let history_split = gtk4::Paned::new(gtk4::Orientation::Vertical);
    history_split.set_vexpand(true);
    history_split.set_wide_handle(true);
    history_split.set_resize_start_child(true);
    history_split.set_resize_end_child(true);
    history_split.set_shrink_start_child(false);
    history_split.set_shrink_end_child(false);
    history_split.set_start_child(Some(changes_pane));
    history_split.set_end_child(Some(&history.widget()));
    history_split.set_position(HISTORY_SPLIT_DEFAULT_POSITION);
    history_split
}
