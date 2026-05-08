use std::rc::Rc;
use std::sync::OnceLock;

use gtk4::{gdk, glib, prelude::*};
use libadwaita::prelude::*;

use crate::settings::AppSettings;
use crate::window_shell::WindowShell;
use crate::workspace::Workspace;

mod editor_grid;

use editor_grid::EditorPaletteGrid;

const APPEARANCE_CSS_RESOURCE: &str = "/io/github/cadric/Riteed/ui/appearance.css";

static APPEARANCE_CSS_INSTALLED: OnceLock<()> = OnceLock::new();

#[derive(Clone)]
pub struct WindowAppearanceController {
    state: Rc<AppearanceState>,
}

struct AppearanceState {
    #[cfg(test)]
    preferences_dialog: libadwaita::PreferencesDialog,
    #[cfg(test)]
    page: libadwaita::PreferencesPage,
    editor_grid: EditorPaletteGrid,
}

impl WindowAppearanceController {
    pub fn new(settings: &AppSettings, workspace: &Rc<Workspace>, shell: &WindowShell) -> Self {
        shell
            .style_group
            .add(&crate::window_theme::build_selector());
        let state = Rc::new(AppearanceState {
            #[cfg(test)]
            preferences_dialog: shell.preferences_dialog.clone(),
            #[cfg(test)]
            page: shell.appearance_page.clone(),
            editor_grid: EditorPaletteGrid::new(settings, workspace, &shell.palette_flow_box),
        });
        state.sync_all();
        Self { state }
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

fn clear_flow_box(flow_box: &gtk4::FlowBox) {
    while let Some(child) = flow_box.first_child() {
        flow_box.remove(&child);
    }
}
