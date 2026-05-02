use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gettextrs::gettext;
use gtk4::accessible::{Property, State};
use gtk4::{AccessibleTristate, prelude::*};

use crate::palette_preview::PalettePreview;
use crate::settings::{AppSettings, EditorPalette};
use crate::workspace::Workspace;

const PALETTE_TILE_WIDTH: i32 = 132;
const PALETTE_TILE_HEIGHT: i32 = 92;

#[derive(Clone)]
pub(super) struct EditorPaletteGrid {
    state: Rc<EditorGridState>,
}

struct EditorGridState {
    settings: AppSettings,
    workspace: Weak<Workspace>,
    flow_box: gtk4::FlowBox,
    syncing: Cell<bool>,
    tiles: RefCell<Vec<EditorTile>>,
}

#[derive(Clone)]
struct EditorTile {
    palette: EditorPalette,
    child: gtk4::FlowBoxChild,
    preview: Option<PalettePreview>,
}

struct PaletteTarget {
    scheme_id: String,
    available: bool,
}

impl EditorPaletteGrid {
    pub(super) fn new(
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
        flow_box: &gtk4::FlowBox,
    ) -> Self {
        let state = Rc::new(EditorGridState {
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
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
    pub(super) fn set_palette_for_tests(&self, palette: EditorPalette) {
        self.state.activate_palette(palette);
    }

    #[cfg(test)]
    pub(super) fn selected_palette_for_ui(&self) -> EditorPalette {
        self.state.selected_palette_for_ui()
    }
}

impl EditorGridState {
    fn rebuild_tiles(&self) {
        super::clear_flow_box(&self.flow_box);
        self.tiles.borrow_mut().clear();
        for palette in palette_order() {
            let target = palette_target(palette);
            let label = palette.label();
            let (content, preview) = palette_tile_content(&target, &label);
            let child = gtk4::FlowBoxChild::builder()
                .accessible_role(gtk4::AccessibleRole::Radio)
                .focusable(true)
                .child(&content)
                .build();
            child.add_css_class("riteed-palette-tile");
            child.set_tooltip_text(Some(&label));
            self.flow_box.append(&child);
            self.tiles.borrow_mut().push(EditorTile {
                palette,
                child,
                preview,
            });
        }
    }

    fn refresh_selection(&self) {
        let selected = self.selected_palette_for_ui();
        for tile in self.tiles.borrow().iter() {
            let target = palette_target(tile.palette);
            let unavailable = !target.available;
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

    fn selected_palette_for_ui(&self) -> EditorPalette {
        let palette = self.settings.editor_palette();
        if palette_available_for_selection(palette) {
            palette
        } else {
            current_adwaita_palette()
        }
    }

    fn activate_palette(&self, palette: EditorPalette) {
        if self.syncing.get() || !palette_available_for_selection(palette) {
            return;
        }
        self.settings.set_editor_palette(palette);
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.apply_source_style_scheme_to_tabs();
        }
        self.syncing.set(true);
        self.refresh_selection();
        self.syncing.set(false);
    }
}

fn install_callbacks(state: &Rc<EditorGridState>) {
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

fn palette_order() -> [EditorPalette; 8] {
    [
        EditorPalette::AdwaitaLight,
        EditorPalette::AdwaitaDark,
        EditorPalette::ClassicLight,
        EditorPalette::ClassicDark,
        EditorPalette::Kate,
        EditorPalette::KateDark,
        EditorPalette::SolarizedLight,
        EditorPalette::SolarizedDark,
    ]
}

fn palette_tile_content(
    target: &PaletteTarget,
    label: &str,
) -> (gtk4::Widget, Option<PalettePreview>) {
    let unavailable = !target.available;
    let overlay = gtk4::Overlay::new();
    let (preview_widget, preview) = palette_preview_widget(target);
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
    (overlay.upcast::<gtk4::Widget>(), preview)
}

fn palette_preview_widget(target: &PaletteTarget) -> (gtk4::Widget, Option<PalettePreview>) {
    if let Some(scheme) = safe_preview_scheme(Some(target.scheme_id.as_str())) {
        let preview = PalettePreview::new(&scheme);
        (preview.widget(), Some(preview))
    } else {
        let label = gtk4::Label::new(Some(&gettext("Preview unavailable")));
        label.set_halign(gtk4::Align::Center);
        label.set_can_focus(false);
        label.set_can_target(false);
        label.set_size_request(PALETTE_TILE_WIDTH, PALETTE_TILE_HEIGHT);
        (label.upcast::<gtk4::Widget>(), None)
    }
}

fn update_tile_accessibility(
    child: &gtk4::FlowBoxChild,
    palette: EditorPalette,
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

fn palette_target(palette: EditorPalette) -> PaletteTarget {
    let scheme_id = palette_scheme_id(palette);
    let available = sourceview5::StyleSchemeManager::default()
        .scheme(&scheme_id)
        .is_some();
    PaletteTarget {
        scheme_id,
        available,
    }
}

fn palette_scheme_id(palette: EditorPalette) -> String {
    let dark = libadwaita::StyleManager::default().is_dark();
    crate::palette_engine::editor_scheme_id(palette, dark)
}

fn palette_available_for_selection(palette: EditorPalette) -> bool {
    palette_order().contains(&palette) && palette_target(palette).available
}

fn current_adwaita_palette() -> EditorPalette {
    if libadwaita::StyleManager::default().is_dark() {
        EditorPalette::AdwaitaDark
    } else {
        EditorPalette::AdwaitaLight
    }
}

fn safe_preview_scheme(target: Option<&str>) -> Option<sourceview5::StyleScheme> {
    let manager = sourceview5::StyleSchemeManager::default();
    target
        .and_then(|scheme_id| manager.scheme(scheme_id))
        .or_else(|| manager.scheme(crate::palette_engine::ADWAITA_LIGHT_SCHEME))
        .or_else(|| {
            manager
                .scheme_ids()
                .first()
                .and_then(|scheme_id| manager.scheme(scheme_id.as_str()))
        })
}
