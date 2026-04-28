use std::cell::Cell;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{gdk, gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::APP_ID;
use crate::dialogs;
use crate::editor_zoom::EditorZoomController;
use crate::error::AppError;
use crate::settings::AppSettings;
use crate::sidebar_host::SidebarHost;
use crate::source_control::SourceControlController;
use crate::window_appearance::WindowAppearanceController;
use crate::window_compare::WindowCompareController;
use crate::window_preferences::WindowPreferencesController;
use crate::window_project::WindowProjectController;
use crate::window_shell::WindowShell;
use crate::workspace::{OpenSource, Workspace, WorkspaceParts};

#[cfg(test)]
mod testing;

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
    find_next_action: gio::SimpleAction,
    find_prev_action: gio::SimpleAction,
    appearance_action: gio::SimpleAction,
    fullscreen_action: gio::SimpleAction,
    settings: AppSettings,
    workspace: Rc<Workspace>,
    appearance: WindowAppearanceController,
    _preferences: WindowPreferencesController,
    compare: Rc<WindowCompareController>,
    project: WindowProjectController,
    #[cfg(test)]
    sidebar_host: SidebarHost,
    #[cfg(not(test))]
    _sidebar_host: SidebarHost,
    source_control: SourceControlController,
    zoom: Rc<EditorZoomController>,
    last_non_fullscreen_size: Cell<(i32, i32)>,
}

impl Window {
    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub fn new(app: &adw::Application) -> Result<Rc<Self>, AppError> {
        Self::build(app, AppSettings::new(), WindowInit::primary())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub fn new_secondary(app: &adw::Application) -> Result<Rc<Self>, AppError> {
        Self::build(app, AppSettings::new(), WindowInit::secondary())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub fn new_for_tests(app: &adw::Application) -> Result<Rc<Self>, AppError> {
        Self::build(app, AppSettings::new_for_tests(), WindowInit::primary())
    }

    /// # Errors
    ///
    /// Returns an error when the resource-backed editor UI cannot be loaded.
    pub fn new_secondary_for_tests(app: &adw::Application) -> Result<Rc<Self>, AppError> {
        Self::build(app, AppSettings::new_for_tests(), WindowInit::secondary())
    }

    #[cfg(test)]
    pub(crate) fn new_with_settings_for_tests(
        app: &adw::Application,
        settings: AppSettings,
    ) -> Result<Rc<Self>, AppError> {
        Self::build(app, settings, WindowInit::primary())
    }

    fn build(
        app: &adw::Application,
        settings: AppSettings,
        init: WindowInit,
    ) -> Result<Rc<Self>, AppError> {
        let shell = WindowShell::new(app)?;
        configure_open_button(&shell);
        sourceview5::init();
        configure_runtime_icon_support(&shell.window);
        WindowAppearanceController::install_css(&gtk4::prelude::WidgetExt::display(&shell.window));
        let save_action = gio::SimpleAction::new("save", None);
        let save_as_action = gio::SimpleAction::new("save-as", None);
        let close_action = gio::SimpleAction::new("close", None);
        let recent_files_action = gio::SimpleAction::new("recent-files", None);
        let search_action = gio::SimpleAction::new("search", None);
        let replace_action = gio::SimpleAction::new("replace", None);
        let find_next_action = gio::SimpleAction::new("find-next", None);
        let find_prev_action = gio::SimpleAction::new("find-prev", None);
        let appearance_action = gio::SimpleAction::new("appearance", None);
        let fullscreen_action =
            gio::SimpleAction::new_stateful("fullscreen", None, &false.to_variant());
        shell.window.add_action(&save_action);
        shell.window.add_action(&save_as_action);
        shell.window.add_action(&close_action);
        shell.window.add_action(&recent_files_action);
        shell.window.add_action(&search_action);
        shell.window.add_action(&replace_action);
        shell.window.add_action(&find_next_action);
        shell.window.add_action(&find_prev_action);
        shell.window.add_action(&appearance_action);
        shell.window.add_action(&fullscreen_action);

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
            persist_session: init.persist_session,
        });
        let zoom = EditorZoomController::new(&shell.window, &workspace, &settings);
        let appearance = WindowAppearanceController::new(&settings, &workspace)?;
        let preferences = WindowPreferencesController::new(&shell, &settings, &workspace, &zoom);
        let compare = WindowCompareController::new(&shell.window, &workspace);
        let project =
            WindowProjectController::new(&shell, &settings, &workspace, init.restore_project);
        let source_control = SourceControlController::new(&shell.window, &settings, &workspace);
        let sidebar_host = SidebarHost::new(&project.sidebar_widget(), &source_control.widget());
        shell
            .project_split_view
            .set_start_child(Some(sidebar_host.widget()));
        shell
            .project_split_view
            .set_end_child(Some(&shell.workspace_box));
        project.set_root_change_handler(source_control.root_change_handler());
        source_control.set_status_handler(project.git_status_handler());
        workspace.set_save_notification_handler(source_control.save_notification_handler());

        let window = Rc::new(Self {
            shell,
            save_action,
            save_as_action,
            close_action,
            recent_files_action,
            search_action,
            replace_action,
            find_next_action,
            find_prev_action,
            appearance_action,
            fullscreen_action,
            settings,
            workspace,
            appearance,
            _preferences: preferences,
            compare,
            project,
            #[cfg(test)]
            sidebar_host,
            #[cfg(not(test))]
            _sidebar_host: sidebar_host,
            source_control,
            zoom,
            last_non_fullscreen_size: Cell::new((width, height)),
        });
        window
            .source_control
            .set_project_root(window.project.current_root_file());
        window.zoom.set_editor_font(&window.settings.editor_font());
        window.install_accessible_labels();
        window.appearance.sync();
        window.compare.refresh_action_state();
        window.install_callbacks();
        window.install_style_callbacks();
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

    pub fn show_appearance(&self) {
        self.appearance.present(&self.shell.window);
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

    fn install_document_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.shell.open_button.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                window.request_open_dialog();
            }
        });

        let weak = Rc::downgrade(self);
        self.save_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_save();
            }
        });

        let weak = Rc::downgrade(self);
        self.recent_files_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.show_recent_files();
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

        let weak = Rc::downgrade(self);
        self.appearance_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.show_appearance();
            }
        });
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

    fn install_style_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        let _handler = adw::StyleManager::default().connect_dark_notify(move |_| {
            if let Some(window) = weak.upgrade() {
                window.workspace.apply_source_style_scheme_to_tabs();
            }
        });
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

fn configure_open_button(shell: &WindowShell) {
    let menu = crate::workspace_menu::build_open_menu();
    shell.open_button.set_menu_model(Some(&menu));
    shell
        .open_button
        .set_dropdown_tooltip(&pgettext("open menu tooltip", "Open Choices"));
}

fn configure_runtime_icon_support(window: &adw::ApplicationWindow) {
    if let Some(display) = gtk4::gdk::Display::default() {
        let icon_theme = gtk4::IconTheme::for_display(&display);
        icon_theme.add_resource_path("/io/github/cadric/Riteed/icons");
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
