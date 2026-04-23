use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::glib::translate::IntoGlib;
use gtk4::{gdk, gio, pango, prelude::*};
use libadwaita as adw;

use crate::editor_tab::EditorTab;
use crate::settings::AppSettings;
use crate::workspace::Workspace;

pub const EDITOR_VIEW_CSS_CLASS: &str = "riteed-editor-view";

const DEFAULT_FONT_FAMILY: &str = "Monospace";
const DEFAULT_FONT_SIZE_PT: i32 = 11;
const MINIMAP_FONT_SIZE_PT: i32 = 1;
const DEFAULT_VIEW_MARGIN_PX: i32 = 12;
const SCROLL_PAST_END_LINES: i32 = 10;
const DEFAULT_ZOOM_PERCENT: i32 = 100;
const MIN_ZOOM_PERCENT: i32 = 50;
const MAX_ZOOM_PERCENT: i32 = 200;
const ZOOM_STEP_PERCENT: i32 = 10;

pub struct EditorZoomController {
    workspace: Rc<Workspace>,
    display: Option<gdk::Display>,
    provider: gtk4::CssProvider,
    base_font: RefCell<pango::FontDescription>,
    zoom_percent: Cell<i32>,
}

struct ZoomApplyContext {
    editor_font: pango::FontDescription,
    minimap_font: pango::FontDescription,
    scroll_past_end_padding: i32,
}

impl EditorZoomController {
    #[must_use]
    pub fn new(
        window: &adw::ApplicationWindow,
        workspace: &Rc<Workspace>,
        settings: &AppSettings,
    ) -> Rc<Self> {
        let controller = Rc::new(Self {
            workspace: Rc::clone(workspace),
            display: gdk::Display::default(),
            provider: gtk4::CssProvider::new(),
            base_font: RefCell::new(resolve_editor_font_description(&settings.editor_font())),
            zoom_percent: Cell::new(DEFAULT_ZOOM_PERCENT),
        });
        controller.install_provider();
        controller.install_actions(window);
        controller.install_tab_hooks();
        controller.apply_to_workspace();
        controller
    }

    pub fn set_editor_font(&self, stored_font: &str) {
        self.base_font
            .replace(resolve_editor_font_description(stored_font));
        self.apply_to_workspace();
    }

    #[cfg(test)]
    #[must_use]
    pub fn zoom_percent(&self) -> i32 {
        self.zoom_percent.get()
    }

    fn install_provider(&self) {
        if let Some(display) = self.display.as_ref() {
            gtk4::style_context_add_provider_for_display(
                display,
                &self.provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
    }

    fn install_actions(self: &Rc<Self>, window: &adw::ApplicationWindow) {
        for (name, delta) in [
            ("zoom-in", ZOOM_STEP_PERCENT),
            ("zoom-out", -ZOOM_STEP_PERCENT),
        ] {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(controller) = weak.upgrade() {
                    controller.step_zoom(delta);
                }
            });
            window.add_action(&action);
        }

        let reset_action = gio::SimpleAction::new("zoom-reset", None);
        let weak = Rc::downgrade(self);
        reset_action.connect_activate(move |_, _| {
            if let Some(controller) = weak.upgrade() {
                controller.reset_zoom();
            }
        });
        window.add_action(&reset_action);
    }

    fn install_tab_hooks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.workspace
            .tab_view
            .connect_page_attached(move |_, page, _position| {
                if let Some(controller) = weak.upgrade()
                    && let Some(tab) = controller.workspace.find_tab_by_page(page)
                {
                    controller.apply_to_tab(&tab);
                }
            });

