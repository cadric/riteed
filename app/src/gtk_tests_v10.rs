use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

pub(crate) fn exercise_chrome_palette(test_app: &libadwaita::Application) {
    sourceview5::init();
    crate::source_styles::install_builtin_style_schemes();
    crate::palette_engine::exercise_palette_engine_for_tests();
    crate::window_chrome::exercise_chrome_css_for_tests();
    let manager = sourceview5::StyleSchemeManager::default();
    for scheme_id in [
        crate::palette_engine::ADWAITA_LIGHT_SCHEME,
        crate::palette_engine::ADWAITA_DARK_SCHEME,
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
        let css = crate::window_chrome::chrome_css(
            "riteed-test-chrome",
            &crate::palette_engine::derive_chrome_colors(&scheme),
        );
        assert!(crate::window_chrome::css_is_scoped_for_tests(
            &css,
            "riteed-test-chrome"
        ));
        assert!(!css.contains("@define-color"));
    }

    let Some(window) = crate::gtk_tests::build_window(test_app) else {
        return;
    };
    window.set_window_palette_for_tests(crate::settings::WindowPalette::Solarized);
    assert_eq!(
        window.selected_window_palette_for_tests(),
        crate::settings::WindowPalette::Solarized
    );
    let css_class = window.chrome_css_class_for_tests();
    let css = window.chrome_css_for_tests();
    if !css.is_empty() {
        assert!(window.widget().has_css_class(&css_class));
        assert!(css.contains(&css_class));
        assert!(css.contains("--dialog-bg-color"));
        assert!(css.contains("--card-bg-color"));
        assert!(css.contains("--accent-bg-color"));
        assert!(!css.contains("--destructive-bg-color"));
        assert!(!css.contains("tabbar tab"));
        assert!(crate::window_chrome::css_is_scoped_for_tests(
            &css, &css_class
        ));
    }
    let ((tab_bar_classed, tab_view_classed), sidebar_classes) =
        window.chrome_surface_classes_for_tests();
    assert!(tab_bar_classed);
    assert!(tab_view_classed);
    assert!(sidebar_classes.0);
    assert!(sidebar_classes.1);
    assert!(sidebar_classes.2);
    assert!(sidebar_classes.3);

    window.present();
    let dialog = adw::Dialog::builder().build();
    dialog.present(Some(window.widget()));
    crate::gtk_tests::spin_until("dialog inherits chrome root", || {
        dialog_root_has_chrome_scope(&dialog, &css_class)
    });
    let nested_dialog = adw::Dialog::builder().build();
    nested_dialog.present(Some(&dialog));
    crate::gtk_tests::spin_until("nested dialog inherits chrome root", || {
        dialog_root_has_chrome_scope(&nested_dialog, &css_class)
    });
    let _closed = nested_dialog.close();
    let _closed = dialog.close();
}

fn dialog_root_has_chrome_scope(dialog: &adw::Dialog, css_class: &str) -> bool {
    dialog
        .root()
        .and_then(|root| root.downcast::<adw::ApplicationWindow>().ok())
        .is_some_and(|window| window.has_css_class(css_class))
}
