use gtk4::{glib, prelude::*};
use libadwaita as adw;

use crate::error::AppError;

pub struct WindowShell {
    pub window: adw::ApplicationWindow,
    pub toolbar_view: adw::ToolbarView,
    pub title_widget: adw::WindowTitle,
    pub toast_overlay: adw::ToastOverlay,
    pub project_split_view: adw::OverlaySplitView,
    pub workspace_box: gtk4::Box,
    pub new_button: gtk4::Button,
    pub open_button: gtk4::Button,
    pub project_sidebar_button: gtk4::ToggleButton,
    pub save_button: gtk4::Button,
    pub primary_menu_button: gtk4::MenuButton,
    pub preferences_dialog: adw::PreferencesDialog,
    pub theme_row: adw::ComboRow,
    pub word_wrap_row: adw::SwitchRow,
    pub line_numbers_row: adw::SwitchRow,
    pub minimap_row: adw::SwitchRow,
    pub editor_palette_row: adw::ComboRow,
    pub current_line_row: adw::SwitchRow,
    pub editor_font_row: adw::ActionRow,
    pub autosave_row: adw::SwitchRow,
    pub insert_spaces_row: adw::SwitchRow,
    pub tab_width_row: adw::SpinRow,
    pub indent_width_row: adw::SpinRow,
    pub encoding_row: adw::ActionRow,
    pub line_ending_row: adw::ComboRow,
}

impl WindowShell {
    /// # Errors
    ///
    /// Returns an error when the resource-backed window cannot be loaded.
    pub fn new(app: &adw::Application) -> Result<Self, AppError> {
        let builder = gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/window.ui");
        let window: adw::ApplicationWindow = builder_object(&builder, "window")?;
        let toolbar_view: adw::ToolbarView = builder_object(&builder, "toolbar_view")?;
        let title_widget: adw::WindowTitle = builder_object(&builder, "window_title")?;
        let toast_overlay: adw::ToastOverlay = builder_object(&builder, "toast_overlay")?;
        let project_split_view: adw::OverlaySplitView =
            builder_object(&builder, "project_split_view")?;
        let workspace_box: gtk4::Box = builder_object(&builder, "workspace_box")?;
        let new_button: gtk4::Button = builder_object(&builder, "new_button")?;
        let open_button: gtk4::Button = builder_object(&builder, "open_button")?;
        let project_sidebar_button: gtk4::ToggleButton =
            builder_object(&builder, "project_sidebar_button")?;
        let save_button: gtk4::Button = builder_object(&builder, "save_button")?;
        let primary_menu_button: gtk4::MenuButton =
            builder_object(&builder, "primary_menu_button")?;

        let preferences_builder =
            gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/preferences.ui");
        let preferences_dialog: adw::PreferencesDialog =
            builder_object(&preferences_builder, "preferences_dialog")?;
        let theme_row: adw::ComboRow = builder_object(&preferences_builder, "theme_row")?;
        let word_wrap_row: adw::SwitchRow = builder_object(&preferences_builder, "word_wrap_row")?;
        let line_numbers_row: adw::SwitchRow =
            builder_object(&preferences_builder, "line_numbers_row")?;
        let minimap_row: adw::SwitchRow = builder_object(&preferences_builder, "minimap_row")?;
        let editor_palette_row: adw::ComboRow =
            builder_object(&preferences_builder, "editor_palette_row")?;
        let current_line_row: adw::SwitchRow =
            builder_object(&preferences_builder, "current_line_row")?;
        let editor_font_row: adw::ActionRow =
            builder_object(&preferences_builder, "editor_font_row")?;
        let autosave_row: adw::SwitchRow = builder_object(&preferences_builder, "autosave_row")?;
        let insert_spaces_row: adw::SwitchRow =
            builder_object(&preferences_builder, "insert_spaces_row")?;
        let tab_width_row: adw::SpinRow = builder_object(&preferences_builder, "tab_width_row")?;
        let indent_width_row: adw::SpinRow =
            builder_object(&preferences_builder, "indent_width_row")?;
        let encoding_row: adw::ActionRow = builder_object(&preferences_builder, "encoding_row")?;
        let line_ending_row: adw::ComboRow =
            builder_object(&preferences_builder, "line_ending_row")?;

        let shortcuts_builder =
            gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/shortcuts.ui");
        let shortcuts_window: gtk4::ShortcutsWindow =
            builder_object(&shortcuts_builder, "shortcuts_window")?;

        window.set_application(Some(app));
        window.set_help_overlay(Some(&shortcuts_window));

        Ok(Self {
            window,
            toolbar_view,
            title_widget,
            toast_overlay,
            project_split_view,
            workspace_box,
            new_button,
            open_button,
            project_sidebar_button,
            save_button,
            primary_menu_button,
            preferences_dialog,
            theme_row,
            word_wrap_row,
            line_numbers_row,
            minimap_row,
            editor_palette_row,
            current_line_row,
            editor_font_row,
            autosave_row,
            insert_spaces_row,
            tab_width_row,
            indent_width_row,
            encoding_row,
            line_ending_row,
        })
    }
}

fn builder_object<T: IsA<glib::Object>>(builder: &gtk4::Builder, id: &str) -> Result<T, AppError> {
    builder
        .object(id)
        .ok_or_else(|| AppError::Internal(format!("Missing resource object `{id}`.")))
}

#[cfg(test)]
pub(crate) fn builder_object_for_tests() -> Result<gtk4::TextView, AppError> {
    builder_object(&gtk4::Builder::new(), "missing")
}
