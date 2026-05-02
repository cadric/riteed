use std::cell::{Cell, RefCell};
use std::f64::consts::{FRAC_PI_2, PI};
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::accessible::{Property, State};
use gtk4::{AccessibleTristate, cairo, prelude::*};

use crate::palette_engine::{self, ChromeColors};
use crate::settings::{AppSettings, WindowPalette};

const PREVIEW_WIDTH: i32 = 132;
const PREVIEW_HEIGHT: i32 = 92;
const PREVIEW_RADIUS: f64 = 9.0;

#[derive(Clone)]
pub(super) struct WindowPaletteGrid {
    state: Rc<WindowGridState>,
}

struct WindowGridState {
    settings: AppSettings,
    flow_box: gtk4::FlowBox,
    syncing: Cell<bool>,
    tiles: RefCell<Vec<WindowTile>>,
}

#[derive(Clone)]
struct WindowTile {
    palette: WindowPalette,
    child: gtk4::FlowBoxChild,
    preview: Option<ChromePreview>,
}

struct WindowTarget {
    scheme_id: String,
    available: bool,
}

#[derive(Clone)]
struct ChromePreview {
    widget: gtk4::Overlay,
    selected_ring: gtk4::Box,
    selected_badge: gtk4::Image,
}

impl WindowPaletteGrid {
    pub(super) fn new(settings: &AppSettings, flow_box: &gtk4::FlowBox) -> Self {
        let state = Rc::new(WindowGridState {
            settings: settings.clone(),
            flow_box: flow_box.clone(),
            syncing: Cell::new(false),
            tiles: RefCell::new(Vec::new()),
        });
        install_callbacks(&state);
        let grid = Self { state };
        grid.sync();
        grid
    }

    pub(super) fn sync(&self) {
        self.state.syncing.set(true);
        self.state.rebuild_tiles();
        self.state.refresh_selection();
        self.state.syncing.set(false);
    }

    pub(super) fn queue_resize(&self) {
        self.state.flow_box.queue_resize();
        for tile in self.state.tiles.borrow().iter() {
            tile.child.queue_resize();
            if let Some(preview) = tile.preview.as_ref() {
                preview.queue_resize();
            }
        }
    }

    #[cfg(test)]
    pub(super) fn set_palette_for_tests(&self, palette: WindowPalette) {
        self.state.activate_palette(palette);
    }

    #[cfg(test)]
    pub(super) fn selected_palette_for_ui(&self) -> WindowPalette {
        self.state.selected_palette_for_ui()
    }
}

impl WindowGridState {
    fn rebuild_tiles(&self) {
        super::clear_flow_box(&self.flow_box);
        self.tiles.borrow_mut().clear();
        for palette in WindowPalette::ALL {
            let target = window_target(&self.settings, palette);
            let label = palette.label();
            let (content, preview) = window_tile_content(palette, target.as_ref(), &label);
            let child = gtk4::FlowBoxChild::builder()
                .accessible_role(gtk4::AccessibleRole::Radio)
                .focusable(true)
                .child(&content)
                .build();
            child.add_css_class("riteed-palette-tile");
            child.set_tooltip_text(Some(&label));
            self.flow_box.append(&child);
            self.tiles.borrow_mut().push(WindowTile {
                palette,
                child,
                preview,
            });
        }
    }

    fn refresh_selection(&self) {
        let selected = self.selected_palette_for_ui();
        for tile in self.tiles.borrow().iter() {
            let target = window_target(&self.settings, tile.palette);
            let unavailable = target.as_ref().is_none_or(|target| !target.available);
            let is_selected = tile.palette == selected;
            tile.child.set_sensitive(!unavailable);
            if let Some(preview) = tile.preview.as_ref() {
                preview.set_selected(is_selected);
            }
            update_tile_accessibility(&tile.child, tile.palette, unavailable, is_selected);
        }
        if let Some(active_child) = self
            .tiles
            .borrow()
            .iter()
            .find(|tile| tile.palette == selected)
            .map(|tile| tile.child.clone())
        {
            self.flow_box.select_child(&active_child);
        } else {
            self.flow_box.unselect_all();
        }
    }

    fn selected_palette_for_ui(&self) -> WindowPalette {
        let palette = self.settings.window_palette();
        if window_palette_available(&self.settings, palette) {
            palette
        } else {
            WindowPalette::FollowEditor
        }
    }

    fn activate_palette(&self, palette: WindowPalette) {
        if self.syncing.get() || !window_palette_available(&self.settings, palette) {
            return;
        }
        self.settings.set_window_palette(palette);
        self.syncing.set(true);
        self.refresh_selection();
        self.syncing.set(false);
    }
}

