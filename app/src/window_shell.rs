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
    pub large_file_full_feature_limit_row: adw::SpinRow,
    pub large_file_editor_limit_row: adw::SpinRow,
    pub large_file_strong_warning_limit_row: adw::SpinRow,
    pub large_file_viewer_only_limit_row: adw::SpinRow,
    pub large_file_always_edit_row: adw::SwitchRow,
    pub language_row: adw::ComboRow,
    pub insert_spaces_row: adw::SwitchRow,
    pub tab_width_row: adw::SpinRow,
    pub indent_width_row: adw::SpinRow,
    pub git_name_row: adw::EntryRow,
    pub git_email_row: adw::EntryRow,
    pub git_identity_apply_button: gtk4::Button,
}

struct PreferenceWidgets {
    preferences_dialog: adw::PreferencesDialog,
    general_preferences_page: adw::PreferencesPage,
    appearance_page: adw::PreferencesPage,
    style_group: adw::PreferencesGroup,
    palette_flow_box: gtk4::FlowBox,
    word_wrap_row: adw::SwitchRow,
    line_numbers_row: adw::SwitchRow,
    highlight_current_line_row: adw::SwitchRow,
    minimap_row: adw::SwitchRow,
    editor_font_row: adw::ActionRow,
    autosave_row: adw::SwitchRow,
    large_file_full_feature_limit_row: adw::SpinRow,
    large_file_editor_limit_row: adw::SpinRow,
    large_file_strong_warning_limit_row: adw::SpinRow,
    large_file_viewer_only_limit_row: adw::SpinRow,
    large_file_always_edit_row: adw::SwitchRow,
    language_row: adw::ComboRow,
    insert_spaces_row: adw::SwitchRow,
    tab_width_row: adw::SpinRow,
    indent_width_row: adw::SpinRow,
    git_name_row: adw::EntryRow,
    git_email_row: adw::EntryRow,
    git_identity_apply_button: gtk4::Button,
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

        let preferences = load_preference_widgets()?;

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
            preferences_dialog: preferences.preferences_dialog,
            general_preferences_page: preferences.general_preferences_page,
            appearance_page: preferences.appearance_page,
            style_group: preferences.style_group,
            palette_flow_box: preferences.palette_flow_box,
            word_wrap_row: preferences.word_wrap_row,
            line_numbers_row: preferences.line_numbers_row,
            highlight_current_line_row: preferences.highlight_current_line_row,
            minimap_row: preferences.minimap_row,
            editor_font_row: preferences.editor_font_row,
            autosave_row: preferences.autosave_row,
            large_file_full_feature_limit_row: preferences.large_file_full_feature_limit_row,
            large_file_editor_limit_row: preferences.large_file_editor_limit_row,
            large_file_strong_warning_limit_row: preferences.large_file_strong_warning_limit_row,
            large_file_viewer_only_limit_row: preferences.large_file_viewer_only_limit_row,
            large_file_always_edit_row: preferences.large_file_always_edit_row,
            language_row: preferences.language_row,
            insert_spaces_row: preferences.insert_spaces_row,
            tab_width_row: preferences.tab_width_row,
            indent_width_row: preferences.indent_width_row,
            git_name_row: preferences.git_name_row,
            git_email_row: preferences.git_email_row,
            git_identity_apply_button: preferences.git_identity_apply_button,
        })
    }
}

fn load_preference_widgets() -> Result<PreferenceWidgets, AppError> {
    let builder = gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/preferences.ui");
    Ok(PreferenceWidgets {
        preferences_dialog: builder_object(&builder, "preferences_dialog")?,
        general_preferences_page: builder_object(&builder, "general_preferences_page")?,
        appearance_page: builder_object(&builder, "appearance_page")?,
        style_group: builder_object(&builder, "style_group")?,
        palette_flow_box: builder_object(&builder, "palette_flow_box")?,
        word_wrap_row: builder_object(&builder, "word_wrap_row")?,
        line_numbers_row: builder_object(&builder, "line_numbers_row")?,
        highlight_current_line_row: builder_object(&builder, "highlight_current_line_row")?,
        minimap_row: builder_object(&builder, "minimap_row")?,
        editor_font_row: builder_object(&builder, "editor_font_row")?,
        autosave_row: builder_object(&builder, "autosave_row")?,
        large_file_full_feature_limit_row: builder_object(
            &builder,
            "large_file_full_feature_limit_row",
        )?,
        large_file_editor_limit_row: builder_object(&builder, "large_file_editor_limit_row")?,
        large_file_strong_warning_limit_row: builder_object(
            &builder,
            "large_file_strong_warning_limit_row",
        )?,
        large_file_viewer_only_limit_row: builder_object(
            &builder,
            "large_file_viewer_only_limit_row",
        )?,
        large_file_always_edit_row: builder_object(&builder, "large_file_always_edit_row")?,
        language_row: builder_object(&builder, "language_row")?,
        insert_spaces_row: builder_object(&builder, "insert_spaces_row")?,
        tab_width_row: builder_object(&builder, "tab_width_row")?,
        indent_width_row: builder_object(&builder, "indent_width_row")?,
        git_name_row: builder_object(&builder, "git_name_row")?,
        git_email_row: builder_object(&builder, "git_email_row")?,
        git_identity_apply_button: builder_object(&builder, "git_identity_apply_button")?,
    })
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
