use gtk4::{glib, prelude::*};
use libadwaita as adw;

use crate::error::AppError;

pub struct WindowShell {
    pub window: adw::ApplicationWindow,
    pub toolbar_view: adw::ToolbarView,
    pub header_bar: adw::HeaderBar,
    pub title_widget: adw::WindowTitle,
    pub toast_overlay: adw::ToastOverlay,
    pub project_split_view: gtk4::Paned,
    pub workspace_box: gtk4::Box,
    pub new_button: gtk4::Button,
    pub open_button: adw::SplitButton,
    pub project_sidebar_button: gtk4::ToggleButton,
    pub save_button: gtk4::Button,
    pub git_actions_group: gtk4::Box,
    pub git_diff_button: gtk4::Button,
    pub git_stage_button: gtk4::Button,
    pub git_unstage_button: gtk4::Button,
    pub git_discard_button: gtk4::Button,
    pub primary_menu_button: gtk4::MenuButton,
    pub preferences_dialog: adw::PreferencesDialog,
    pub general_preferences_page: adw::PreferencesPage,
    pub appearance_page: adw::PreferencesPage,
    pub style_group: adw::PreferencesGroup,
    pub palette_flow_box: gtk4::FlowBox,
    pub word_wrap_row: adw::SwitchRow,
    pub line_numbers_row: adw::SwitchRow,
    pub highlight_current_line_row: adw::SwitchRow,
    pub minimap_row: adw::SwitchRow,
    pub editor_font_row: adw::ActionRow,
    pub autosave_row: adw::SwitchRow,
    pub language_row: adw::ComboRow,
    pub insert_spaces_row: adw::SwitchRow,
    pub tab_width_row: adw::SpinRow,
    pub indent_width_row: adw::SpinRow,
    pub encoding_row: adw::ActionRow,
    pub line_ending_row: adw::ComboRow,
    pub git_name_row: adw::EntryRow,
    pub git_email_row: adw::EntryRow,
    pub git_identity_apply_button: gtk4::Button,
}

impl WindowShell {
    /// # Errors
    ///
    /// Returns an error when the resource-backed window cannot be loaded.
    pub fn new(app: &adw::Application) -> Result<Self, AppError> {
        let builder = gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/window.ui");
        let window: adw::ApplicationWindow = builder_object(&builder, "window")?;
        let toolbar_view: adw::ToolbarView = builder_object(&builder, "toolbar_view")?;
        let header_bar: adw::HeaderBar = builder_object(&builder, "header_bar")?;
        let title_widget: adw::WindowTitle = builder_object(&builder, "window_title")?;
        let toast_overlay: adw::ToastOverlay = builder_object(&builder, "toast_overlay")?;
        let project_split_view: gtk4::Paned = builder_object(&builder, "project_split_view")?;
        let workspace_box: gtk4::Box = builder_object(&builder, "workspace_box")?;
        let new_button: gtk4::Button = builder_object(&builder, "new_button")?;
        let open_button: adw::SplitButton = builder_object(&builder, "open_button")?;
        let project_sidebar_button: gtk4::ToggleButton =
            builder_object(&builder, "project_sidebar_button")?;
        let save_button: gtk4::Button = builder_object(&builder, "save_button")?;
        let git_actions_group: gtk4::Box = builder_object(&builder, "git_actions_group")?;
        let git_diff_button: gtk4::Button = builder_object(&builder, "git_diff_button")?;
        let git_stage_button: gtk4::Button = builder_object(&builder, "git_stage_button")?;
        let git_unstage_button: gtk4::Button = builder_object(&builder, "git_unstage_button")?;
        let git_discard_button: gtk4::Button = builder_object(&builder, "git_discard_button")?;
        let primary_menu_button: gtk4::MenuButton =
            builder_object(&builder, "primary_menu_button")?;

        let preferences_builder =
            gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/preferences.ui");
        let preferences_dialog: adw::PreferencesDialog =
            builder_object(&preferences_builder, "preferences_dialog")?;
        let general_preferences_page: adw::PreferencesPage =
            builder_object(&preferences_builder, "general_preferences_page")?;
        let appearance_page: adw::PreferencesPage =
            builder_object(&preferences_builder, "appearance_page")?;
        let style_group: adw::PreferencesGroup =
            builder_object(&preferences_builder, "style_group")?;
        let palette_flow_box: gtk4::FlowBox =
            builder_object(&preferences_builder, "palette_flow_box")?;
        let word_wrap_row: adw::SwitchRow = builder_object(&preferences_builder, "word_wrap_row")?;
        let line_numbers_row: adw::SwitchRow =
            builder_object(&preferences_builder, "line_numbers_row")?;
        let highlight_current_line_row: adw::SwitchRow =
            builder_object(&preferences_builder, "highlight_current_line_row")?;
        let minimap_row: adw::SwitchRow = builder_object(&preferences_builder, "minimap_row")?;
        let editor_font_row: adw::ActionRow =
            builder_object(&preferences_builder, "editor_font_row")?;
        let autosave_row: adw::SwitchRow = builder_object(&preferences_builder, "autosave_row")?;
        let language_row: adw::ComboRow = builder_object(&preferences_builder, "language_row")?;
        let insert_spaces_row: adw::SwitchRow =
            builder_object(&preferences_builder, "insert_spaces_row")?;
        let tab_width_row: adw::SpinRow = builder_object(&preferences_builder, "tab_width_row")?;
        let indent_width_row: adw::SpinRow =
            builder_object(&preferences_builder, "indent_width_row")?;
        let encoding_row: adw::ActionRow = builder_object(&preferences_builder, "encoding_row")?;
        let line_ending_row: adw::ComboRow =
            builder_object(&preferences_builder, "line_ending_row")?;
        let git_name_row: adw::EntryRow = builder_object(&preferences_builder, "git_name_row")?;
        let git_email_row: adw::EntryRow = builder_object(&preferences_builder, "git_email_row")?;
        let git_identity_apply_button: gtk4::Button =
            builder_object(&preferences_builder, "git_identity_apply_button")?;

        let shortcuts_builder =
            gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/shortcuts.ui");
        let shortcuts_window: gtk4::ShortcutsWindow =
            builder_object(&shortcuts_builder, "shortcuts_window")?;

        window.set_application(Some(app));
        window.set_help_overlay(Some(&shortcuts_window));

        Ok(Self {
            window,
            toolbar_view,
            header_bar,
            title_widget,
            toast_overlay,
            project_split_view,
            workspace_box,
            new_button,
            open_button,
            project_sidebar_button,
            save_button,
            git_actions_group,
            git_diff_button,
            git_stage_button,
            git_unstage_button,
            git_discard_button,
            primary_menu_button,
            preferences_dialog,
            general_preferences_page,
            appearance_page,
            style_group,
            palette_flow_box,
            word_wrap_row,
            line_numbers_row,
            highlight_current_line_row,
            minimap_row,
            editor_font_row,
            autosave_row,
            language_row,
            insert_spaces_row,
            tab_width_row,
            indent_width_row,
            encoding_row,
            line_ending_row,
            git_name_row,
            git_email_row,
            git_identity_apply_button,
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
