use std::cell::RefCell;
use std::collections::HashMap;

use gtk4::{gdk, prelude::*};
use libadwaita as adw;

const COLOR_PROBE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/compare.css";

#[derive(Clone, Copy)]
struct ProbeColors {
    added: gdk::RGBA,
    modified: gdk::RGBA,
    deleted: gdk::RGBA,
}

thread_local! {
    static PROBE_CACHE: RefCell<HashMap<(bool, bool), ProbeColors>> =
        RefCell::new(HashMap::new());
}

#[cfg(test)]
thread_local! {
    static PROBE_MISSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(crate) struct Palette {
    pub(crate) added: gdk::RGBA,
    pub(crate) modified: gdk::RGBA,
    pub(crate) deleted: gdk::RGBA,
}

impl Palette {
    pub(crate) fn from_view(view: &sourceview5::View, stale: bool) -> Self {
        let manager = adw::StyleManager::default();
        let key = (manager.is_dark(), manager.is_high_contrast());
        let colors = PROBE_CACHE.with(|cache| {
            if let Some(colors) = cache.borrow().get(&key) {
                return *colors;
            }
            #[cfg(test)]
            PROBE_MISSES.with(|misses| misses.set(misses.get().saturating_add(1)));
            let colors = probe_colors(view);
            cache.borrow_mut().insert(key, colors);
            colors
        });
        let alpha = minimap_alpha(stale);
        Self {
            added: with_alpha(&colors.added, alpha),
            modified: with_alpha(&colors.modified, alpha),
            deleted: with_alpha(&colors.deleted, alpha),
        }
    }
}

#[cfg(test)]
pub(crate) fn probe_cache_len_for_tests() -> usize {
    PROBE_CACHE.with(|cache| cache.borrow().len())
}

#[cfg(test)]
pub(crate) fn probe_miss_count_for_tests() -> usize {
    PROBE_MISSES.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn clear_probe_cache_for_tests() {
    PROBE_CACHE.with(|cache| cache.borrow_mut().clear());
    PROBE_MISSES.with(|misses| misses.set(0));
}

fn probe_colors(view: &sourceview5::View) -> ProbeColors {
    let fallback_added = adw::AccentColor::Green.to_rgba();
    let fallback_modified = adw::AccentColor::Yellow.to_rgba();
    let fallback_deleted = adw::AccentColor::Red.to_rgba();
    ProbeColors {
        added: resolve_probe_color(view, "riteed-diff-current-color-probe", &fallback_added),
        modified: resolve_probe_color(view, "riteed-diff-modified-color-probe", &fallback_modified),
        deleted: resolve_probe_color(view, "riteed-diff-reference-color-probe", &fallback_deleted),
    }
}

fn minimap_alpha(stale: bool) -> f32 {
    let high_contrast = adw::StyleManager::default().is_high_contrast();
    match (high_contrast, stale) {
        (true, true) => 0.12,
        (true, false) => 0.22,
        (false, true) => 0.04,
        (false, false) => 0.10,
    }
}

fn with_alpha(color: &gdk::RGBA, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(color.red(), color.green(), color.blue(), alpha)
}

fn resolve_probe_color(
    view: &sourceview5::View,
    css_class: &str,
    fallback: &gdk::RGBA,
) -> gdk::RGBA {
    let base = view.color();
    let display = view.display();
    let provider = gtk4::CssProvider::new();
    provider.load_from_resource(COLOR_PROBE_CSS_RESOURCE);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    view.add_css_class(css_class);
    let resolved = view.color();
    view.remove_css_class(css_class);
    gtk4::style_context_remove_provider_for_display(&display, &provider);
    if rgba_close(&resolved, &base) {
        *fallback
    } else {
        resolved
    }
}

fn rgba_close(left: &gdk::RGBA, right: &gdk::RGBA) -> bool {
    (left.red() - right.red()).abs() < f32::EPSILON
        && (left.green() - right.green()).abs() < f32::EPSILON
        && (left.blue() - right.blue()).abs() < f32::EPSILON
        && (left.alpha() - right.alpha()).abs() < f32::EPSILON
}
