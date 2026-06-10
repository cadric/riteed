use libadwaita as adw;

use crate::gtk_tests::spin_until;
use crate::settings::AppSettings;
use crate::window::Window;
use crate::window_preferences::font_description_is_monospace_for_tests;

fn exercise_preference_startup_and_indentation(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    let window = Window::new_with_settings_for_tests(test_app, settings.clone()).ok();
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };

    assert!(settings.write_log_for_tests().is_empty());
    window.ensure_default_tab();
    assert_eq!(window.selected_indentation_for_tests(), Some((true, 4, 4)));
    assert_eq!(
        window.indentation_control_state_for_tests(),
        ((true, 1.0), (true, 1.0))
    );
    let description = gtk4::pango::FontDescription::from_string("FreeMono Regular 13");
    assert!(font_description_is_monospace_for_tests(
        None,
        window.widget(),
        &description,
    ));

    window.set_insert_spaces_for_tests(false);
    window.set_tab_width_for_tests(8);
    window.set_indent_width_for_tests(2);
    spin_until("indentation preferences apply to current tab", || {
        window.selected_indentation_for_tests() == Some((false, 8, 2))
    });
    assert_eq!(
        window.preferences_write_log_for_tests(),
        vec![
            String::from("insert-spaces-instead-of-tabs"),
            String::from("tab-width"),
            String::from("indent-width"),
        ]
    );

    window.request_new();
    spin_until("indentation preferences apply to new tab", || {
        window.tab_count_for_tests() == 2
            && window.selected_indentation_for_tests() == Some((false, 8, 2))
    });
}

fn exercise_indentation_content_behavior(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    let window = Window::new_with_settings_for_tests(test_app, settings).ok();
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };

    window.ensure_default_tab();
    window.set_tab_width_for_tests(10);
    window.set_indent_width_for_tests(4);
    window.set_insert_spaces_for_tests(true);
    spin_until("space indentation preferences apply", || {
        window.selected_indentation_for_tests() == Some((true, 10, 4))
    });
    window.set_selected_text_for_tests("alpha\nbeta");
    window.select_offsets_for_tests(0, 0);
    window.indent_selected_lines_for_tests();
    assert_eq!(window.selected_text_for_tests(), "    alpha\nbeta");
    window.unindent_selected_lines_for_tests();
    assert_eq!(window.selected_text_for_tests(), "alpha\nbeta");

    window.set_insert_spaces_for_tests(false);
    spin_until("tab indentation preference applies", || {
        window.selected_indentation_for_tests() == Some((false, 10, 4))
    });
    window.set_selected_text_for_tests("alpha\nbeta");
    window.select_offsets_for_tests(0, 0);
    window.indent_selected_lines_for_tests();
    assert_eq!(window.selected_text_for_tests(), "    alpha\nbeta");
    window.unindent_selected_lines_for_tests();
    assert_eq!(window.selected_text_for_tests(), "alpha\nbeta");

    window.set_indent_width_for_tests(10);
    spin_until("tab-aligned indentation preference applies", || {
        window.selected_indentation_for_tests() == Some((false, 10, 10))
    });
    window.set_selected_text_for_tests("alpha\nbeta");
    window.select_offsets_for_tests(0, 0);
    window.indent_selected_lines_for_tests();
    assert_eq!(window.selected_text_for_tests(), "\talpha\nbeta");
    assert_eq!(
        window.selected_visual_column_at_offset_for_tests(1),
        Some(10)
    );
    window.unindent_selected_lines_for_tests();
    assert_eq!(window.selected_text_for_tests(), "alpha\nbeta");

    window.set_tab_width_for_tests(2);
    spin_until("tab width preference reapplies visually", || {
        window.selected_indentation_for_tests() == Some((false, 2, 10))
    });
    window.set_selected_text_for_tests("\talpha");
    assert_eq!(
        window.selected_visual_column_at_offset_for_tests(1),
        Some(2)
    );
}

fn exercise_zoom_controller(test_app: &adw::Application) {
    let settings = AppSettings::new_for_tests();
    let window = Window::new_for_tests(test_app, &settings, None).ok();
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };

    window.ensure_default_tab();
    assert_eq!(window.zoom_percent_for_tests(), 100);
    assert_eq!(window.status_zoom_percent_for_tests(), "100%");
    assert!(window.selected_zoom_class_for_tests());
    spin_until("default minimap font resolves", || {
        window.selected_minimap_font_for_tests().is_some()
    });
    // GtkSourceMap mirrors the editor's bottom margin through its own scaled
    // binding, so the minimap margin must stay well below the editor margin.
    spin_until("default scroll past end padding resolves", || {
        window
            .selected_scroll_past_end_padding_for_tests()
            .is_some_and(|(editor, minimap)| editor > 12 && minimap > 0 && minimap < editor)
    });
    let scroll_padding_before = window.selected_scroll_past_end_padding_for_tests();
    assert!(
        scroll_padding_before
            .is_some_and(|(editor, minimap)| { editor > 12 && minimap > 0 && minimap < editor })
    );
    let minimap_before = window
        .selected_minimap_font_for_tests()
        .map(|desc| desc.to_string());

    window.activate_status_zoom_in_for_tests();
    spin_until("zoom in updates zoom percent", || {
        window.zoom_percent_for_tests() == 110 && window.status_zoom_percent_for_tests() == "110%"
    });
    assert_eq!(
        window
            .selected_minimap_font_for_tests()
            .map(|desc| desc.to_string()),
        minimap_before,
        "zoom in keeps minimap font stable",
    );

    window.activate_status_zoom_out_for_tests();
    spin_until("zoom out returns to default", || {
        window.zoom_percent_for_tests() == 100 && window.status_zoom_percent_for_tests() == "100%"
    });
    assert_eq!(
        window
            .selected_minimap_font_for_tests()
            .map(|desc| desc.to_string()),
        minimap_before,
        "zoom out keeps minimap font stable",
    );

    window.activate_status_zoom_in_for_tests();
    window.activate_status_zoom_in_for_tests();
    spin_until("zoom in increases scroll past end padding", || {
        window.zoom_percent_for_tests() == 120
            && window.status_zoom_percent_for_tests() == "120%"
            && window
                .selected_scroll_past_end_padding_for_tests()
                .is_some_and(|(editor, minimap)| {
                    scroll_padding_before.is_some_and(|(before_editor, _before_minimap)| {
                        editor > before_editor && minimap < editor
                    })
                })
    });
    window.activate_status_zoom_reset_for_tests();
    spin_until("zoom reset action works", || {
        window.zoom_percent_for_tests() == 100
            && window.status_zoom_percent_for_tests() == "100%"
            && window.selected_scroll_past_end_padding_for_tests() == scroll_padding_before
    });
}

pub(crate) fn exercise_v5b_editor_controls(test_app: &adw::Application) {
    exercise_preference_startup_and_indentation(test_app);
    exercise_indentation_content_behavior(test_app);
    exercise_zoom_controller(test_app);
}
