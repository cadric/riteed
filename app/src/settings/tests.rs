use super::{
    AppSettings, EditorPalette, SourceControlViewMode, ThemePreference, WindowPalette,
    sanitize_editor_width, sanitize_restored_dimension,
};
#[test]
fn theme_preference_roundtrips_enum_values() {
    for theme in ThemePreference::ALL {
        assert_eq!(
            ThemePreference::from_enum_value(theme.enum_value()),
            theme,
            "{}",
            theme.nick()
        );
        assert_eq!(ThemePreference::from_nick(theme.nick()), Some(theme));
    }
    assert_eq!(ThemePreference::from_nick("unknown"), None);
}

#[test]
fn editor_palette_roundtrips_enum_values() {
    for palette in EditorPalette::ALL {
        assert_eq!(
            EditorPalette::from_enum_value(palette.enum_value()),
            palette,
            "{}",
            palette.nick()
        );
    }
    assert_eq!(EditorPalette::ClassicLight.enum_value(), 3);
    assert_eq!(EditorPalette::ClassicLight.nick(), "classic");
    assert_eq!(EditorPalette::ClassicDark.enum_value(), 8);
    assert_eq!(EditorPalette::ClassicDark.nick(), "classic-dark");
    assert_eq!(EditorPalette::FollowSystem.label(), "Auto");
}

#[test]
fn window_palette_roundtrips_enum_values() {
    for palette in WindowPalette::ALL {
        assert_eq!(
            WindowPalette::from_enum_value(palette.enum_value()),
            palette,
            "{}",
            palette.nick()
        );
    }
    assert_eq!(WindowPalette::FollowEditor.enum_value(), 0);
    assert_eq!(WindowPalette::FollowEditor.nick(), "follow-editor");
}

#[test]
fn source_control_view_mode_roundtrips_enum_values() {
    for mode in SourceControlViewMode::ALL {
        assert_eq!(
            SourceControlViewMode::from_enum_value(mode.enum_value()),
            mode,
            "{}",
            mode.nick()
        );
    }
}

#[test]
fn restored_window_dimensions_are_clamped() {
    assert_eq!(sanitize_restored_dimension(900, 840, 360, 4096), 900);
    assert_eq!(sanitize_restored_dimension(0, 840, 360, 4096), 840);
    assert_eq!(sanitize_restored_dimension(-1, 620, 320, 2160), 620);
    assert_eq!(sanitize_restored_dimension(200, 840, 360, 4096), 360);
    assert_eq!(sanitize_restored_dimension(5000, 840, 360, 4096), 4096);
}

#[test]
fn memory_backend_clamps_restored_window_size() {
    let settings = AppSettings::new_for_tests();
    settings.set_window_size(200, 3000);
    assert_eq!(settings.window_size(), (360, 2160));

    settings.set_window_size(0, -1);
    assert_eq!(settings.window_size(), (840, 620));
}

#[test]
fn invalid_editor_widths_fall_back() {
    assert_eq!(sanitize_editor_width(4, 8), 4);
    assert_eq!(sanitize_editor_width(0, 8), 8);
    assert_eq!(sanitize_editor_width(17, 8), 8);
}

#[test]
fn memory_backend_roundtrips_values() {
    let settings = AppSettings::new_for_tests();
    settings.set_theme(ThemePreference::Dark);
    settings.set_editor_palette(EditorPalette::KateDark);
    settings.set_window_palette(WindowPalette::Solarized);
    settings.set_word_wrap(true);
    settings.set_show_line_numbers(true);
    settings.set_show_minimap(true);
    settings.set_highlight_current_line(false);
    settings.set_autosave_enabled(true);
    settings.set_insert_spaces_instead_of_tabs(false);
    settings.set_tab_width(8);
    settings.set_indent_width(2);
    settings.set_editor_font("Fira Code 12");
    settings.set_window_size(900, 700);
    settings.set_recent_files(&[
        String::from("file:///tmp/one.txt"),
        String::from("file:///tmp/two.txt"),
    ]);
    settings.set_session_files(&[String::from("file:///tmp/session.txt")]);
    settings.set_session_selected_file("file:///tmp/session.txt");
    settings.set_git_identity("Ada Lovelace", "ada@example.test");
    settings.set_source_control_view_mode(SourceControlViewMode::List);
    settings.set_project_folder_uri("file:///tmp/project");
    settings.set_project_folder_display_name("Project");
    settings.set_project_sidebar_visible(true);
    settings.set_project_show_hidden(true);

    assert_eq!(settings.theme(), ThemePreference::Dark);
    assert_eq!(settings.editor_palette(), EditorPalette::KateDark);
    assert_eq!(settings.window_palette(), WindowPalette::Solarized);
    assert!(settings.word_wrap());
    assert!(settings.show_line_numbers());
    assert!(settings.show_minimap());
    assert!(!settings.highlight_current_line());
    assert!(settings.autosave_enabled());
    assert!(!settings.insert_spaces_instead_of_tabs());
    assert_eq!(settings.tab_width(), 8);
    assert_eq!(settings.indent_width(), 2);
    assert_eq!(settings.editor_font(), "Fira Code 12");
    assert_eq!(settings.window_size(), (900, 700));
    assert_eq!(
        settings.recent_files(),
        vec![
            String::from("file:///tmp/one.txt"),
            String::from("file:///tmp/two.txt"),
        ]
    );
    assert_eq!(
        settings.session_files(),
        vec![String::from("file:///tmp/session.txt")]
    );
    assert_eq!(settings.session_selected_file(), "file:///tmp/session.txt");
    assert_eq!(
        settings.git_identity(),
        (
            String::from("Ada Lovelace"),
            String::from("ada@example.test")
        )
    );
    assert_eq!(
        settings.source_control_view_mode(),
        SourceControlViewMode::List
    );
    assert_eq!(settings.project_folder_uri(), "file:///tmp/project");
    assert_eq!(settings.project_folder_display_name(), "Project");
    assert!(settings.project_sidebar_visible());
    assert!(settings.project_show_hidden());
}

#[test]
fn memory_backend_records_writes_for_tests() {
    let settings = AppSettings::new_for_tests();
    assert!(settings.write_log_for_tests().is_empty());
    settings.set_tab_width(6);
    settings.set_project_show_hidden(true);
    settings.set_editor_font("Monospace 11");
    settings.set_window_palette(WindowPalette::Classic);
    settings.set_autosave_enabled(true);
    settings.set_git_identity("Ada", "ada@example.test");
    settings.set_source_control_view_mode(SourceControlViewMode::List);
    assert_eq!(
        settings.write_log_for_tests(),
        vec![
            String::from("tab-width"),
            String::from("project-show-hidden"),
            String::from("editor-font"),
            String::from("window-palette"),
            String::from("autosave-enabled"),
            String::from("git-user-name"),
            String::from("git-user-email"),
            String::from("source-control-view-mode"),
        ]
    );
}
