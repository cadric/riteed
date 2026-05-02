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
    tabbar_background: String,
    active_tab_background: String,
    hover_tab_background: String,
    sidebar_background: String,
    statusbar_background: String,
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
            tabbar_background: rgba_to_css(&colors.tabbar_bg),
            active_tab_background: rgba_to_css(&colors.active_tab_bg),
            hover_tab_background: rgba_to_css(&colors.hover_tab_bg),
            sidebar_background: rgba_to_css(&colors.sidebar_bg),
            statusbar_background: rgba_to_css(&colors.statusbar_bg),
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

pub(crate) fn chrome_css_for_settings(css_class: &str, settings: &AppSettings) -> String {
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
            chrome_css(css_class, &palette_engine::derive_chrome_colors(&scheme))
        })
}

pub(crate) fn chrome_css(css_class: &str, colors: &ChromeColors) -> String {
    let colors = CssColors::new(colors);
    format!(
        "{}{}{}",
        variable_css(css_class, &colors),
        surface_css(css_class, &colors),
        popover_and_card_css(css_class, &colors),
    )
}

fn variable_css(css_class: &str, colors: &CssColors) -> String {
    format!(
        ".{css_class},
.{css_class} dialog,
dialog.{css_class},
popover.{css_class} {{
  --window-bg-color: {window_background};
  --window-fg-color: {window_text};
  --view-bg-color: {view_background};
  --view-fg-color: {view_text};
  --headerbar-bg-color: {headerbar_background};
  --headerbar-fg-color: {window_text};
  --headerbar-border-color: {border};
  --headerbar-shade-color: {shade};
  --sidebar-bg-color: {sidebar_background};
  --sidebar-fg-color: {window_text};
  --sidebar-border-color: {border};
  --sidebar-shade-color: {shade};
  --dialog-bg-color: {dialog_background};
  --dialog-fg-color: {dialog_text};
  --popover-bg-color: {popover_background};
  --popover-fg-color: {popover_text};
  --popover-shade-color: {shade};
  --card-bg-color: {card_background};
  --card-fg-color: {card_text};
  --card-shade-color: {card_shade};
  --accent-bg-color: {accent_background};
  --accent-fg-color: {accent_text};
  --shade-color: {shade};
  background-color: {window_background};
  color: {window_text};
}}
",
        window_background = &colors.window_background,
        window_text = &colors.window_text,
        view_background = &colors.view_background,
        view_text = &colors.view_text,
        headerbar_background = &colors.headerbar_background,
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

fn surface_css(css_class: &str, colors: &CssColors) -> String {
    format!(
        ".{css_class} dialog,
dialog.{css_class} {{
  background-color: var(--dialog-bg-color);
  color: var(--dialog-fg-color);
}}
.{css_class} dialog preferencespage,
dialog.{css_class} preferencespage {{
  background-color: var(--dialog-bg-color);
  color: var(--dialog-fg-color);
}}
.{css_class} headerbar,
.{css_class} .toolbar {{
  background-color: var(--headerbar-bg-color);
  color: var(--headerbar-fg-color);
  box-shadow: inset 0 -1px var(--headerbar-border-color);
}}
.{css_class} .riteed-tab-bar {{
  background-color: {tabbar_background};
  color: {window_text};
  box-shadow: inset 0 -1px {border};
}}
.{css_class} .riteed-tab-view {{
  background-color: {active_tab_background};
  color: {window_text};
}}
.{css_class} .riteed-sidebar-host,
.{css_class} .riteed-sidebar-stack,
.{css_class} .riteed-sidebar-content {{
  background-color: var(--sidebar-bg-color);
  color: var(--sidebar-fg-color);
}}
.{css_class} .riteed-sidebar-host {{
  box-shadow: inset -1px 0 var(--sidebar-border-color);
}}
.{css_class} .riteed-sidebar-host headerbar,
.{css_class} .riteed-sidebar-header {{
  background-color: var(--sidebar-bg-color);
  color: var(--sidebar-fg-color);
  box-shadow: inset 0 -1px var(--sidebar-border-color);
}}
.{css_class} .riteed-sidebar-switcher {{
  color: var(--sidebar-fg-color);
}}
.{css_class} .riteed-sidebar-host listview,
.{css_class} .riteed-sidebar-host listview row,
.{css_class} .riteed-sidebar-row,
.{css_class} .riteed-git-row {{
  background-color: transparent;
  color: var(--sidebar-fg-color);
}}
.{css_class} .riteed-sidebar-host listview row:hover,
.{css_class} .riteed-sidebar-row:hover,
.{css_class} .riteed-git-row:hover {{
  background-color: {hover_tab_background};
}}
.{css_class} .riteed-git-row .riteed-git-row-actions {{
  background: linear-gradient(to right, transparent, {sidebar_background} 24px);
}}
.{css_class} .riteed-status-bar {{
  background-color: {statusbar_background};
  color: {window_text};
  box-shadow: inset 0 1px {border};
}}
",
        tabbar_background = &colors.tabbar_background,
        window_text = &colors.window_text,
        border = &colors.border,
        active_tab_background = &colors.active_tab_background,
        hover_tab_background = &colors.hover_tab_background,
        sidebar_background = &colors.sidebar_background,
        statusbar_background = &colors.statusbar_background,
    )
}

fn popover_and_card_css(css_class: &str, colors: &CssColors) -> String {
    format!(
        ".{css_class} banner {{
  background-color: {popover_background};
  color: {popover_text};
  box-shadow: inset 0 -1px {border};
}}
.{css_class} popover contents,
popover.{css_class} contents {{
  background-color: var(--popover-bg-color);
  color: var(--popover-fg-color);
  box-shadow: 0 0 0 1px var(--popover-shade-color);
}}
.{css_class} popover button.model:hover,
popover.{css_class} button.model:hover {{
  background-color: {hover_tab_background};
}}
.{css_class} .card,
.{css_class} .boxed-list,
dialog.{css_class} .card,
dialog.{css_class} .boxed-list {{
  background-color: var(--card-bg-color);
  color: var(--card-fg-color);
}}
",
        popover_background = &colors.popover_background,
        popover_text = &colors.popover_text,
        border = &colors.border,
        hover_tab_background = &colors.hover_tab_background,
    )
}

#[cfg(test)]
pub(crate) fn css_is_scoped_for_tests(stylesheet: &str, css_class: &str) -> bool {
    stylesheet
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("--")
                && !trimmed.starts_with("background")
                && !trimmed.starts_with("color")
                && !trimmed.starts_with("box-shadow")
                && !trimmed.starts_with('}')
        })
        .all(|line| line.contains(css_class))
}

#[cfg(test)]
pub(crate) fn exercise_chrome_css_for_tests() {
    let manager = sourceview5::StyleSchemeManager::default();
    let Some(scheme) = manager.scheme("solarized-dark") else {
        return;
    };
    let stylesheet = chrome_css(
        "riteed-test-scope",
        &palette_engine::derive_chrome_colors(&scheme),
    );
    assert!(css_is_scoped_for_tests(&stylesheet, "riteed-test-scope"));
    assert!(!stylesheet.contains("@define-color"));
    assert!(!stylesheet.contains("@window_bg_color"));
    assert!(!stylesheet.contains("tabbar tab"));
    assert!(stylesheet.contains("--dialog-bg-color"));
    assert!(stylesheet.contains("--popover-bg-color"));
    assert!(stylesheet.contains("--accent-bg-color"));
    assert!(!stylesheet.contains("--destructive-bg-color"));
}