impl ChromePreview {
    fn new(colors: &ChromeColors) -> Self {
        let colors = *colors;
        let area = gtk4::DrawingArea::new();
        area.set_content_width(PREVIEW_WIDTH);
        area.set_content_height(PREVIEW_HEIGHT);
        area.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        area.set_can_focus(false);
        area.set_can_target(false);
        area.set_draw_func(move |_, context, width, height| {
            draw_chrome_preview(context, f64::from(width), f64::from(height), &colors);
        });

        let selected_ring = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        selected_ring.add_css_class("riteed-palette-selected-ring");
        selected_ring.set_halign(gtk4::Align::Fill);
        selected_ring.set_valign(gtk4::Align::Fill);
        selected_ring.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        selected_ring.set_visible(false);
        selected_ring.set_can_focus(false);
        selected_ring.set_can_target(false);

        let selected_badge = gtk4::Image::from_icon_name("object-select-symbolic");
        selected_badge.add_css_class("riteed-palette-selected-badge");
        selected_badge.set_halign(gtk4::Align::End);
        selected_badge.set_valign(gtk4::Align::End);
        selected_badge.set_pixel_size(14);
        selected_badge.set_visible(false);
        selected_badge.set_can_focus(false);
        selected_badge.set_can_target(false);

        let widget = gtk4::Overlay::new();
        widget.add_css_class("riteed-palette-preview");
        widget.set_halign(gtk4::Align::Center);
        widget.set_valign(gtk4::Align::Center);
        widget.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        widget.set_can_focus(false);
        widget.set_can_target(false);
        widget.set_child(Some(&area));
        widget.add_overlay(&selected_ring);
        widget.add_overlay(&selected_badge);
        Self {
            widget,
            selected_ring,
            selected_badge,
        }
    }

    fn widget(&self) -> gtk4::Widget {
        self.widget.clone().upcast::<gtk4::Widget>()
    }

    fn set_selected(&self, selected: bool) {
        if selected {
            self.widget.add_css_class("selected");
        } else {
            self.widget.remove_css_class("selected");
        }
        self.selected_ring.set_visible(selected);
        self.selected_badge.set_visible(selected);
    }

    fn queue_resize(&self) {
        self.widget.queue_resize();
    }
}

fn install_callbacks(state: &Rc<WindowGridState>) {
    let weak = Rc::downgrade(state);
    state.flow_box.connect_child_activated(move |_, child| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        if state.syncing.get() {
            return;
        }
        let palette = state
            .tiles
            .borrow()
            .iter()
            .find(|tile| tile.child == *child)
            .map(|tile| tile.palette);
        if let Some(palette) = palette {
            state.activate_palette(palette);
        }
    });

    let weak = Rc::downgrade(state);
    state.flow_box.connect_map(move |flow_box| {
        if let Some(state) = weak.upgrade() {
            state.refresh_selection();
        }
        flow_box.queue_resize();
    });
}

fn window_tile_content(
    palette: WindowPalette,
    target: Option<&WindowTarget>,
    label: &str,
) -> (gtk4::Widget, Option<ChromePreview>) {
    let unavailable = target.is_none_or(|target| !target.available);
    let overlay = gtk4::Overlay::new();
    let (preview_widget, preview) = window_preview_widget(target);
    overlay.set_tooltip_text(Some(label));
    preview_widget.set_tooltip_text(Some(label));
    overlay.set_child(Some(&preview_widget));
    if unavailable {
        let label = gtk4::Label::new(Some(&gettext("Unavailable")));
        label.add_css_class("riteed-palette-unavailable");
        label.set_halign(gtk4::Align::Center);
        label.set_valign(gtk4::Align::Center);
        overlay.add_overlay(&label);
    }
    if palette == WindowPalette::FollowEditor {
        let badge = gtk4::Image::from_icon_name("display-brightness-symbolic");
        badge.add_css_class("riteed-palette-adaptive-badge");
        badge.set_halign(gtk4::Align::End);
        badge.set_valign(gtk4::Align::Start);
        badge.set_pixel_size(12);
        overlay.add_overlay(&badge);
    }
    (overlay.upcast::<gtk4::Widget>(), preview)
}

