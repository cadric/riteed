use gettextrs::{gettext, pgettext};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

pub(crate) fn show_help(parent: &impl IsA<gtk4::Widget>) {
    let dialog = adw::PreferencesDialog::builder()
        .title(pgettext("help dialog", "Help"))
        .content_width(580)
        .content_height(640)
        .follows_content_size(false)
        .build();

    dialog.add(&help_overview_page());
    dialog.add(&help_technical_page());
    dialog.present(Some(parent));
}

fn help_overview_page() -> adw::PreferencesPage {
    let overview = adw::PreferencesPage::builder()
        .title(pgettext("help page", "Overview"))
        .icon_name("dialog-information-symbolic")
        .build();

    let getting_started = adw::PreferencesGroup::builder()
        .title(pgettext("help section", "Getting Started"))
        .description(gettext(
            "Riteed is a lightweight GNOME editor for text, code, config, and markdown files, with tabs, find, syntax highlighting, session restore, and encoding-aware open and save behavior.",
        ))
        .build();
    getting_started.add(&help_row(
        &pgettext("help row", "Tabs and Files"),
        &gettext(
            "Press Ctrl+T to create another tab, and use Open Files (Ctrl+O) to load local files into separate tabs with their saved encoding and line endings.",
        ),
    ));
    getting_started.add(&help_row(
        &pgettext("help row", "Saving Work"),
        &gettext(
            "Riteed tracks unsaved changes per tab, restores saved files from your previous session, and warns before external file changes replace your work.",
        ),
    ));

    let editing = adw::PreferencesGroup::builder()
        .title(pgettext("help section", "Everyday Editing"))
        .build();
    editing.add(&help_row(
        &pgettext("help row", "Find and Replace"),
        &gettext(
            "Press Ctrl+F to find text in the current document, Ctrl+H to show replace, and Ctrl+G or Shift+Ctrl+G to move between matches.",
        ),
    ));
    editing.add(&help_row(
        &pgettext("help row", "Find in Files"),
        &gettext(
            "Press Ctrl+Shift+F to search the open folder. Riteed streams results from the current workspace and skips hidden or generated folders that are not useful for text search.",
        ),
    ));
    editing.add(&help_row(
        &pgettext("help row", "Editor Tools"),
        &gettext(
            "Use the main menu to print, view document statistics, or switch System, Light, and Dark appearance. Open Preferences to adjust window and editor palettes, current-line highlight, line numbers, and the minimap.",
        ),
    ));
    editing.add(&help_row(
        &pgettext("help row", "More Shortcuts"),
        &gettext(
            "Open Keyboard Shortcuts from the main menu to review the available file, tab, find, and app commands.",
        ),
    ));

    let source_control = adw::PreferencesGroup::builder()
        .title(pgettext("help section", "Source Control"))
        .build();
    source_control.add(&help_row(
        &pgettext("help row", "Changed Files"),
        &gettext(
            "Open a Git folder to review local changes in the Source Control sidebar. Select a changed file to compare it, or switch between tree and list views when a flat list is easier to scan.",
        ),
    ));
    source_control.add(&help_row(
        &pgettext("help row", "Stage and Commit"),
        &gettext(
            "Use the row actions to stage or unstage files, then write a commit message and commit local staged changes with the Git identity saved in Preferences.",
        ),
    ));
    source_control.add(&help_row(
        &pgettext("help row", "Discard Changes"),
        &gettext(
            "Tracked unstaged files can be discarded after confirmation. Riteed keeps unsafe discard cases disabled when it cannot restore the file predictably.",
        ),
    ));
    source_control.add(&help_row(
        &pgettext("help row", "Local Git Only"),
        &gettext(
            "Source Control supports local review, diff, stage, unstage, discard, and simple commits. It does not manage remotes, branches, merges, rebases, conflicts, credentials, or build workflows.",
        ),
    ));

    overview.add(&getting_started);
    overview.add(&editing);
    overview.add(&source_control);
    overview
}

fn help_technical_page() -> adw::PreferencesPage {
    let technical = adw::PreferencesPage::builder()
        .title(pgettext("help page", "Technical Notes"))
        .icon_name("applications-engineering-symbolic")
        .build();

    let source_control_notes = adw::PreferencesGroup::builder()
        .title(pgettext("help section", "Source Control"))
        .build();
    source_control_notes.add(&help_row(
        &pgettext("help row", "Safe Discard Limits"),
        &gettext(
            "Discard stays disabled when open tabs, Git filters, working-tree encodings, or line-ending conversion make an exact restore unsafe.",
        ),
    ));
    source_control_notes.add(&help_row(
        &pgettext("help row", "Live Refresh"),
        &gettext(
            "Riteed refreshes Git status after saves and local Git metadata changes. Document-portal folders use periodic polling when native file monitoring is unavailable.",
        ),
    ));
    source_control_notes.add(&help_row(
        &pgettext("help row", "Find in Files Limits"),
        &gettext(
            "Find in Files searches UTF-8 text without an index. It skips generated folders such as .git, target, build, node_modules, vendor, dist, .flatpak-builder, __pycache__, and .venv.",
        ),
    ));

    technical.add(&source_control_notes);
    technical
}

fn help_row(title: &str, subtitle: &str) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();
    row.set_activatable(false);
    row
}
