use gtk4::{glib, prelude::*};
use libadwaita as adw;

use crate::error::AppError;

pub struct WindowShell {
    pub window: adw::ApplicationWindow,
    pub toolbar_view: adw::ToolbarView,
    pub title_widget: adw::WindowTitle,
    pub toast_overlay: adw::ToastOverlay,
    pub workspace_box: gtk4::Box,
    pub primary_menu_button: gtk4::MenuButton,
    pub preferences_dialog: adw::PreferencesDialog,
    pub theme_row: adw::ComboRow,
    pub word_wrap_row: adw::SwitchRow,
    pub line_numbers_row: adw::SwitchRow,
    pub minimap_row: adw::SwitchRow,
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
        let workspace_box: gtk4::Box = builder_object(&builder, "workspace_box")?;
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
            workspace_box,
            primary_menu_button,
            preferences_dialog,
            theme_row,
            word_wrap_row,
            line_numbers_row,
            minimap_row,
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