fn window_preview_widget(target: Option<&WindowTarget>) -> (gtk4::Widget, Option<ChromePreview>) {
    if let Some(scheme) = safe_preview_scheme(target.map(|target| target.scheme_id.as_str())) {
        let colors = palette_engine::derive_chrome_colors(&scheme);
        let preview = ChromePreview::new(&colors);
        (preview.widget(), Some(preview))
    } else {
        let label = gtk4::Label::new(Some(&gettext("Preview unavailable")));
        label.set_halign(gtk4::Align::Center);
        label.set_can_focus(false);
        label.set_can_target(false);
        label.set_size_request(PREVIEW_WIDTH, PREVIEW_HEIGHT);
        (label.upcast::<gtk4::Widget>(), None)
    }
}

fn update_tile_accessibility(
    child: &gtk4::FlowBoxChild,
    palette: WindowPalette,
    unavailable: bool,
    selected: bool,
) {
    let label = palette.label();
    child.update_property(&[Property::Label(&label)]);
    child.update_state(&[
        State::Selected(Some(selected)),
        State::Disabled(unavailable),
        State::Checked(if selected {
            AccessibleTristate::True
        } else {
            AccessibleTristate::False
        }),
    ]);
}

fn window_target(settings: &AppSettings, palette: WindowPalette) -> Option<WindowTarget> {
    let dark = libadwaita::StyleManager::default().is_dark();
    let scheme_id = palette_engine::window_scheme_id(palette, settings.editor_palette(), dark)?;
    let available = sourceview5::StyleSchemeManager::default()
        .scheme(&scheme_id)
        .is_some();
    Some(WindowTarget {
        scheme_id,
        available,
    })
}

fn window_palette_available(settings: &AppSettings, palette: WindowPalette) -> bool {
    window_target(settings, palette).is_some_and(|target| target.available)
}

fn safe_preview_scheme(target: Option<&str>) -> Option<sourceview5::StyleScheme> {
    let manager = sourceview5::StyleSchemeManager::default();
    target
        .and_then(|scheme_id| manager.scheme(scheme_id))
        .or_else(|| manager.scheme(crate::palette_engine::ADWAITA_LIGHT_SCHEME))
}

fn draw_chrome_preview(context: &cairo::Context, width: f64, height: f64, colors: &ChromeColors) {
    if width <= 1.0 || height <= 1.0 {
        return;
    }
    add_rounded_rectangle(context, 0.5, 0.5, width - 1.0, height - 1.0, PREVIEW_RADIUS);
    set_source_rgba(context, &colors.window_bg);
    if context.fill().is_err() {
        return;
    }
    draw_rect(context, 0.5, 0.5, width - 1.0, 20.0, &colors.headerbar_bg);
    draw_rect(context, width - 20.0, 7.0, 9.0, 7.0, &colors.accent_bg);
    draw_rect(context, width - 17.0, 9.5, 3.0, 2.0, &colors.accent_fg);
    draw_rect(context, 0.5, 20.5, width - 1.0, 13.0, &colors.tabbar_bg);
    draw_rect(context, 9.0, 23.0, 38.0, 10.0, &colors.active_tab_bg);
    draw_rect(context, 50.0, 24.0, 30.0, 8.0, &colors.hover_tab_bg);
    draw_rect(context, 10.0, 32.0, 36.0, 2.0, &colors.active_tab_indicator);
    draw_rect(context, 0.5, 34.0, 36.0, height - 34.5, &colors.sidebar_bg);
    draw_rect(context, 36.0, 34.0, 1.0, height - 34.5, &colors.border);
    draw_rect(context, 42.0, 44.0, width - 54.0, 4.0, &colors.border);
    draw_rect(context, 42.0, 56.0, width - 74.0, 4.0, &colors.border);
    draw_rect(context, 42.0, 68.0, width - 64.0, 4.0, &colors.border);
    draw_rect(
        context,
        0.5,
        height - 13.0,
        width - 1.0,
        12.5,
        &colors.statusbar_bg,
    );
}

fn draw_rect(
    context: &cairo::Context,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    color: &gtk4::gdk::RGBA,
) {
    context.rectangle(left, top, width, height);
    set_source_rgba(context, color);
    let _result = context.fill();
}

fn add_rounded_rectangle(
    context: &cairo::Context,
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    radius: f64,
) {
    let right = left + width;
    let bottom = top + height;
    context.new_sub_path();
    context.arc(right - radius, top + radius, radius, -FRAC_PI_2, 0.0);
    context.arc(right - radius, bottom - radius, radius, 0.0, FRAC_PI_2);
    context.arc(left + radius, bottom - radius, radius, FRAC_PI_2, PI);
    context.arc(left + radius, top + radius, radius, PI, PI + FRAC_PI_2);
    context.close_path();
}

fn set_source_rgba(context: &cairo::Context, color: &gtk4::gdk::RGBA) {
    context.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()),
    );
}