        let weak = Rc::downgrade(self);
        self.workspace
            .tab_view
            .connect_page_detached(move |_, page, _position| {
                if let Some(controller) = weak.upgrade()
                    && let Some(tab) = controller.workspace.find_tab_by_page(page)
                {
                    tab.clear_zoom_style();
                }
            });
    }

    fn step_zoom(&self, delta: i32) {
        let next = clamp_zoom_percent(self.zoom_percent.get() + delta);
        if next == self.zoom_percent.get() {
            return;
        }
        self.zoom_percent.set(next);
        self.apply_to_workspace();
    }

    fn reset_zoom(&self) {
        if self.zoom_percent.get() == DEFAULT_ZOOM_PERCENT {
            return;
        }
        self.zoom_percent.set(DEFAULT_ZOOM_PERCENT);
        self.apply_to_workspace();
    }

    fn apply_to_workspace(&self) {
        self.workspace
            .status_bar
            .set_zoom_percent(self.zoom_percent.get());
        let context = self.apply_context();
        self.update_provider_css(&context);
        for tab in self.workspace.ordered_tabs() {
            Self::apply_to_tab_with(&tab, &context);
        }
    }

    fn apply_to_tab(&self, tab: &EditorTab) {
        let context = self.apply_context();
        Self::apply_to_tab_with(tab, &context);
    }

    fn apply_to_tab_with(tab: &EditorTab, context: &ZoomApplyContext) {
        tab.restore_zoom_style();
        tab.apply_scroll_past_end_padding(context.scroll_past_end_padding);
        tab.apply_minimap_font_desc(Some(&context.minimap_font));
    }

    fn update_provider_css(&self, context: &ZoomApplyContext) {
        self.provider
            .load_from_data(&editor_view_css(&context.editor_font));
    }

    fn apply_context(&self) -> ZoomApplyContext {
        let editor_font = self.effective_font_description();
        ZoomApplyContext {
            scroll_past_end_padding: scroll_past_end_bottom_margin(&editor_font),
            editor_font,
            minimap_font: self.minimap_font_description(),
        }
    }

    fn effective_font_description(&self) -> pango::FontDescription {
        let mut effective = self.base_font.borrow().clone();
        effective.set_size(scale_font_size(
            normalized_font_size(&effective),
            self.zoom_percent.get(),
        ));
        effective
    }

    fn minimap_font_description(&self) -> pango::FontDescription {
        let mut minimap = self.base_font.borrow().clone();
        minimap.set_size(MINIMAP_FONT_SIZE_PT * pango::SCALE);
        minimap
    }
}

impl Drop for EditorZoomController {
    fn drop(&mut self) {
        if let Some(display) = self.display.as_ref() {
            gtk4::style_context_remove_provider_for_display(display, &self.provider);
        }
    }
}

#[must_use]
pub fn resolve_editor_font_description(stored_font: &str) -> pango::FontDescription {
    let trimmed = stored_font.trim();
    if trimmed.is_empty() {
        return fallback_font_description();
    }

    let mut desc = pango::FontDescription::from_string(trimmed);
    if desc.family().is_none() {
        return fallback_font_description();
    }
    if desc.size() <= 0 {
        desc.set_size(DEFAULT_FONT_SIZE_PT * pango::SCALE);
    }
    desc
}

#[must_use]
pub fn font_row_subtitle(stored_font: &str) -> String {
    let desc = resolve_editor_font_description(stored_font);
    let label = desc.to_string();
    if label.is_empty() {
        gettext(DEFAULT_FONT_FAMILY)
    } else {
        label
    }
}

#[must_use]
pub fn resolve_minimap_font_description(stored_font: &str) -> pango::FontDescription {
    let mut desc = resolve_editor_font_description(stored_font);
    desc.set_size(MINIMAP_FONT_SIZE_PT * pango::SCALE);
    desc
}

#[must_use]
pub fn resolve_scroll_past_end_padding(stored_font: &str) -> i32 {
    scroll_past_end_bottom_margin(&resolve_editor_font_description(stored_font))
}

#[must_use]
pub fn resolve_font_family(
    parent: &adw::ApplicationWindow,
    desc: &pango::FontDescription,
) -> Option<pango::FontFamily> {
    let font_map = parent.font_map()?;
    resolve_font_family_in_map(&font_map, desc)
}

#[must_use]
pub(crate) fn resolve_font_family_in_map(
    font_map: &pango::FontMap,
    desc: &pango::FontDescription,
) -> Option<pango::FontFamily> {
    let family_name = desc.family()?;
    if let Some(family) = font_map.family(family_name.as_str()) {
        return Some(family);
    }

    font_map
        .list_families()
        .into_iter()
        .filter(|family| font_family_name_matches(family_name.as_str(), family.name().as_str()))
        .max_by_key(|family| family.name().len())
}

#[must_use]
fn font_family_name_matches(requested: &str, candidate: &str) -> bool {
    requested == candidate
        || requested
            .strip_prefix(candidate)
            .is_some_and(|suffix| suffix.starts_with(' '))
}

#[must_use]
pub fn clamp_zoom_percent(percent: i32) -> i32 {
    percent.clamp(MIN_ZOOM_PERCENT, MAX_ZOOM_PERCENT)
}

#[must_use]
fn fallback_font_description() -> pango::FontDescription {
    let mut desc = pango::FontDescription::new();
    desc.set_family(DEFAULT_FONT_FAMILY);
    desc.set_size(DEFAULT_FONT_SIZE_PT * pango::SCALE);
    desc
}

#[must_use]
fn normalized_font_size(desc: &pango::FontDescription) -> i32 {
    let size = desc.size();
    if size > 0 {
        size
    } else {
        DEFAULT_FONT_SIZE_PT * pango::SCALE
    }
}

