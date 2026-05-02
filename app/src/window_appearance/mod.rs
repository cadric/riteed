use std::rc::Rc;
use std::sync::OnceLock;

use gettextrs::gettext;
use gtk4::accessible::Property;
use gtk4::{gdk, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::AdwDialogExt;

use crate::error::AppError;
use crate::settings::AppSettings;
use crate::workspace::Workspace;

mod editor_grid;
mod window_grid;

use editor_grid::EditorPaletteGrid;
use window_grid::WindowPaletteGrid;

const APPEARANCE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance.css";
const APPEARANCE_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance_panel.ui";

static APPEARANCE_CSS_INSTALLED: OnceLock<()> = OnceLock::new();

#[derive(Clone)]
pub struct WindowAppearanceController {
    state: Rc<AppearanceState>,
}

struct AppearanceState {
    dialog: adw::Dialog,
    dialog_child: gtk4::Widget,
    editor_grid: EditorPaletteGrid,
    window_grid: WindowPaletteGrid,
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
        let editor_flow_box: gtk4::FlowBox = builder_object(&builder, "palette_flow_box")?;
        let window_flow_box: gtk4::FlowBox = builder_object(&builder, "window_palette_flow_box")?;
        close_button.update_property(&[Property::Label(&gettext("Close Appearance Panel"))]);
        install_close_callback(&dialog, &close_button);

        let state = Rc::new(AppearanceState {
            dialog,
            dialog_child,
            editor_grid: EditorPaletteGrid::new(settings, workspace, &editor_flow_box),
            window_grid: WindowPaletteGrid::new(settings, &window_flow_box),
        });
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
    pub(crate) fn set_palette_for_tests(&self, palette: crate::settings::EditorPalette) {
        self.state.editor_grid.set_palette_for_tests(palette);
    }

    #[cfg(test)]
    pub(crate) fn selected_palette_for_tests(&self) -> crate::settings::EditorPalette {
        self.state.editor_grid.selected_palette_for_ui()
    }

    #[cfg(test)]
    pub(crate) fn set_window_palette_for_tests(&self, palette: crate::settings::WindowPalette) {
        self.state.window_grid.set_palette_for_tests(palette);
    }

    #[cfg(test)]
    pub(crate) fn selected_window_palette_for_tests(&self) -> crate::settings::WindowPalette {
        self.state.window_grid.selected_palette_for_ui()
    }
}

impl AppearanceState {
    fn sync_all(&self) {
        self.editor_grid.sync();
        self.window_grid.sync();
    }

    fn present(&self, parent: &impl IsA<gtk4::Widget>) {
        self.sync_all();
        self.dialog.present(Some(parent));
        self.queue_first_open_resize();
    }

    fn queue_first_open_resize(&self) {
        let dialog_child = self.dialog_child.clone();
        let editor_grid = self.editor_grid.clone();
        let window_grid = self.window_grid.clone();
        glib::idle_add_local_once(move || {
            dialog_child.queue_resize();
            editor_grid.queue_resize();
            window_grid.queue_resize();
        });
    }
}

fn install_close_callback(dialog: &adw::Dialog, button: &gtk4::Button) {
    let dialog = dialog.clone();
    button.connect_clicked(move |_| {
        let _closed = dialog.close();
    });
}

fn builder_object<T: IsA<gtk4::glib::Object>>(
    builder: &gtk4::Builder,
    id: &str,
) -> Result<T, AppError> {
    builder
        .object(id)
        .ok_or_else(|| AppError::Internal(format!("Missing resource object `{id}`.")))
}

fn clear_flow_box(flow_box: &gtk4::FlowBox) {
    while let Some(child) = flow_box.first_child() {
        flow_box.remove(&child);
    }
}
