use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::prelude::*;
#[cfg(test)]
use std::cell::Cell;

use crate::editor_tab::EditorTab;

struct StatusControls {
    format_button: gtk4::MenuButton,
    zoom_box: gtk4::Box,
    zoom_percent_label: gtk4::Label,
    zoom_out_button: gtk4::Button,
    zoom_in_button: gtk4::Button,
}

pub struct EditorStatusBar {
    root: gtk4::Box,
    name_label: gtk4::Label,
    location_label: gtk4::Label,
    modified_label: gtk4::Label,
    position_label: gtk4::Label,
    format_button: gtk4::MenuButton,
    #[cfg(test)]
    format_button_label_updates: Cell<usize>,
    zoom_box: gtk4::Box,
    zoom_percent_label: gtk4::Label,
    zoom_out_button: gtk4::Button,
    zoom_in_button: gtk4::Button,
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
            .margin_end(12)
            .margin_start(12)
            .build();
        root.add_css_class("toolbar");
        root.add_css_class("riteed-status-bar");

        let left = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .hexpand(true)
            .build();

        let right = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk4::Align::End)
            .build();

        let name_label = gtk4::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        let location_label = gtk4::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .hexpand(true)
            .build();
        location_label.add_css_class("dim-label");
        let modified_label = gtk4::Label::builder().xalign(0.0).build();
        modified_label.add_css_class("dim-label");

        let position_label = gtk4::Label::builder().xalign(1.0).build();
        position_label.add_css_class("dim-label");

        let controls = build_status_controls();

        left.append(&location_label);
        left.append(&modified_label);
        right.append(&status_separator());
        right.append(&controls.format_button);
        right.append(&status_separator());
        right.append(&controls.zoom_box);
        right.append(&status_separator());
        right.append(&position_label);
        root.append(&left);
        root.append(&right);

        controls
            .format_button
            .update_property(&[Property::Label(&gettext("Document Format"))]);
        location_label.update_property(&[Property::Label(&gettext("Current Document Location"))]);
        modified_label.update_property(&[Property::Label(&gettext("Modification State"))]);
        position_label.update_property(&[Property::Label(&gettext("Cursor Position"))]);

        let status = Self {
            root,
            name_label,
            location_label,
            modified_label,
            position_label,
            format_button: controls.format_button,
            #[cfg(test)]
            format_button_label_updates: Cell::new(0),
            zoom_box: controls.zoom_box,
            zoom_percent_label: controls.zoom_percent_label,
            zoom_out_button: controls.zoom_out_button,
            zoom_in_button: controls.zoom_in_button,
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
        let location = status_location(tab);
        set_label_if_changed(&self.name_label, &name);
        set_label_if_changed(&self.location_label, &location);
        let tooltip = if location.is_empty() {
            None
        } else {
            Some(location.as_str())
        };
        if self.location_label.tooltip_text().as_deref() != tooltip {
            self.location_label.set_tooltip_text(tooltip);
        }
        set_label_if_changed(&self.modified_label, &modified);
        set_label_if_changed(&self.position_label, &position);

        if let Some(tab) = tab {
            self.set_format_button_label_if_changed(&tab.current_format_summary());
            self.format_button.set_sensitive(true);
            self.zoom_box.set_sensitive(true);
            self.zoom_out_button.set_sensitive(true);
            self.zoom_in_button.set_sensitive(true);
        } else {
            self.set_format_button_label_if_changed(&pgettext("status format", "Format"));
            self.format_button.set_sensitive(false);
            self.zoom_box.set_sensitive(false);
            self.zoom_out_button.set_sensitive(false);
            self.zoom_in_button.set_sensitive(false);
        }
    }

    pub fn set_zoom_percent(&self, percent: i32) {
        self.zoom_percent_label.set_label(&format!("{percent}%"));
    }

    fn set_format_button_label_if_changed(&self, label: &str) {
        if set_menu_button_label_if_changed(&self.format_button, label) {
            #[cfg(test)]
            self.record_format_button_label_update_for_tests();
        }
    }

    #[cfg(test)]
    pub(crate) fn labels_for_tests(&self) -> (String, String, String) {
        (
            self.name_label.text().to_string(),
            self.modified_label.text().to_string(),
            self.position_label.text().to_string(),
        )
    }

    #[cfg(test)]
    fn record_format_button_label_update_for_tests(&self) {
        self.format_button_label_updates
            .set(self.format_button_label_updates.get().saturating_add(1));
    }

    #[cfg(test)]
    pub(crate) fn format_button_label_updates_for_tests(&self) -> usize {
        self.format_button_label_updates.get()
    }

    #[cfg(test)]
    pub(crate) fn format_summary_for_tests(&self) -> String {
        self.format_button
            .label()
            .map(|label| label.to_string())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub(crate) fn zoom_percent_for_tests(&self) -> String {
        self.zoom_percent_label.text().to_string()
    }

    #[cfg(test)]
    pub(crate) fn activate_zoom_in_for_tests(&self) {
        self.zoom_in_button.emit_clicked();
    }

    #[cfg(test)]
    pub(crate) fn activate_zoom_out_for_tests(&self) {
        self.zoom_out_button.emit_clicked();
    }

    #[cfg(test)]
    pub(crate) fn activate_zoom_reset_for_tests(&self) {
        let _activated = self.root.activate_action("win.zoom-reset", None);
    }
}

