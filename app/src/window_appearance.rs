use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

use gettextrs::{gettext, pgettext};
use gtk4::accessible::{Property, State};
use gtk4::{gdk, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::AdwDialogExt;

use crate::error::AppError;
use crate::settings::{AppSettings, EditorPalette, ThemePreference};
use crate::workspace::Workspace;

const APPEARANCE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance.css";
const APPEARANCE_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance_panel.ui";
const PALETTE_TILE_WIDTH: i32 = 132;
const PALETTE_TILE_HEIGHT: i32 = 92;

static APPEARANCE_CSS_INSTALLED: OnceLock<()> = OnceLock::new();

pub struct WindowAppearanceController {
    state: Rc<AppearanceState>,
}

struct AppearanceState {
    settings: AppSettings,
    workspace: Weak<Workspace>,
    syncing: Cell<bool>,
    dialog: adw::Dialog,
    dialog_child: gtk4::Widget,
    palette_flow_box: gtk4::FlowBox,
    theme_buttons: RefCell<Vec<ThemeButton>>,
    palette_tiles: RefCell<Vec<PaletteTile>>,
}

#[derive(Clone)]
struct ThemeButton {
    theme: ThemePreference,
    button: gtk4::ToggleButton,
}

#[derive(Clone)]
struct PaletteTile {
    palette: EditorPalette,
    child: gtk4::FlowBoxChild,
    preview: Option<sourceview5::StyleSchemePreview>,
}

struct PaletteTarget {
    scheme_id: String,
    available: bool,
}

impl WindowAppearanceController {
    /// # Errors
    ///
    /// Returns an error when the resource-backed appearance panel cannot be loaded.
    pub fn new(settings: &AppSettings, workspace: &Rc<Workspace>) -> Result<Self, AppError> {
        let builder = gtk4::Builder::from_resource(APPEARANCE_RESOURCE);
        let dialog: adw::Dialog = builder_object(&builder, "appearance_dialog")?;
        let dialog_child = dialog
            .child()
            .ok_or_else(|| AppError::Internal(String::from("Missing appearance dialog child.")))?;
        let close_button: gtk4::Button = builder_object(&builder, "appearance_close_button")?;
        let app_box: gtk4::Box = builder_object(&builder, "app_appearance_box")?;
        let palette_flow_box: gtk4::FlowBox = builder_object(&builder, "palette_flow_box")?;
        close_button.update_property(&[Property::Label(&gettext("Close Appearance Panel"))]);
        install_close_callback(&dialog, &close_button);

        let (app_group, theme_buttons) = build_app_appearance_group();
        app_box.append(&app_group);

        let state = Rc::new(AppearanceState {
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
            syncing: Cell::new(false),
            dialog,
            dialog_child,
            palette_flow_box: palette_flow_box.clone(),
            theme_buttons: RefCell::new(theme_buttons),
            palette_tiles: RefCell::new(Vec::new()),
        });

        build_palette_flow_box(&state, &palette_flow_box);
        install_callbacks(&state, &palette_flow_box);
        state.sync_all();
        Ok(Self { state })
    }

    pub fn sync(&self) {
        self.state.sync_all();
    }

    pub fn present(&self, parent: &impl IsA<gtk4::Widget>) {
        self.state.present(parent);
    }

    #[cfg(test)]
    pub(crate) fn present_for_tests(&self, parent: &impl IsA<gtk4::Widget>) {
        self.state.present(parent);
    }

    pub fn install_css(display: &gdk::Display) {
        APPEARANCE_CSS_INSTALLED.get_or_init(|| {
            let provider = gtk4::CssProvider::new();
            provider.load_from_resource(APPEARANCE_CSS_RESOURCE);
            gtk4::style_context_add_provider_for_display(
                display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        });
    }

    #[cfg(test)]
    pub(crate) fn sync_for_tests(&self) {
        self.state.sync_all();
    }

    #[cfg(test)]
    pub(crate) fn set_theme_for_tests(&self, theme: ThemePreference) {
        self.state.activate_theme(theme);
    }

    #[cfg(test)]
    pub(crate) fn set_palette_for_tests(&self, palette: EditorPalette) {
        self.state.activate_palette(palette);
    }

    #[cfg(test)]
    pub(crate) fn selected_palette_for_tests(&self) -> EditorPalette {
        self.state.selected_palette_for_ui()
    }
}

impl AppearanceState {
    fn sync_all(&self) {
        self.syncing.set(true);
        let theme = self.settings.theme();
        for theme_button in self.theme_buttons.borrow().iter() {
            let selected = theme_button.theme == theme;
            theme_button.button.set_active(selected);
            update_theme_accessibility(&theme_button.button, theme_button.theme, selected);
        }
        self.refresh_palette_tiles();
        self.queue_palette_preview_resize();
        self.syncing.set(false);
    }

    fn refresh_palette_tiles(&self) {
        let selected = self.selected_palette_for_ui();
        for tile in self.palette_tiles.borrow().iter() {
            let target = palette_target(tile.palette);
            let unavailable = target.as_ref().is_none_or(|target| !target.available);
            let is_selected = tile.palette == selected;
            tile.child.set_sensitive(!unavailable);
            if let Some(preview) = tile.preview.as_ref() {
                preview.set_selected(is_selected);
            }
            update_tile_accessibility(
                tile.child.upcast_ref::<gtk4::Widget>(),
                tile.palette,
                unavailable,
                is_selected,
            );
        }
        if let Some(parent) = self
            .palette_tiles
            .borrow()
            .first()
            .and_then(|tile| tile.child.parent())
            .and_then(|parent| parent.downcast::<gtk4::FlowBox>().ok())
        {
            if let Some(active_child) = self
                .palette_tiles
                .borrow()
                .iter()
                .find(|tile| tile.palette == selected)
                .map(|tile| tile.child.clone())
            {
                parent.select_child(&active_child);
            } else {
                parent.unselect_all();
            }
        }
    }

    fn queue_palette_preview_resize(&self) {
        for tile in self.palette_tiles.borrow().iter() {
            if let Some(preview) = tile.preview.as_ref() {
                preview.queue_resize();
            }
        }
    }

    fn selected_palette_for_ui(&self) -> EditorPalette {
        let palette = self.settings.editor_palette();
        if palette_available_for_selection(palette) {
            palette
        } else {
            EditorPalette::FollowSystem
        }
    }

    fn activate_theme(&self, theme: ThemePreference) {
        if self.syncing.get() {
            return;
        }
        self.settings.set_theme(theme);
        self.settings.apply_theme();
        self.apply_source_styles();
        self.sync_all();
    }

    fn activate_palette(&self, palette: EditorPalette) {
        if self.syncing.get() || !palette_available_for_selection(palette) {
            return;
        }
        self.settings.set_editor_palette(palette);
        self.apply_source_styles();
        self.sync_all();
    }

    fn apply_source_styles(&self) {
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.apply_source_style_scheme_to_tabs();
        }
    }

    fn present(&self, parent: &impl IsA<gtk4::Widget>) {
        self.sync_all();
        self.dialog.present(Some(parent));
        self.queue_first_open_resize();
    }

    fn queue_first_open_resize(&self) {
        let dialog_child = self.dialog_child.clone();
        let palette_flow_box = self.palette_flow_box.clone();
        let tiles = self.palette_tiles.borrow().clone();
        glib::idle_add_local_once(move || {
            dialog_child.queue_resize();
            palette_flow_box.queue_resize();
            for tile in &tiles {
                tile.child.queue_resize();
                if let Some(preview) = tile.preview.as_ref() {
                    preview.queue_resize();
                }
            }
        });
    }
}

fn build_app_appearance_group() -> (gtk4::Box, Vec<ThemeButton>) {
    let group = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .hexpand(true)
        .build();
    group.add_css_class("linked");
    group.add_css_class("riteed-app-appearance");
    group.set_accessible_role(gtk4::AccessibleRole::RadioGroup);

    let mut buttons = Vec::new();
    let mut radio_group: Option<gtk4::ToggleButton> = None;
    for theme in [
        ThemePreference::System,
        ThemePreference::Light,
        ThemePreference::Dark,
    ] {
        let button = gtk4::ToggleButton::builder()
            .accessible_role(gtk4::AccessibleRole::Radio)
            .focusable(true)
            .hexpand(true)
            .child(&theme_button_content(theme))
            .build();
        if let Some(radio_group) = radio_group.as_ref() {
            button.set_group(Some(radio_group));
        } else {
            radio_group = Some(button.clone());
        }
        group.append(&button);
        buttons.push(ThemeButton { theme, button });
    }
    (group, buttons)
}

fn theme_button_content(theme: ThemePreference) -> gtk4::Box {
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .spacing(4)
        .halign(gtk4::Align::Center)
        .build();
    let icon = gtk4::Image::from_icon_name(theme_icon_name(theme));
    let label = gtk4::Label::new(Some(&theme_label(theme)));
    content.append(&icon);
    content.append(&label);
    content
}

fn theme_icon_name(theme: ThemePreference) -> &'static str {
    match theme {
        ThemePreference::System => "display-brightness-symbolic",
        ThemePreference::Light => "weather-clear-symbolic",
        ThemePreference::Dark => "weather-clear-night-symbolic",
    }
}

