use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::APP_ID;
use crate::dialogs;
use crate::editor_zoom::EditorZoomController;
use crate::error::AppError;
use crate::settings::AppSettings;
use crate::window_compare::WindowCompareController;
use crate::window_preferences::WindowPreferencesController;
use crate::window_project::WindowProjectController;
use crate::window_shell::WindowShell;
use crate::workspace::{OpenSource, Workspace, WorkspaceParts};

#[cfg(test)]
mod testing;

pub struct Window {
    shell: WindowShell,
    save_action: gio::SimpleAction,
    save_as_action: gio::SimpleAction,
    close_action: gio::SimpleAction,
    search_action: gio::SimpleAction,
    replace_action: gio::SimpleAction,
    find_next_action: gio::SimpleAction,
    find_prev_action: gio::SimpleAction,
    settings: AppSettings,
    workspace: Rc<Workspace>,
    _preferences: WindowPreferencesController,
    compare: Rc<WindowCompareController>,
    project: WindowProjectController,
    zoom: Rc<EditorZoomController>,
}

impl Window {
    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub fn new(app: &adw::Application) -> Result<Rc<Self>, AppError> {
        Self::build(app, AppSettings::new())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub fn new_for_tests(app: &adw::Application) -> Result<Rc<Self>, AppError> {
        Self::build(app, AppSettings::new_for_tests())
    }

    #[cfg(test)]
    pub(crate) fn new_with_settings_for_tests(
        app: &adw::Application,
        settings: AppSettings,
    ) -> Result<Rc<Self>, AppError> {
        Self::build(app, settings)
    }

    fn build(app: &adw::Application, settings: AppSettings) -> Result<Rc<Self>, AppError> {
        let shell = WindowShell::new(app)?;
        sourceview5::init();
        configure_runtime_icon_support(&shell.window);
        let save_action = gio::SimpleAction::new("save", None);
        let save_as_action = gio::SimpleAction::new("save-as", None);
        let close_action = gio::SimpleAction::new("close", None);
        let search_action = gio::SimpleAction::new("search", None);
        let replace_action = gio::SimpleAction::new("replace", None);
        let find_next_action = gio::SimpleAction::new("find-next", None);
        let find_prev_action = gio::SimpleAction::new("find-prev", None);
        shell.window.add_action(&save_action);
        shell.window.add_action(&save_as_action);
        shell.window.add_action(&close_action);
        shell.window.add_action(&search_action);
        shell.window.add_action(&replace_action);
        shell.window.add_action(&find_next_action);
        shell.window.add_action(&find_prev_action);

        settings.apply_theme();

        let (width, height) = settings.window_size();
        shell.window.set_default_size(width, height);

        let workspace = Workspace::new(WorkspaceParts {
            shell: &shell.window,
            toolbar_view: &shell.toolbar_view,
            title_widget: &shell.title_widget,
            toast_overlay: &shell.toast_overlay,
            workspace_box: &shell.workspace_box,
            menu_button: &shell.primary_menu_button,
            save_action: &save_action,
            save_as_action: &save_as_action,
            close_action: &close_action,
            settings: &settings,
        });
        let zoom = EditorZoomController::new(&shell.window, &workspace, &settings);
        let preferences = WindowPreferencesController::new(&shell, &settings, &workspace, &zoom);
        let compare = WindowCompareController::new(&shell.window, &workspace);
        let project = WindowProjectController::new(&shell, &settings, &workspace);

        let window = Rc::new(Self {
            shell,
            save_action,
            save_as_action,
            close_action,
            search_action,
            replace_action,
            find_next_action,
            find_prev_action,
            settings,
            workspace,
            _preferences: preferences,
            compare,
            project,
            zoom,
        });
        window.zoom.set_editor_font(&window.settings.editor_font());
        window.compare.refresh_action_state();
        window.install_callbacks();
        Ok(window)
    }

    #[must_use]
    pub fn widget(&self) -> &adw::ApplicationWindow {
        &self.shell.window
    }

    pub fn present(&self) {
        self.shell.window.present();
    }

    pub fn ensure_default_tab(self: &Rc<Self>) {
        self.workspace.ensure_default_tab();
    }

    pub fn restore_session(self: &Rc<Self>) {
        self.project.restore_before_session();
        self.workspace.restore_session();
    }

    pub fn request_new(self: &Rc<Self>) {
        self.workspace.request_new_tab();
    }

    pub fn request_open_dialog(self: &Rc<Self>) {
        self.workspace.request_open_dialog(&self.shell.window);
    }

    pub fn request_open_files(self: &Rc<Self>, files: Vec<gio::File>, source: OpenSource) {
        self.workspace.request_open_files(files, source);
    }

    pub fn request_open_folder_dialog(&self) {
        self.project.request_open_folder_dialog();
    }

    pub fn request_open_recent(self: &Rc<Self>, uri: &str) {
        self.workspace.request_open_recent(uri);
    }

    pub fn handle_application_open(&self, files: Vec<gio::File>) {
        self.project.handle_application_open(files);
    }

    pub fn request_save(self: &Rc<Self>) {
        self.workspace.request_save_selected(false);
    }

    pub fn request_save_as(self: &Rc<Self>) {
        self.workspace.request_save_selected(true);
    }

    pub fn request_close_current_tab(&self) {
        self.workspace.request_close_selected_tab();
    }

    pub fn open_search(self: &Rc<Self>, replace_mode: bool) {
        self.workspace.open_search(replace_mode);
    }

    pub fn find_next(self: &Rc<Self>) {
        self.workspace.find_next();
    }

    pub fn find_previous(self: &Rc<Self>) {
        self.workspace.find_previous();
    }

    pub fn show_preferences(&self) {
        self.shell
            .preferences_dialog
            .present(Some(&self.shell.window));
    }

    pub fn show_about(&self) {
        dialogs::show_about(&self.shell.window);
    }

    pub fn show_help(&self) {
        dialogs::show_help(&self.shell.window);
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.shell.window.connect_close_request(move |_| {
            weak.upgrade().map_or(glib::Propagation::Proceed, |window| {
                window.on_close_request()
            })
        });

        let weak = Rc::downgrade(self);
        self.save_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_save();
            }
        });

        let weak = Rc::downgrade(self);
        self.save_as_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_save_as();
            }
        });

        let weak = Rc::downgrade(self);
        self.close_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_close_current_tab();
            }
        });

        let weak = Rc::downgrade(self);
        self.search_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.open_search(false);
            }
        });

        let weak = Rc::downgrade(self);
        self.replace_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.open_search(true);
            }
        });

        let weak = Rc::downgrade(self);
        self.find_next_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.find_next();
            }
        });

        let weak = Rc::downgrade(self);
        self.find_prev_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.find_previous();
            }
        });
    }

    fn on_close_request(self: &Rc<Self>) -> glib::Propagation {
        if self.workspace.allow_window_close() {
            self.persist_window_size();
            return glib::Propagation::Proceed;
        }
        self.workspace.handle_window_close_request()
    }

    fn persist_window_size(&self) {
        self.settings
            .set_window_size(self.shell.window.width(), self.shell.window.height());
    }
}

fn configure_runtime_icon_support(window: &adw::ApplicationWindow) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let icon_theme = gtk4::IconTheme::for_display(&display);
        if let Ok(path) = std::env::var("RITEED_DEV_ICON_DIR") {
            let icon_dir = std::path::PathBuf::from(path);
            if icon_dir.is_dir() {
                let mut search_paths = icon_theme.search_path();
                search_paths.retain(|existing| existing != &icon_dir);
                search_paths.insert(0, icon_dir);
                let refs = search_paths
                    .iter()
                    .map(std::path::PathBuf::as_path)
                    .collect::<Vec<_>>();
                icon_theme.set_search_path(&refs);
            }
        }
    }
    gtk4::Window::set_default_icon_name(APP_ID);
    window.set_icon_name(Some(APP_ID));
}