fn build_status_controls() -> StatusControls {
    let format_button = gtk4::MenuButton::builder()
        .menu_model(&crate::window_format_menu::build_menu())
        .tooltip_text(gettext("Document Format"))
        .valign(gtk4::Align::Center)
        .direction(gtk4::ArrowType::Up)
        .build();
    format_button.add_css_class("flat");

    let zoom_box = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(6)
        .valign(gtk4::Align::Center)
        .build();
    let zoom_label = gtk4::Label::builder()
        .label(pgettext("status zoom", "Zoom"))
        .build();
    zoom_label.add_css_class("dim-label");

    let zoom_percent_label = gtk4::Label::builder().label("100%").width_chars(4).build();
    let zoom_out_button = zoom_button(
        "zoom-out-symbolic",
        &pgettext("zoom action", "Zoom Out"),
        "win.zoom-out",
    );
    let zoom_in_button = zoom_button(
        "zoom-in-symbolic",
        &pgettext("zoom action", "Zoom In"),
        "win.zoom-in",
    );

    zoom_box.append(&zoom_label);
    zoom_box.append(&zoom_out_button);
    zoom_box.append(&zoom_percent_label);
    zoom_box.append(&zoom_in_button);

    StatusControls {
        format_button,
        zoom_box,
        zoom_percent_label,
        zoom_out_button,
        zoom_in_button,
    }
}

fn status_separator() -> gtk4::Separator {
    let separator = gtk4::Separator::new(gtk4::Orientation::Vertical);
    separator.set_valign(gtk4::Align::Center);
    separator
}

fn zoom_button(icon_name: &str, label: &str, action_name: &str) -> gtk4::Button {
    let button = gtk4::Button::builder()
        .icon_name(icon_name)
        .tooltip_text(label)
        .build();
    button.add_css_class("flat");
    button.set_action_name(Some(action_name));
    button.update_property(&[Property::Label(label)]);
    button
}

fn set_label_if_changed(label: &gtk4::Label, text: &str) {
    if label.label().as_str() != text {
        label.set_label(text);
    }
}

fn set_menu_button_label_if_changed(button: &gtk4::MenuButton, label: &str) -> bool {
    if button.label().as_deref() == Some(label) {
        return false;
    }
    button.set_label(label);
    true
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

fn status_location(tab: Option<&EditorTab>) -> String {
    tab.and_then(EditorTab::path_display).unwrap_or_default()
}

#[must_use]
pub fn format_line_column(line: u32, column: u32) -> String {
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