fn build_palette_flow_box(state: &Rc<AppearanceState>, flow_box: &gtk4::FlowBox) {
    for palette in palette_order() {
        let target = palette_target(palette);
        let (content, preview) = palette_tile_content(palette, target.as_ref());
        let child = gtk4::FlowBoxChild::builder()
            .accessible_role(gtk4::AccessibleRole::Radio)
            .focusable(true)
            .child(&content)
            .build();
        child.add_css_class("riteed-palette-tile");
        flow_box.append(&child);
        state.palette_tiles.borrow_mut().push(PaletteTile {
            palette,
            child,
            preview,
        });
    }
}

fn install_callbacks(state: &Rc<AppearanceState>, flow_box: &gtk4::FlowBox) {
    let weak = Rc::downgrade(state);
    flow_box.connect_child_activated(move |_, child| {
        if let Some(state) = weak.upgrade() {
            if state.syncing.get() {
                return;
            }
            let palette = state
                .palette_tiles
                .borrow()
                .iter()
                .find(|tile| tile.child == *child)
                .map(|tile| tile.palette);
            if let Some(palette) = palette {
                state.activate_palette(palette);
            }
        }
    });

    let weak = Rc::downgrade(state);
    flow_box.connect_map(move |flow_box| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        // GtkSourceStyleSchemePreview can adjust its preferred size on first map without
        // queueing a resize, which makes the grid start oversized until the next interaction.
        state.queue_palette_preview_resize();
        flow_box.queue_resize();
    });

    for theme_button in state.theme_buttons.borrow().iter() {
        let weak = Rc::downgrade(state);
        let theme = theme_button.theme;
        theme_button.button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            if let Some(state) = weak.upgrade() {
                state.activate_theme(theme);
            }
        });
    }

    let weak = Rc::downgrade(state);
    let _handler = adw::StyleManager::default().connect_dark_notify(move |_| {
        if let Some(state) = weak.upgrade() {
            state.syncing.set(true);
            state.refresh_palette_tiles();
            state.syncing.set(false);
        }
    });
}

