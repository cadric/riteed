pub(crate) fn exercise_chrome_palette(test_app: &libadwaita::Application) {
    sourceview5::init();
    crate::source_styles::install_builtin_style_schemes();
    crate::palette_engine::exercise_palette_engine_for_tests();
    crate::app_chrome::exercise_chrome_css_for_tests();

    let manager = sourceview5::StyleSchemeManager::default();
    for scheme_id in [
        "classic",
        "classic-dark",
        "kate",
        "kate-dark",
        "solarized-light",
        "solarized-dark",
    ] {
        let Some(scheme) = manager.scheme(scheme_id) else {
            continue;
        };
        let css =
            crate::app_chrome::chrome_css(&crate::palette_engine::derive_chrome_colors(&scheme));
        assert!(css.contains(":root"));
        assert!(css.contains("--window-bg-color:"));
        assert!(css.contains("--popover-bg-color:"));
        assert!(css.contains("--accent-bg-color:"));
        assert!(css.contains("--headerbar-backdrop-color:"));
        assert!(css.contains("--secondary-sidebar-bg-color:"));
        assert!(css.contains("--headerbar-darker-shade-color:"));
        assert!(!css.contains("--accent-color:"));
        assert!(!css.contains("@define-color"));
        assert!(!css.contains("riteed-window-chrome-"));
        assert!(!css.contains("background-color:"));
        assert!(!css.contains("box-shadow:"));
    }

    let settings = crate::settings::AppSettings::new_for_tests();
    settings.set_theme(crate::settings::ThemePreference::Light);
    settings.apply_theme();
    settings.set_window_palette(crate::settings::WindowPalette::Solarized);
    settings.set_editor_palette(crate::settings::EditorPalette::AdwaitaDark);
    assert!(crate::app_chrome::chrome_css_for_settings(&settings).is_empty());

    settings.set_editor_palette(crate::settings::EditorPalette::SolarizedDark);
    let Some(solarized_light) = manager.scheme("solarized-light") else {
        return;
    };
    assert_eq!(
        crate::app_chrome::chrome_css_for_settings(&settings),
        crate::app_chrome::chrome_css(&crate::palette_engine::derive_chrome_colors(
            &solarized_light
        ))
    );

    let Some(window) = crate::gtk_tests::build_window(test_app) else {
        return;
    };
    let css = window.chrome_css_for_tests();
    assert!(!css.contains("tabbar tab"));
    assert!(!css.contains("riteed-window-chrome-"));
}
