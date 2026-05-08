use std::cell::Cell;
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::accessible::Property;
use gtk4::{gdk, gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::app_chrome::{AppChromeController, ChromeObserver};
use crate::dialogs;
use crate::document_tools::DocumentToolsController;
use crate::editor_zoom::EditorZoomController;
use crate::error::AppError;
use crate::find_in_files::FindInFilesController;
use crate::settings::AppSettings;
use crate::sidebar_host::SidebarHost;
use crate::source_control::SourceControlController;
use crate::window_appearance::WindowAppearanceController;
use crate::window_compare::WindowCompareController;
use crate::window_preferences::WindowPreferencesController;
use crate::window_project::WindowProjectController;
use crate::window_shell::WindowShell;
use crate::window_support::{
    configure_open_button, create_window_actions, install_chrome_observer,
};
use crate::workspace::{OpenSource, Workspace, WorkspaceParts};

mod actions;
mod git_actions;
mod search_coordinator;
mod sidebar_wiring;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod testing_compare;

#[derive(Clone, Copy, Debug)]
pub struct WindowInit {
    pub persist_session: bool,
    pub restore_project: bool,
}

impl WindowInit {
    #[must_use]
    pub const fn primary() -> Self {
        Self {
            persist_session: true,
            restore_project: true,
        }
    }

    #[must_use]
    pub const fn secondary() -> Self {
        Self {
            persist_session: false,
            restore_project: false,
        }
    }
}

pub struct Window {
    shell: WindowShell,
    save_action: gio::SimpleAction,
    save_as_action: gio::SimpleAction,
    close_action: gio::SimpleAction,
    recent_files_action: gio::SimpleAction,
    search_action: gio::SimpleAction,
    replace_action: gio::SimpleAction,
    find_in_files_action: gio::SimpleAction,
    find_next_action: gio::SimpleAction,
    find_prev_action: gio::SimpleAction,
    fullscreen_action: gio::SimpleAction,
    #[cfg(test)]
    theme_action: gio::SimpleAction,
    settings: AppSettings,
    workspace: Rc<Workspace>,
    appearance: WindowAppearanceController,
    _chrome_observer: Option<ChromeObserver>,
    _preferences: WindowPreferencesController,
    compare: Rc<WindowCompareController>,
    document_tools: Rc<DocumentToolsController>,
    project: WindowProjectController,
    sidebar_host: Rc<SidebarHost>,
    find_in_files: Rc<FindInFilesController>,
    source_control: SourceControlController,
    _git_actions: Rc<git_actions::WindowGitActions>,
    zoom: Rc<EditorZoomController>,
    last_non_fullscreen_size: Cell<(i32, i32)>,
}

impl Window {
    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub(crate) fn new(
        app: &adw::Application,
        settings: &AppSettings,
        chrome: Option<&AppChromeController>,
    ) -> Result<Rc<Self>, AppError> {
        Self::build(app, settings.clone(), chrome, WindowInit::primary())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub(crate) fn new_secondary(
        app: &adw::Application,
        settings: &AppSettings,
        chrome: Option<&AppChromeController>,
    ) -> Result<Rc<Self>, AppError> {
        Self::build(app, settings.clone(), chrome, WindowInit::secondary())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    #[cfg(test)]
    pub(crate) fn new_for_tests(
        app: &adw::Application,
        settings: &AppSettings,
        chrome: Option<&AppChromeController>,
    ) -> Result<Rc<Self>, AppError> {
        crate::window_support::install_sourceview_for_tests();
        Self::build(app, settings.clone(), chrome, WindowInit::primary())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    #[cfg(test)]
    pub(crate) fn new_secondary_for_tests(
        app: &adw::Application,
        settings: &AppSettings,
        chrome: Option<&AppChromeController>,
    ) -> Result<Rc<Self>, AppError> {
        crate::window_support::install_sourceview_for_tests();
        Self::build(app, settings.clone(), chrome, WindowInit::secondary())
    }

    #[cfg(test)]
    pub(crate) fn new_with_settings_for_tests(
        app: &adw::Application,
        settings: AppSettings,
    ) -> Result<Rc<Self>, AppError> {
        crate::window_support::install_sourceview_for_tests();
        Self::build(app, settings, None, WindowInit::primary())
    }

    fn build(
        app: &adw::Application,
        settings: AppSettings,
        chrome: Option<&AppChromeController>,
        init: WindowInit,
    ) -> Result<Rc<Self>, AppError> {
        let shell = WindowShell::new(app)?;
        configure_open_button(&shell);
        crate::runtime_icons::configure(&shell.window);
        let actions = create_window_actions(&shell.window, &settings);

        let (width, height) = settings.window_size();
        shell.window.set_default_size(width, height);

        let workspace = Workspace::new(WorkspaceParts {
            shell: &shell.window,
            toolbar_view: &shell.toolbar_view,
            title_widget: &shell.title_widget,
            toast_overlay: &shell.toast_overlay,
            workspace_box: &shell.workspace_box,
            menu_button: &shell.primary_menu_button,
            save_action: &actions.save,
            save_as_action: &actions.save_as,
            close_action: &actions.close,
            settings: &settings,
            persist_session: init.persist_session,
        });
        let zoom = EditorZoomController::new(&shell.window, &workspace, &settings);
        let appearance = WindowAppearanceController::new(&settings, &workspace, &shell);
        let chrome_observer = install_chrome_observer(chrome, &appearance, &workspace);
        crate::window_theme::install(
            &actions.theme,
            &settings,
            &workspace,
            &appearance,
            chrome,
            &shell.primary_menu_button,
        );
        let preferences = WindowPreferencesController::new(&shell, &settings, &workspace, &zoom);
        let compare = WindowCompareController::new(&shell.window, &workspace);
        let document_tools = DocumentToolsController::new(&shell.window, &workspace);
        let project =
            WindowProjectController::new(&shell, &settings, &workspace, init.restore_project);
        let sidebar = sidebar_wiring::install(&shell, &settings, &workspace, &project);

        let window = Rc::new(Self {
            shell,
            save_action: actions.save,
            save_as_action: actions.save_as,
            close_action: actions.close,
            recent_files_action: actions.recent_files,
            search_action: actions.search,
            replace_action: actions.replace,
            find_in_files_action: actions.find_in_files,
            find_next_action: actions.find_next,
            find_prev_action: actions.find_prev,
            fullscreen_action: actions.fullscreen,
            #[cfg(test)]
            theme_action: actions.theme,
            settings,
            workspace,
            appearance,
            _chrome_observer: chrome_observer,
            _preferences: preferences,
            compare,
            document_tools,
            project,
            sidebar_host: sidebar.sidebar_host,
            find_in_files: sidebar.find_in_files,
            source_control: sidebar.source_control,
            _git_actions: sidebar.git_actions,
            zoom,
            last_non_fullscreen_size: Cell::new((width, height)),
        });
        window.finish_initialization();
        Ok(window)
    }

    #[must_use]
    pub fn widget(&self) -> &adw::ApplicationWindow {
        &self.shell.window
    }

    pub fn present(&self) {
        self.shell.window.present();
    }

    pub(crate) fn workspace(&self) -> Rc<Workspace> {
        Rc::clone(&self.workspace)
    }

    pub(crate) fn set_tab_transfer_window_handler(
        &self,
        handler: crate::workspace::tabs::TransferWindowHandler,
    ) {
        self.workspace.set_transfer_window_handler(handler);
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
        self.open_search_with_scope(crate::editor_search::SearchScope::Document, replace_mode);
    }

    pub fn find_next(self: &Rc<Self>) {
        self.workspace.find_next();
    }

    pub fn find_previous(self: &Rc<Self>) {
        self.workspace.find_previous();
    }

    pub fn show_find_in_files(self: &Rc<Self>) {
        self.sidebar_host.set_search_results_visible(true);
        self.open_project_search();
    }

    pub fn show_preferences(&self) {
        self.appearance.sync();
        self.shell
            .preferences_dialog
            .present(Some(&self.shell.window));
        self.appearance.queue_resize();
    }

    pub fn show_about(&self) {
        dialogs::show_about(&self.shell.window);
    }

    pub fn show_help(&self) {
        dialogs::show_help(&self.shell.window);
    }

    pub fn show_recent_files(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let on_open_uri: Rc<dyn Fn(String)> = Rc::new(move |uri| {
            if let Some(window) = weak.upgrade() {
                window.request_open_recent(&uri);
            }
        });
        dialogs::show_recent_files_dialog(&self.shell.window, &self.settings, &on_open_uri);
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.shell.window.connect_close_request(move |_| {
            weak.upgrade().map_or(glib::Propagation::Proceed, |window| {
                window.on_close_request()
            })
        });

        self.install_document_callbacks();
        self.install_window_state_callbacks();
    }

    fn finish_initialization(self: &Rc<Self>) {
        self.source_control
            .set_project_root(self.project.current_root_file());
        self.find_in_files
            .set_project_root(self.project.current_root_file());
        self.zoom.set_editor_font(&self.settings.editor_font());
        self.install_accessible_labels();
        self.appearance.sync();
        self.compare.refresh_action_state();
        self.document_tools.sync_current();
        self.install_callbacks();
    }

    fn install_window_state_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.fullscreen_action.connect_activate(move |action, _| {
            let enabled = action
                .state()
                .and_then(|state| state.get::<bool>())
                .is_some_and(|state| state);
            if let Some(window) = weak.upgrade() {
                window.set_fullscreen(!enabled);
            }
        });

        let weak = Rc::downgrade(self);
        self.shell.window.connect_fullscreened_notify(move |shell| {
            if let Some(window) = weak.upgrade() {
                let fullscreened = shell.is_fullscreen();
                window
                    .fullscreen_action
                    .set_state(&fullscreened.to_variant());
                if !fullscreened {
                    window.capture_non_fullscreen_size();
                }
            }
        });

        let weak = Rc::downgrade(self);
        self.shell.window.connect_default_width_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.capture_non_fullscreen_size();
            }
        });

        let weak = Rc::downgrade(self);
        self.shell.window.connect_default_height_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.capture_non_fullscreen_size();
            }
        });

        let key_controller = gtk4::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        key_controller.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::Escape
                && modifiers.is_empty()
                && let Some(window) = weak.upgrade()
                && window.shell.window.is_fullscreen()
            {
                window.set_fullscreen(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.shell.window.add_controller(key_controller);
    }

    fn install_accessible_labels(&self) {
        self.shell
            .new_button
            .update_property(&[Property::Label(&gettext("New Tab"))]);
        self.shell
            .open_button
            .update_property(&[Property::Label(&gettext("Open Files"))]);
        self.shell
            .project_sidebar_button
            .update_property(&[Property::Label(&gettext("Project Sidebar"))]);
        self.shell
            .save_button
            .update_property(&[Property::Label(&gettext("Save Current File"))]);
        self.shell
            .primary_menu_button
            .update_property(&[Property::Label(&gettext("Main Menu"))]);
    }

    fn set_fullscreen(&self, fullscreen: bool) {
        if fullscreen {
            self.capture_non_fullscreen_size();
            self.shell.window.fullscreen();
        } else {
            self.shell.window.unfullscreen();
        }
        self.fullscreen_action.set_state(&fullscreen.to_variant());
    }

    fn capture_non_fullscreen_size(&self) {
        if self.shell.window.is_fullscreen() {
            return;
        }
        let width = self.shell.window.width();
        let height = self.shell.window.height();
        if width > 0 && height > 0 {
            self.last_non_fullscreen_size.set((width, height));
        }
    }

    fn on_close_request(self: &Rc<Self>) -> glib::Propagation {
        if self.workspace.allow_window_close() {
            self.persist_window_size();
            return glib::Propagation::Proceed;
        }
        self.workspace.handle_window_close_request()
    }

    fn persist_window_size(&self) {
        let (width, height) = if self.shell.window.is_fullscreen() {
            self.last_non_fullscreen_size.get()
        } else {
            (self.shell.window.width(), self.shell.window.height())
        };
        self.settings.set_window_size(width, height);
    }
}
