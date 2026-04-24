use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::OnceLock;

use gettextrs::{gettext, pgettext};
use gtk4::accessible::{Property, State};
use gtk4::{gdk, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::AdwDialogExt;
use sourceview5::prelude::*;

use crate::error::AppError;
use crate::settings::{AppSettings, EditorPalette, ThemePreference};
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

const APPEARANCE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance.css";
const APPEARANCE_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance_panel.ui";
const PALETTE_COLUMNS: i32 = 2;

static APPEARANCE_CSS_INSTALLED: OnceLock<()> = OnceLock::new();

pub struct WindowAppearanceController {
    state: Rc<AppearanceState>,
}

struct AppearanceState {
    settings: AppSettings,
    workspace: Weak<Workspace>,
    syncing: Cell<bool>,
    dialog: adw::Dialog,
    theme_buttons: RefCell<Vec<ThemeButton>>,
    palette_tiles: RefCell<Vec<PaletteTile>>,
    highlight_row: adw::SwitchRow,
}

#[derive(Clone)]
struct ThemeButton {
    theme: ThemePreference,
    button: gtk4::ToggleButton,
}

#[derive(Clone)]
struct PaletteTile {
    palette: EditorPalette,
    button: gtk4::ToggleButton,
}

struct PaletteTarget {
    scheme_id: String,
    available: bool,
}

impl WindowAppearanceController {
    /// # Errors
    ///
    /// Returns an error when the resource-backed appearance panel cannot be loaded.
    pub fn new(
        shell: &WindowShell,
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
    ) -> Result<Self, AppError> {
        let builder = gtk4::Builder::from_resource(APPEARANCE_RESOURCE);
        let dialog: adw::Dialog = builder_object(&builder, "appearance_dialog")?;
        let close_button: gtk4::Button = builder_object(&builder, "appearance_close_button")?;
        let app_box: gtk4::Box = builder_object(&builder, "app_appearance_box")?;
        let palette_grid: gtk4::Grid = builder_object(&builder, "palette_grid")?;
        let highlight_row: adw::SwitchRow = builder_object(&builder, "highlight_current_line_row")?;

        close_button.update_property(&[Property::Label(&gettext("Close Appearance Panel"))]);
        install_close_callback(&dialog, &close_button);

        let (app_group, theme_buttons) = build_app_appearance_group();
        app_box.append(&app_group);

        let state = Rc::new(AppearanceState {
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
            syncing: Cell::new(false),
            dialog,
            theme_buttons: RefCell::new(theme_buttons),
            palette_tiles: RefCell::new(Vec::new()),
            highlight_row,
        });

        build_palette_grid(&state, &palette_grid);
        install_callbacks(&state);
        install_button_callback(&state, &shell.appearance_button);
        state.sync_all();
        Ok(Self { state })
    }

    pub fn sync(&self) {
        self.state.sync_all();
    }

    #[cfg(test)]
    pub(crate) fn present_for_tests(&self, parent: &impl IsA<gtk4::Widget>) {
        self.state.sync_all();
        self.state.dialog.present(Some(parent));
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
    pub(crate) fn set_highlight_for_tests(&self, enabled: bool) {
        self.state.activate_highlight(enabled);
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
        self.highlight_row
            .set_active(self.settings.highlight_current_line());
        self.refresh_palette_tiles();
        self.syncing.set(false);
    }

    fn refresh_palette_tiles(&self) {
        let selected = self.selected_palette_for_ui();
        for tile in self.palette_tiles.borrow().iter() {
            let target = palette_target(tile.palette);
            let unavailable = target.as_ref().is_none_or(|target| !target.available);
            tile.button.set_child(Some(&palette_tile_content(
                tile.palette,
                target.as_ref(),
                unavailable,
            )));
            tile.button.set_sensitive(!unavailable);
            tile.button.set_active(tile.palette == selected);
            update_tile_accessibility(
                &tile.button,
                tile.palette,
                unavailable,
                tile.palette == selected,
            );
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

    fn activate_highlight(&self, enabled: bool) {
        if self.syncing.get() {
            return;
        }
        self.settings.set_highlight_current_line(enabled);
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.apply_current_line_highlight_to_tabs();
        }
        self.sync_all();
    }

    fn apply_source_styles(&self) {
        if let Some(workspace) = self.workspace.upgrade() {
            workspace.apply_source_style_scheme_to_tabs();
        }
    }
}

fn build_app_appearance_group() -> (gtk4::Box, Vec<ThemeButton>) {
    let group = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Horizontal)
        .hexpand(true)
        .build();
    group.add_css_class("linked");
    group.set_accessible_role(gtk4::AccessibleRole::RadioGroup);

    let mut buttons = Vec::new();
    let mut radio_group: Option<gtk4::ToggleButton> = None;
    for theme in [
        ThemePreference::System,
        ThemePreference::Light,
        ThemePreference::Dark,
    ] {
        let label = theme_label(theme);
        let button = gtk4::ToggleButton::builder()
            .accessible_role(gtk4::AccessibleRole::Radio)
            .focusable(true)
            .hexpand(true)
            .label(&label)
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

fn build_palette_grid(state: &Rc<AppearanceState>, grid: &gtk4::Grid) {
    grid.set_accessible_role(gtk4::AccessibleRole::RadioGroup);
    let mut group: Option<gtk4::ToggleButton> = None;
    for (index, palette) in palette_order().iter().copied().enumerate() {
        let button = gtk4::ToggleButton::builder()
            .accessible_role(gtk4::AccessibleRole::Radio)
            .focusable(true)
            .build();
        button.add_css_class("riteed-palette-tile");
        if let Some(group) = group.as_ref() {
            button.set_group(Some(group));
        } else {
            group = Some(button.clone());
        }
        let grid_index = i32::try_from(index).ok().map_or(0, |index| index);
        let row = grid_index / PALETTE_COLUMNS;
        let column = grid_index % PALETTE_COLUMNS;
        grid.attach(&button, column, row, 1, 1);
        state.palette_tiles.borrow_mut().push(PaletteTile {
            palette,
            button: button.clone(),
        });
        let weak = Rc::downgrade(state);
        button.connect_toggled(move |button| {
            if !button.is_active() {
                return;
            }
            if let Some(state) = weak.upgrade() {
                state.activate_palette(palette);
            }
        });
    }
}

fn install_callbacks(state: &Rc<AppearanceState>) {
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
    state.highlight_row.connect_active_notify(move |row| {
        if let Some(state) = weak.upgrade() {
            state.activate_highlight(row.is_active());
        }
    });

    let weak = Rc::downgrade(state);
    let _handler = adw::StyleManager::default().connect_dark_notify(move |_| {
        if let Some(state) = weak.upgrade() {
            state.syncing.set(true);
            state.refresh_palette_tiles();
            state.syncing.set(false);
        }
    });
}

fn install_button_callback(state: &Rc<AppearanceState>, button: &gtk4::Button) {
    let weak = Rc::downgrade(state);
    button.connect_clicked(move |button| {
        if let Some(state) = weak.upgrade() {
            state.sync_all();
            state.dialog.present(Some(button));
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
        EditorPalette::AdwaitaLight,
        EditorPalette::AdwaitaDark,
        EditorPalette::Kate,
        EditorPalette::KateDark,
        EditorPalette::SolarizedLight,
        EditorPalette::SolarizedDark,
        EditorPalette::Classic,
    ]
}

fn palette_tile_content(
    palette: EditorPalette,
    target: Option<&PaletteTarget>,
    unavailable: bool,
) -> gtk4::Box {
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(6)
        .build();
    let overlay = gtk4::Overlay::new();
    overlay.set_child(Some(&palette_preview_widget(target)));
    if unavailable {
        let label = gtk4::Label::new(Some(&gettext("Unavailable")));
        label.add_css_class("riteed-palette-unavailable");
        label.set_halign(gtk4::Align::Center);
        label.set_valign(gtk4::Align::Center);
        overlay.add_overlay(&label);
    }
    content.append(&overlay);
    let label = gtk4::Label::new(Some(&palette.label()));
    label.set_wrap(true);
    content.append(&label);
    content
}

fn palette_preview_widget(target: Option<&PaletteTarget>) -> gtk4::Widget {
    if let Some(scheme) = safe_preview_scheme(target.map(|target| target.scheme_id.as_str())) {
        let buffer = sourceview5::Buffer::builder().enable_undo(false).build();
        buffer.set_style_scheme(Some(&scheme));
        buffer.set_text("fn main() {\n    text();\n}");
        let view = sourceview5::View::with_buffer(&buffer);
        view.add_css_class("riteed-palette-preview");
        view.set_can_focus(false);
        view.set_cursor_visible(false);
        view.set_editable(false);
        view.set_focusable(false);
        view.set_left_margin(8);
        view.set_monospace(true);
        view.set_right_margin(8);
        view.set_show_line_numbers(false);
        view.set_size_request(132, 58);
        view.set_top_margin(6);
        view.upcast::<gtk4::Widget>()
    } else {
        let label = gtk4::Label::new(Some(&gettext("Preview unavailable")));
        label.set_size_request(132, 58);
        label.upcast::<gtk4::Widget>()
    }
}

fn update_tile_accessibility(
    button: &gtk4::ToggleButton,
    palette: EditorPalette,
    unavailable: bool,
    selected: bool,
) {
    let label = palette.label();
    button.update_property(&[Property::Label(&label)]);
    button.update_state(&[
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
