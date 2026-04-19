use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::prelude::*;

use crate::editor_tab::EditorTab;

pub struct EditorStatusBar {
    root: gtk4::Box,
    name_label: gtk4::Label,
    modified_label: gtk4::Label,
    position_label: gtk4::Label,
}

impl Default for EditorStatusBar {
    fn default() -> Self {
        Self::new()
    }
}

impl EditorStatusBar {
    #[must_use]
    pub fn new() -> Self {
        let root = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .margin_bottom(6)
            .margin_end(12)
            .margin_start(12)
            .margin_top(6)
            .build();
        root.add_css_class("toolbar");

        let left = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .hexpand(true)
            .build();

        let name_label = gtk4::Label::builder().xalign(0.0).build();
        let modified_label = gtk4::Label::builder().xalign(0.0).build();
        modified_label.add_css_class("dim-label");

        let position_label = gtk4::Label::builder().xalign(1.0).build();
        position_label.add_css_class("dim-label");
        position_label.set_hexpand(false);

        left.append(&name_label);
        left.append(&modified_label);
        root.append(&left);
        root.append(&position_label);

        name_label.update_property(&[Property::Label(&gettext("Current Document"))]);
        modified_label.update_property(&[Property::Label(&gettext("Modification State"))]);
        position_label.update_property(&[Property::Label(&gettext("Cursor Position"))]);

        let status = Self {
            root,
            name_label,
            modified_label,
            position_label,
        };
        status.update(None);
        status
    }

    #[must_use]
    pub fn widget(&self) -> &gtk4::Box {
        &self.root
    }

    pub fn update(&self, tab: Option<&EditorTab>) {
        let (name, modified, position) = status_strings(tab);
        self.name_label.set_label(&name);
        self.modified_label.set_label(&modified);
        self.position_label.set_label(&position);
    }

    #[cfg(test)]
    pub(crate) fn labels_for_tests(&self) -> (String, String, String) {
        (
            self.name_label.text().to_string(),
            self.modified_label.text().to_string(),
            self.position_label.text().to_string(),
        )
    }
}

#[must_use]
pub fn status_strings(tab: Option<&EditorTab>) -> (String, String, String) {
    let Some(tab) = tab else {
        return (
            pgettext("document title", "Untitled"),
            String::new(),
            format_line_column(1, 1),
        );
    };

    let name = tab.title();
    let modified = if tab.is_dirty() {
        gettext("Modified")
    } else {
        String::new()
    };
    let (line, column) = tab.cursor_position();
    (name, modified, format_line_column(line, column))
}

#[must_use]
pub fn format_line_column(line: u32, column: u32) -> String {
    // Translators: keep both placeholders and adapt the label order for your language.
    let template = pgettext("status position", "Ln %1$d, Col %2$d");
    template
        .replace("%1$d", &line.to_string())
        .replace("%2$d", &column.to_string())
}

#[cfg(test)]
mod tests {
    use super::{format_line_column, status_strings};

    #[test]
    fn line_column_format_replaces_placeholders() {
        let formatted = format_line_column(3, 12);
        assert!(formatted.contains('3'));
        assert!(formatted.contains("12"));
    }

    #[test]
    fn empty_status_defaults_to_untitled() {
        let (name, modified, position) = status_strings(None);
        assert_eq!(name, "Untitled");
        assert!(modified.is_empty());
        assert!(position.contains('1'));
    }
}