fn install_close_callback(dialog: &adw::Dialog, button: &gtk4::Button) {
    let dialog = dialog.clone();
    button.connect_clicked(move |_| {
        let _closed = dialog.close();
    });
}

fn theme_label(theme: ThemePreference) -> String {
    match theme {
        ThemePreference::System => pgettext("app appearance", "Follow System"),
        ThemePreference::Light => pgettext("app appearance", "Light"),
        ThemePreference::Dark => pgettext("app appearance", "Dark"),
    }
}

fn update_theme_accessibility(button: &gtk4::ToggleButton, theme: ThemePreference, selected: bool) {
    let label = theme_label(theme);
    button.update_property(&[Property::Label(&label)]);
    button.update_state(&[
        State::Selected(Some(selected)),
        State::Checked(if selected {
            gtk4::AccessibleTristate::True
        } else {
            gtk4::AccessibleTristate::False
        }),
    ]);
}

fn palette_order() -> [EditorPalette; 8] {
    [
        EditorPalette::FollowSystem,
        EditorPalette::Classic,
        EditorPalette::AdwaitaLight,
        EditorPalette::AdwaitaDark,
        EditorPalette::Kate,
        EditorPalette::KateDark,
        EditorPalette::SolarizedLight,
        EditorPalette::SolarizedDark,
    ]
}

