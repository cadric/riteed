use libadwaita as adw;

use crate::palette_engine::{
    self, ADWAITA_DARK_SCHEME, ADWAITA_LIGHT_SCHEME, ChromeColors, rgba_to_css,
};
use crate::settings::AppSettings;

struct CssColors {
    window_background: String,
    window_text: String,
    view_background: String,
    view_text: String,
    headerbar_background: String,
    headerbar_darker_shade: String,
    sidebar_background: String,
    popover_background: String,
    popover_text: String,
    dialog_background: String,
    dialog_text: String,
    card_background: String,
    card_text: String,
    card_shade: String,
    accent_background: String,
    accent_text: String,
    border: String,
    shade: String,
}

impl CssColors {
    fn new(colors: &ChromeColors) -> Self {
        Self {
            window_background: rgba_to_css(&colors.window_bg),
            window_text: rgba_to_css(&colors.window_fg),
            view_background: rgba_to_css(&colors.view_bg),
            view_text: rgba_to_css(&colors.view_fg),
            headerbar_background: rgba_to_css(&colors.headerbar_bg),
            headerbar_darker_shade: rgba_to_css(&colors.headerbar_darker_shade),
            sidebar_background: rgba_to_css(&colors.sidebar_bg),
            popover_background: rgba_to_css(&colors.popover_bg),
            popover_text: rgba_to_css(&colors.popover_fg),
            dialog_background: rgba_to_css(&colors.dialog_bg),
            dialog_text: rgba_to_css(&colors.dialog_fg),
            card_background: rgba_to_css(&colors.card_bg),
            card_text: rgba_to_css(&colors.card_fg),
            card_shade: rgba_to_css(&colors.card_shade),
            accent_background: rgba_to_css(&colors.accent_bg),
            accent_text: rgba_to_css(&colors.accent_fg),
            border: rgba_to_css(&colors.border),
            shade: rgba_to_css(&colors.shade),
        }
    }
}

pub(crate) fn chrome_css_for_settings(settings: &AppSettings) -> String {
    let style_manager = adw::StyleManager::default();
    if style_manager.is_high_contrast() {
        return String::new();
    }
    let app_dark = style_manager.is_dark();
    let Some(scheme_id) = palette_engine::window_scheme_id(
        settings.window_palette(),
        settings.editor_palette(),
        app_dark,
    ) else {
        return String::new();
    };
    if scheme_id == ADWAITA_LIGHT_SCHEME || scheme_id == ADWAITA_DARK_SCHEME {
        return String::new();
    }
    sourceview5::StyleSchemeManager::default()
        .scheme(&scheme_id)
        .map_or_else(String::new, |scheme| {
            chrome_css(&palette_engine::derive_chrome_colors(&scheme))
        })
}

pub(crate) fn chrome_css(colors: &ChromeColors) -> String {
    let colors = CssColors::new(colors);
    format!(
        ":root {{
  --window-bg-color: {window_background};
  --window-fg-color: {window_text};

  --view-bg-color: {view_background};
  --view-fg-color: {view_text};
  --thumbnail-bg-color: {view_background};
  --thumbnail-fg-color: {view_text};

  --headerbar-bg-color: {headerbar_background};
  --headerbar-fg-color: {window_text};
  --headerbar-backdrop-color: {headerbar_background};
  --headerbar-border-color: {border};
  --headerbar-shade-color: {shade};
  --headerbar-darker-shade-color: {headerbar_darker_shade};

  --sidebar-bg-color: {sidebar_background};
  --sidebar-fg-color: {window_text};
  --sidebar-backdrop-color: {sidebar_background};
  --sidebar-border-color: {shade};
  --sidebar-shade-color: {shade};

  --secondary-sidebar-bg-color: {sidebar_background};
  --secondary-sidebar-fg-color: {window_text};
  --secondary-sidebar-backdrop-color: {sidebar_background};
  --secondary-sidebar-border-color: {shade};
  --secondary-sidebar-shade-color: {shade};

  --card-bg-color: {card_background};
  --card-fg-color: {card_text};
  --card-shade-color: {card_shade};

  --dialog-bg-color: {dialog_background};
  --dialog-fg-color: {dialog_text};

  --popover-bg-color: {popover_background};
  --popover-fg-color: {popover_text};
  --popover-shade-color: {shade};

  --shade-color: {shade};

  --accent-bg-color: {accent_background};
  --accent-fg-color: {accent_text};
}}
",
        window_background = &colors.window_background,
        window_text = &colors.window_text,
        view_background = &colors.view_background,
        view_text = &colors.view_text,
        headerbar_background = &colors.headerbar_background,
        headerbar_darker_shade = &colors.headerbar_darker_shade,
        border = &colors.border,
        shade = &colors.shade,
        sidebar_background = &colors.sidebar_background,
        dialog_background = &colors.dialog_background,
        dialog_text = &colors.dialog_text,
        popover_background = &colors.popover_background,
        popover_text = &colors.popover_text,
        card_background = &colors.card_background,
        card_text = &colors.card_text,
        card_shade = &colors.card_shade,
        accent_background = &colors.accent_background,
        accent_text = &colors.accent_text,
    )
}

#[cfg(test)]
pub(crate) fn exercise_chrome_css_for_tests() {
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
        let stylesheet = chrome_css(&palette_engine::derive_chrome_colors(&scheme));
        assert!(stylesheet.contains(":root"));
        assert!(stylesheet.contains("--window-bg-color:"));
        assert!(stylesheet.contains("--popover-bg-color:"));
        assert!(stylesheet.contains("--accent-bg-color:"));
        assert!(stylesheet.contains("--headerbar-backdrop-color:"));
        assert!(stylesheet.contains("--secondary-sidebar-bg-color:"));
        assert!(stylesheet.contains("--headerbar-darker-shade-color:"));
        assert!(!stylesheet.contains("--accent-color:"));
        assert!(!stylesheet.contains("@define-color"));
        assert!(!stylesheet.contains("riteed-window-chrome-"));
        assert!(!stylesheet.contains("background-color:"));
        assert!(!stylesheet.contains("box-shadow:"));
    }
}
