use gtk4::gdk;

use crate::settings::{EditorPalette, WindowPalette};

pub(crate) const ADWAITA_LIGHT_SCHEME: &str = "Adwaita";
pub(crate) const ADWAITA_DARK_SCHEME: &str = "Adwaita-dark";

const CLASSIC_LIGHT_SCHEME: &str = "classic";
const CLASSIC_DARK_SCHEME: &str = "classic-dark";
const KATE_LIGHT_SCHEME: &str = "kate";
const KATE_DARK_SCHEME: &str = "kate-dark";
const SOLARIZED_LIGHT_SCHEME: &str = "solarized-light";
const SOLARIZED_DARK_SCHEME: &str = "solarized-dark";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PaletteFamily {
    Adwaita,
    Classic,
    Kate,
    Solarized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SchemePolarity {
    Light,
    Dark,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedScheme {
    pub(crate) id: String,
    pub(crate) family: PaletteFamily,
    pub(crate) polarity: SchemePolarity,
    pub(crate) available: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChromeColors {
    pub(crate) window_bg: gdk::RGBA,
    pub(crate) window_fg: gdk::RGBA,
    pub(crate) view_bg: gdk::RGBA,
    pub(crate) view_fg: gdk::RGBA,
    pub(crate) headerbar_bg: gdk::RGBA,
    pub(crate) tabbar_bg: gdk::RGBA,
    pub(crate) active_tab_bg: gdk::RGBA,
    pub(crate) hover_tab_bg: gdk::RGBA,
    pub(crate) active_tab_indicator: gdk::RGBA,
    pub(crate) sidebar_bg: gdk::RGBA,
    pub(crate) statusbar_bg: gdk::RGBA,
    pub(crate) popover_bg: gdk::RGBA,
    pub(crate) popover_fg: gdk::RGBA,
    pub(crate) dialog_bg: gdk::RGBA,
    pub(crate) dialog_fg: gdk::RGBA,
    pub(crate) card_bg: gdk::RGBA,
    pub(crate) card_fg: gdk::RGBA,
    pub(crate) card_shade: gdk::RGBA,
    pub(crate) accent_bg: gdk::RGBA,
    pub(crate) accent_fg: gdk::RGBA,
    pub(crate) border: gdk::RGBA,
    pub(crate) shade: gdk::RGBA,
}

#[derive(Clone, Copy)]
pub(crate) enum StyleColorSource {
    Foreground,
    Background,
}

impl SchemePolarity {
    #[must_use]
    pub(crate) const fn from_dark(dark: bool) -> Self {
        if dark { Self::Dark } else { Self::Light }
    }

    #[must_use]
    const fn suffix(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl PaletteFamily {
    #[must_use]
    pub(crate) const fn default_scheme_id(self, polarity: SchemePolarity) -> &'static str {
        match (self, polarity) {
            (Self::Adwaita, SchemePolarity::Light) => ADWAITA_LIGHT_SCHEME,
            (Self::Adwaita, SchemePolarity::Dark) => ADWAITA_DARK_SCHEME,
            (Self::Classic, SchemePolarity::Light) => CLASSIC_LIGHT_SCHEME,
            (Self::Classic, SchemePolarity::Dark) => CLASSIC_DARK_SCHEME,
            (Self::Kate, SchemePolarity::Light) => KATE_LIGHT_SCHEME,
            (Self::Kate, SchemePolarity::Dark) => KATE_DARK_SCHEME,
            (Self::Solarized, SchemePolarity::Light) => SOLARIZED_LIGHT_SCHEME,
            (Self::Solarized, SchemePolarity::Dark) => SOLARIZED_DARK_SCHEME,
        }
    }
}

#[must_use]
pub(crate) fn editor_scheme_id(palette: EditorPalette, app_dark: bool) -> String {
    if let Some(scheme_id) = palette.scheme_id()
        && scheme_available(scheme_id)
    {
        return String::from(scheme_id);
    }
    String::from(default_adwaita_scheme(SchemePolarity::from_dark(app_dark)))
}

#[must_use]
pub(crate) fn editor_palette_is_dark(palette: EditorPalette, app_dark: bool) -> bool {
    let scheme_id = editor_scheme_id(palette, app_dark);
    sourceview5::StyleSchemeManager::default()
        .scheme(&scheme_id)
        .map_or(app_dark, |scheme| {
            scheme_polarity(&scheme) == SchemePolarity::Dark
        })
}

#[must_use]
pub(crate) fn editor_palette_family(palette: EditorPalette) -> PaletteFamily {
    match palette {
        EditorPalette::ClassicLight | EditorPalette::ClassicDark => PaletteFamily::Classic,
        EditorPalette::Kate | EditorPalette::KateDark => PaletteFamily::Kate,
        EditorPalette::SolarizedLight | EditorPalette::SolarizedDark => PaletteFamily::Solarized,
        EditorPalette::FollowSystem | EditorPalette::AdwaitaLight | EditorPalette::AdwaitaDark => {
            PaletteFamily::Adwaita
        }
    }
}

#[must_use]
pub(crate) fn window_palette_family(
    window_palette: WindowPalette,
    editor_palette: EditorPalette,
) -> PaletteFamily {
    match window_palette {
        WindowPalette::FollowEditor => editor_palette_family(editor_palette),
        WindowPalette::Adwaita => PaletteFamily::Adwaita,
        WindowPalette::Classic => PaletteFamily::Classic,
        WindowPalette::Kate => PaletteFamily::Kate,
        WindowPalette::Solarized => PaletteFamily::Solarized,
    }
}

#[must_use]
pub(crate) fn resolve_family_scheme(
    family: PaletteFamily,
    polarity: SchemePolarity,
) -> ResolvedScheme {
    let preferred = family.default_scheme_id(polarity);
    let manager = sourceview5::StyleSchemeManager::default();
    if manager.scheme(preferred).is_some() {
        return ResolvedScheme {
            id: String::from(preferred),
            family,
            polarity,
            available: true,
        };
    }
    let fallback = family.default_scheme_id(match polarity {
        SchemePolarity::Light => SchemePolarity::Dark,
        SchemePolarity::Dark => SchemePolarity::Light,
    });
    if let Some(scheme) = manager.scheme(fallback) {
        let id = variant_scheme_id(&scheme, polarity);
        let available = manager.scheme(&id).is_some();
        return ResolvedScheme {
            id,
            family,
            polarity,
            available,
        };
    }
    ResolvedScheme {
        id: String::from(default_adwaita_scheme(polarity)),
        family: PaletteFamily::Adwaita,
        polarity,
        available: manager.scheme(default_adwaita_scheme(polarity)).is_some(),
    }
}

#[must_use]
pub(crate) fn window_scheme_id(
    window_palette: WindowPalette,
    editor_palette: EditorPalette,
    app_dark: bool,
) -> Option<String> {
    let polarity = SchemePolarity::from_dark(app_dark);
    let family = window_palette_family(window_palette, editor_palette);
    let resolved = resolve_family_scheme(family, polarity);
    resolved.available.then_some(resolved.id)
}

#[must_use]
pub(crate) fn variant_scheme_id(
    scheme: &sourceview5::StyleScheme,
    polarity: SchemePolarity,
) -> String {
    let manager = sourceview5::StyleSchemeManager::default();
    let key = match polarity {
        SchemePolarity::Light => "light-variant",
        SchemePolarity::Dark => "dark-variant",
    };
    if let Some(mapped) = scheme.metadata(key)
        && manager.scheme(mapped.as_str()).is_some()
    {
        return mapped.to_string();
    }

    let id = scheme.id();
    let id = id.as_str();
    let base = id
        .strip_suffix("-light")
        .or_else(|| id.strip_suffix("-dark"))
        .unwrap_or(id);
    let candidate = format!("{base}-{}", polarity.suffix());
    if manager.scheme(&candidate).is_some() {
        return candidate;
    }
    if manager.scheme(base).is_some() {
        return String::from(base);
    }
    String::from(id)
}

#[must_use]
pub(crate) fn scheme_polarity(scheme: &sourceview5::StyleScheme) -> SchemePolarity {
    if let Some(variant) = scheme.metadata("variant") {
        match variant.as_str() {
            "dark" => return SchemePolarity::Dark,
            "light" => return SchemePolarity::Light,
            _ => {}
        }
    }
    if scheme.id().as_str().contains("-dark") {
        return SchemePolarity::Dark;
    }
    let background = style_color(
        scheme,
        &["text"],
        StyleColorSource::Background,
        &gdk::RGBA::WHITE,
    );
    if luma(&background) <= 0.5 {
        SchemePolarity::Dark
    } else {
        SchemePolarity::Light
    }
}

#[must_use]
pub(crate) fn derive_chrome_colors(scheme: &sourceview5::StyleScheme) -> ChromeColors {
    let polarity = scheme_polarity(scheme);
    let dark = polarity == SchemePolarity::Dark;
    let (surface_background, surface_text_color) = surface_colors(scheme, dark);
    let line_or_text_bg = opaque(style_color(
        scheme,
        &["line-numbers", "text"],
        StyleColorSource::Background,
        &surface_background,
    ));
    let (selection_background, selection_text_color) =
        selection_colors(scheme, &surface_background, &surface_text_color);
    let headerbar_bg = mix(
        &line_or_text_bg,
        &surface_text_color,
        if dark { 0.10 } else { 0.06 },
    );
    let tabbar_bg = headerbar_bg;
    let active_tab_bg = surface_background;
    let hover_tab_bg = mix(
        &tabbar_bg,
        &surface_text_color,
        if dark { 0.04 } else { 0.02 },
    );
    let sidebar_bg = mix(
        &surface_background,
        &surface_text_color,
        if dark { 0.05 } else { 0.03 },
    );
    let statusbar_bg = headerbar_bg;
    let popover_bg = mix(
        &surface_background,
        &surface_text_color,
        if dark { 0.08 } else { 0.04 },
    );
    let dialog_bg = popover_bg;
    let card_bg = if dark {
        mix(&surface_background, &surface_text_color, 0.08)
    } else {
        mix(&surface_background, &gdk::RGBA::WHITE, 0.55)
    };
    let border = mix(&surface_background, &surface_text_color, 0.15);

    ChromeColors {
        window_bg: surface_background,
        window_fg: surface_text_color,
        view_bg: surface_background,
        view_fg: surface_text_color,
        headerbar_bg,
        tabbar_bg,
        active_tab_bg,
        hover_tab_bg,
        active_tab_indicator: selection_background,
        sidebar_bg,
        statusbar_bg,
        popover_bg,
        popover_fg: surface_text_color,
        dialog_bg,
        dialog_fg: surface_text_color,
        card_bg,
        card_fg: surface_text_color,
        card_shade: border,
        accent_bg: selection_background,
        accent_fg: selection_text_color,
        border,
        shade: border,
    }
}

fn surface_colors(scheme: &sourceview5::StyleScheme, dark: bool) -> (gdk::RGBA, gdk::RGBA) {
    let default_background = if dark {
        gdk::RGBA::new(0.11, 0.12, 0.13, 1.0)
    } else {
        gdk::RGBA::WHITE
    };
    let default_text_color = if dark {
        gdk::RGBA::new(0.88, 0.88, 0.86, 1.0)
    } else {
        gdk::RGBA::new(0.16, 0.16, 0.15, 1.0)
    };
    (
        opaque(style_color(
            scheme,
            &["text"],
            StyleColorSource::Background,
            &default_background,
        )),
        opaque(style_color(
            scheme,
            &["text"],
            StyleColorSource::Foreground,
            &default_text_color,
        )),
    )
}

fn selection_colors(
    scheme: &sourceview5::StyleScheme,
    surface_background: &gdk::RGBA,
    surface_text_color: &gdk::RGBA,
) -> (gdk::RGBA, gdk::RGBA) {
    let selection_background = opaque(style_color(
        scheme,
        &["selection"],
        StyleColorSource::Background,
        &mix(surface_background, surface_text_color, 0.35),
    ));
    let fallback_selection_text = if luma(&selection_background) > 0.5 {
        gdk::RGBA::BLACK
    } else {
        gdk::RGBA::WHITE
    };
    (
        selection_background,
        opaque(style_color(
            scheme,
            &["selection"],
            StyleColorSource::Foreground,
            &fallback_selection_text,
        )),
    )
}

#[must_use]
pub(crate) fn style_color(
    scheme: &sourceview5::StyleScheme,
    style_ids: &[&str],
    source: StyleColorSource,
    fallback: &gdk::RGBA,
) -> gdk::RGBA {
    style_ids
        .iter()
        .find_map(|style_id| {
            let style = scheme.style(style_id)?;
            let color = match source {
                StyleColorSource::Foreground if style.is_foreground_set() => style.foreground(),
                StyleColorSource::Background if style.is_background_set() => {
                    style.background().or_else(|| style.line_background())
                }
                StyleColorSource::Background if style.is_line_background_set() => {
                    style.line_background()
                }
                _ => None,
            }?;
            gdk::RGBA::parse(color.as_str()).ok()
        })
        .unwrap_or(*fallback)
}

#[must_use]
pub(crate) fn luma(color: &gdk::RGBA) -> f32 {
    (0.2126 * color.red()) + (0.7152 * color.green()) + (0.0722 * color.blue())
}

#[must_use]
pub(crate) fn mix(first: &gdk::RGBA, second: &gdk::RGBA, amount: f32) -> gdk::RGBA {
    let inverse = 1.0 - amount;
    opaque(gdk::RGBA::new(
        (first.red() * inverse) + (second.red() * amount),
        (first.green() * inverse) + (second.green() * amount),
        (first.blue() * inverse) + (second.blue() * amount),
        1.0,
    ))
}

#[must_use]
pub(crate) fn rgba_to_css(color: &gdk::RGBA) -> String {
    format!(
        "rgba({:.0}, {:.0}, {:.0}, {:.3})",
        color.red() * 255.0,
        color.green() * 255.0,
        color.blue() * 255.0,
        color.alpha()
    )
}

#[must_use]
fn default_adwaita_scheme(polarity: SchemePolarity) -> &'static str {
    match polarity {
        SchemePolarity::Light => ADWAITA_LIGHT_SCHEME,
        SchemePolarity::Dark => ADWAITA_DARK_SCHEME,
    }
}

#[must_use]
fn scheme_available(scheme_id: &str) -> bool {
    sourceview5::StyleSchemeManager::default()
        .scheme(scheme_id)
        .is_some()
}

#[must_use]
fn opaque(color: gdk::RGBA) -> gdk::RGBA {
    color.with_alpha(1.0)
}

#[cfg(test)]
pub(crate) fn exercise_palette_engine_for_tests() {
    let manager = sourceview5::StyleSchemeManager::default();
    assert_eq!(
        editor_scheme_id(EditorPalette::ClassicLight, false),
        "classic"
    );
    assert_eq!(
        editor_scheme_id(EditorPalette::ClassicDark, false),
        "classic-dark"
    );
    assert_eq!(
        editor_scheme_id(EditorPalette::FollowSystem, true),
        ADWAITA_DARK_SCHEME
    );

    let light = resolve_family_scheme(PaletteFamily::Classic, SchemePolarity::Light);
    let dark = resolve_family_scheme(PaletteFamily::Classic, SchemePolarity::Dark);
    assert_eq!(light.id, "classic");
    assert_eq!(dark.id, "classic-dark");
    assert!(light.available);
    assert!(dark.available);

    if let Some(classic_dark) = manager.scheme("classic-dark") {
        assert_eq!(scheme_polarity(&classic_dark), SchemePolarity::Dark);
    }
    for scheme_id in [
        ADWAITA_LIGHT_SCHEME,
        ADWAITA_DARK_SCHEME,
        CLASSIC_LIGHT_SCHEME,
        CLASSIC_DARK_SCHEME,
        KATE_LIGHT_SCHEME,
        KATE_DARK_SCHEME,
        SOLARIZED_LIGHT_SCHEME,
        SOLARIZED_DARK_SCHEME,
    ] {
        let Some(scheme) = manager.scheme(scheme_id) else {
            continue;
        };
        let colors = derive_chrome_colors(&scheme);
        assert!(colors.window_bg.alpha() >= 1.0);
        assert!(colors.window_fg.alpha() >= 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_uses_component_linear_interpolation() {
        let first = gdk::RGBA::new(0.0, 0.5, 1.0, 0.25);
        let second = gdk::RGBA::new(1.0, 0.0, 0.0, 0.75);
        let mixed = mix(&first, &second, 0.25);
        assert!((mixed.red() - 0.25).abs() < 0.001);
        assert!((mixed.green() - 0.375).abs() < 0.001);
        assert!((mixed.blue() - 0.75).abs() < 0.001);
        assert!((mixed.alpha() - 1.0).abs() < 0.001);
    }

    #[test]
    fn luma_uses_bt709_weights() {
        let color = gdk::RGBA::new(1.0, 1.0, 1.0, 1.0);
        assert!((luma(&color) - 1.0).abs() < 0.001);
    }
}