fn palette_tile_content(
    palette: EditorPalette,
    target: Option<&PaletteTarget>,
) -> (gtk4::Box, Option<sourceview5::StyleSchemePreview>) {
    let unavailable = target.is_none_or(|target| !target.available);
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(4)
        .halign(gtk4::Align::Center)
        .build();
    let overlay = gtk4::Overlay::new();
    let (preview_widget, preview) = palette_preview_widget(target);
    overlay.set_child(Some(&preview_widget));
    if unavailable {
        let label = gtk4::Label::new(Some(&gettext("Unavailable")));
        label.add_css_class("riteed-palette-unavailable");
        label.set_halign(gtk4::Align::Center);
        label.set_valign(gtk4::Align::Center);
        overlay.add_overlay(&label);
    }
    if palette == EditorPalette::FollowSystem {
        let badge = gtk4::Image::from_icon_name("display-brightness-symbolic");
        badge.add_css_class("riteed-palette-adaptive-badge");
        badge.set_halign(gtk4::Align::End);
        badge.set_valign(gtk4::Align::Start);
        badge.set_pixel_size(12);
        overlay.add_overlay(&badge);
    }
    content.append(&overlay);
    let label = gtk4::Label::new(Some(&palette.label()));
    label.add_css_class("caption");
    label.set_wrap(true);
    label.set_justify(gtk4::Justification::Center);
    content.append(&label);
    (content, preview)
}

fn palette_preview_widget(
    target: Option<&PaletteTarget>,
) -> (gtk4::Widget, Option<sourceview5::StyleSchemePreview>) {
    if let Some(scheme) = safe_preview_scheme(target.map(|target| target.scheme_id.as_str())) {
        let preview = sourceview5::StyleSchemePreview::new(&scheme);
        preview.add_css_class("riteed-palette-preview");
        preview.set_can_focus(false);
        preview.set_halign(gtk4::Align::Center);
        preview.set_valign(gtk4::Align::Center);
        preview.set_size_request(PALETTE_TILE_WIDTH, PALETTE_TILE_HEIGHT);
        (preview.clone().upcast::<gtk4::Widget>(), Some(preview))
    } else {
        let label = gtk4::Label::new(Some(&gettext("Preview unavailable")));
        label.set_halign(gtk4::Align::Center);
        label.set_size_request(PALETTE_TILE_WIDTH, PALETTE_TILE_HEIGHT);
        (label.upcast::<gtk4::Widget>(), None)
    }
}

fn update_tile_accessibility(
    widget: &gtk4::Widget,
    palette: EditorPalette,
    unavailable: bool,
    selected: bool,
) {
    let label = palette.label();
    widget.update_property(&[Property::Label(&label)]);
    widget.update_state(&[
        State::Selected(Some(selected)),
        State::Disabled(unavailable),
        State::Checked(if selected {
            gtk4::AccessibleTristate::True
        } else {
            gtk4::AccessibleTristate::False
        }),
    ]);
}

fn palette_target(palette: EditorPalette) -> Option<PaletteTarget> {
    let scheme_id = palette_scheme_id(palette)?;
    let available = sourceview5::StyleSchemeManager::default()
        .scheme(&scheme_id)
        .is_some();
    Some(PaletteTarget {
        scheme_id,
        available,
    })
}

fn palette_scheme_id(palette: EditorPalette) -> Option<String> {
    palette.scheme_id().map_or_else(
        || {
            let dark = adw::StyleManager::default().is_dark();
            Some(String::from(if dark { "Adwaita-dark" } else { "Adwaita" }))
        },
        |scheme_id| Some(String::from(scheme_id)),
    )
}

fn palette_available_for_selection(palette: EditorPalette) -> bool {
    palette_target(palette).is_some_and(|target| target.available)
}

fn safe_preview_scheme(target: Option<&str>) -> Option<sourceview5::StyleScheme> {
    let manager = sourceview5::StyleSchemeManager::default();
    target
        .and_then(|scheme_id| manager.scheme(scheme_id))
        .or_else(|| manager.scheme("Adwaita"))
        .or_else(|| {
            manager
                .scheme_ids()
                .first()
                .and_then(|scheme_id| manager.scheme(scheme_id.as_str()))
        })
}

fn builder_object<T: IsA<gtk4::glib::Object>>(
    builder: &gtk4::Builder,
    id: &str,
) -> Result<T, AppError> {
    builder
        .object(id)
        .ok_or_else(|| AppError::Internal(format!("Missing resource object `{id}`.")))
}
