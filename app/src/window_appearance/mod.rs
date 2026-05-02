use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::{gdk, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::error::AppError;
use crate::settings::AppSettings;
use crate::workspace::Workspace;

mod editor_grid;

use editor_grid::EditorPaletteGrid;

const APPEARANCE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance.css";
const APPEARANCE_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance_page.ui";

static APPEARANCE_CSS_INSTALLED: OnceLock<()> = OnceLock::new();

#[derive(Clone)]
pub struct WindowAppearanceController {
    state: Rc<AppearanceState>,
}

struct AppearanceState {
    #[cfg(test)]
    preferences_dialog: adw::PreferencesDialog,
    #[cfg(test)]
    page: adw::PreferencesPage,
    editor_grid: EditorPaletteGrid,
}

impl WindowAppearanceController {
    /// # Errors
    ///
    /// Returns an error when the resource-backed appearance page cannot be loaded.
    pub fn new(
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
        preferences_dialog: &adw::PreferencesDialog,
    ) -> Result<Self, AppError> {
        let builder = gtk4::Builder::from_resource(APPEARANCE_RESOURCE);
        let page: adw::PreferencesPage = builder_object(&builder, "appearance_page")?;
        let style_group: adw::PreferencesGroup = builder_object(&builder, "style_group")?;
        let editor_flow_box: gtk4::FlowBox = builder_object(&builder, "palette_flow_box")?;
        style_group.add(&crate::window_theme::build_selector());
        preferences_dialog.add(&page);

        let state = Rc::new(AppearanceState {
            #[cfg(test)]
            preferences_dialog: preferences_dialog.clone(),
            #[cfg(test)]
            page,
            editor_grid: EditorPaletteGrid::new(settings, workspace, &editor_flow_box),
        });
        state.sync_all();
        Ok(Self { state })
    }

    pub fn sync(&self) {
        self.state.sync_all();
    }

    pub fn queue_resize(&self) {
        self.state.queue_resize();
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
}

impl AppearanceState {
    fn sync_all(&self) {
        self.editor_grid.sync();
    }

    #[cfg(test)]
    fn present(&self, parent: &impl IsA<gtk4::Widget>) {
        self.sync_all();
        self.preferences_dialog.set_visible_page(&self.page);
        self.preferences_dialog.present(Some(parent));
        self.queue_resize();
    }

    fn queue_resize(&self) {
        let editor_grid = self.editor_grid.clone();
        glib::idle_add_local_once(move || {
            editor_grid.queue_resize();
        });
    }
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