#[must_use]
fn scale_font_size(size: i32, percent: i32) -> i32 {
    let scaled = i64::from(size) * i64::from(percent) / i64::from(DEFAULT_ZOOM_PERCENT);
    i32::try_from(scaled).unwrap_or(DEFAULT_FONT_SIZE_PT * pango::SCALE)
}

#[must_use]
fn scroll_past_end_bottom_margin(desc: &pango::FontDescription) -> i32 {
    let line_height_px = (normalized_font_size(desc) / pango::SCALE).max(1);
    DEFAULT_VIEW_MARGIN_PX.max(line_height_px.saturating_mul(SCROLL_PAST_END_LINES))
}

#[must_use]
fn editor_view_css(desc: &pango::FontDescription) -> String {
    let family = css_escape(desc.family().as_deref().unwrap_or(DEFAULT_FONT_FAMILY));
    let size_points = f64::from(desc.size()) / f64::from(pango::SCALE);
    let style = match desc.style() {
        pango::Style::Italic => "italic",
        pango::Style::Oblique => "oblique",
        _ => "normal",
    };
    let weight = desc.weight().into_glib();
    format!(
        ".{EDITOR_VIEW_CSS_CLASS} {{ font-family: \"{family}\"; font-size: {size_points:.2}pt; font-style: {style}; font-weight: {weight}; }}"
    )
}

#[must_use]
fn css_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_FONT_SIZE_PT, clamp_zoom_percent, editor_view_css, font_family_name_matches,
        font_row_subtitle, resolve_editor_font_description, resolve_minimap_font_description,
        resolve_scroll_past_end_padding, scale_font_size,
    };

    #[test]
    fn zoom_percent_clamps() {
        assert_eq!(clamp_zoom_percent(40), 50);
        assert_eq!(clamp_zoom_percent(100), 100);
        assert_eq!(clamp_zoom_percent(220), 200);
    }

    #[test]
    fn empty_font_falls_back_to_monospace() {
        let desc = resolve_editor_font_description("");
        assert_eq!(desc.family().as_deref(), Some("Monospace"));
        assert_eq!(desc.size(), DEFAULT_FONT_SIZE_PT * gtk4::pango::SCALE);
    }

    #[test]
    fn invalid_font_without_family_falls_back() {
        let desc = resolve_editor_font_description("12");
        assert_eq!(desc.family().as_deref(), Some("Monospace"));
    }

    #[test]
    fn zoom_scale_uses_percent() {
        assert_eq!(
            scale_font_size(11 * gtk4::pango::SCALE, 200),
            22 * gtk4::pango::SCALE
        );
    }

    #[test]
    fn subtitle_uses_resolved_font() {
        assert!(font_row_subtitle("Monospace 13").contains("Monospace"));
    }

    #[test]
    fn subtitle_uses_default_label_for_empty_font() {
        assert_eq!(font_row_subtitle(""), "Monospace 11");
    }

    #[test]
    fn font_without_size_gets_default_size() {
        let desc = resolve_editor_font_description("Monospace");
        assert_eq!(desc.family().as_deref(), Some("Monospace"));
        assert_eq!(desc.size(), DEFAULT_FONT_SIZE_PT * gtk4::pango::SCALE);
    }

    #[test]
    fn minimap_font_uses_fixed_small_size() {
        let desc = resolve_minimap_font_description("JetBrains Mono 14");
        assert_eq!(desc.family().as_deref(), Some("JetBrains Mono"));
        assert_eq!(desc.size(), gtk4::pango::SCALE);
    }

    #[test]
    fn scroll_past_end_padding_scales_with_font_size() {
        assert_eq!(resolve_scroll_past_end_padding("Monospace 11"), 110);
        assert_eq!(resolve_scroll_past_end_padding("Monospace 22"), 220);
    }

    #[test]
    fn editor_css_escapes_font_family() {
        let desc = resolve_editor_font_description("JetBrains Mono");
        let css = editor_view_css(&desc);
        assert!(css.contains(".riteed-editor-view"));
        assert!(css.contains("font-family: \"JetBrains Mono\""));
    }

    #[test]
    fn family_name_match_accepts_style_suffix() {
        assert!(font_family_name_matches("FreeMono Regular", "FreeMono"));
        assert!(font_family_name_matches(
            "JetBrains Mono Bold",
            "JetBrains Mono"
        ));
        assert!(!font_family_name_matches("FreeMonoRegular", "FreeMono"));
        assert!(!font_family_name_matches("FreeSans", "FreeMono"));
    }
}
