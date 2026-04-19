use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs;
use crate::error::AppError;
use crate::settings::{AppSettings, ThemePreference};
use crate::window_shell::WindowShell;
use crate::workspace::{OpenSource, Workspace, WorkspaceParts};

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

        let themes = gtk4::StringList::new(&[
            &pgettext("theme choice", "System Default"),
            &pgettext("theme choice", "Light"),
            &pgettext("theme choice", "Dark"),
        ]);
        shell.theme_row.set_model(Some(&themes));
        shell.theme_row.set_selected(settings.theme().index());
        shell.word_wrap_row.set_active(settings.word_wrap());
        shell
            .line_numbers_row
            .set_active(settings.show_line_numbers());
        shell.minimap_row.set_active(settings.show_minimap());
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
        });
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

    pub fn request_open_recent(self: &Rc<Self>, uri: &str) {
        self.workspace.request_open_recent(uri);
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

    #[cfg(test)]
    pub(crate) fn tab_count_for_tests(&self) -> i32 {
        self.workspace.tab_count()
    }

    #[cfg(test)]
    pub(crate) fn selected_title_for_tests(&self) -> String {
        self.workspace.selected_title()
    }

    #[cfg(test)]
    pub(crate) fn selected_text_for_tests(&self) -> String {
        self.workspace.selected_text()
    }

    #[cfg(test)]
    pub(crate) fn set_selected_text_for_tests(&self, text: &str) {
        self.workspace.set_selected_text(text);
    }

    #[cfg(test)]
    pub(crate) fn close_request_for_tests(self: &Rc<Self>) -> glib::Propagation {
        self.on_close_request()
    }

    #[cfg(test)]
    pub(crate) fn size_for_tests(&self) -> (i32, i32) {
        self.settings.window_size()
    }

    #[cfg(test)]
    pub(crate) fn recent_files_for_tests(&self) -> Vec<String> {
        self.workspace.recent_files()
    }

    #[cfg(test)]
    pub(crate) fn session_files_for_tests(&self) -> Vec<String> {
        self.workspace.session_files()
    }

    #[cfg(test)]
    pub(crate) fn selected_saved_uri_for_tests(&self) -> String {
        self.workspace.selected_saved_uri()
    }

    #[cfg(test)]
    pub(crate) fn text_for_uri_for_tests(&self, uri: &str) -> Option<String> {
        self.workspace.text_for_uri(uri)
    }

    #[cfg(test)]
    pub(crate) fn reorder_selected_to_first_for_tests(&self) -> bool {
        self.workspace.reorder_selected_to_first()
    }

    #[cfg(test)]
    pub(crate) fn shortcuts_enabled_for_tests(&self) -> bool {
        self.workspace.shortcuts_enabled()
    }

    #[cfg(test)]
    pub(crate) fn search_visible_for_tests(&self) -> bool {
        self.workspace.search_visible()
    }

    #[cfg(test)]
    pub(crate) fn replace_visible_for_tests(&self) -> bool {
        self.workspace.replace_visible()
    }

    #[cfg(test)]
    pub(crate) fn search_query_for_tests(&self) -> String {
        self.workspace.search_query()
    }

    #[cfg(test)]
    pub(crate) fn search_result_for_tests(&self) -> String {
        self.workspace.search_result()
    }

    #[cfg(test)]
    pub(crate) fn status_labels_for_tests(&self) -> (String, String, String) {
        self.workspace.status_labels()
    }

    #[cfg(test)]
    pub(crate) fn selected_line_numbers_visible_for_tests(&self) -> bool {
        self.workspace.selected_line_numbers_visible()
    }

    #[cfg(test)]
    pub(crate) fn select_offsets_for_tests(&self, start: i32, end: i32) {
        self.workspace.select_offsets_in_selected(start, end);
    }

    #[cfg(test)]
    pub(crate) fn undo_selected_for_tests(&self) {
        self.workspace.undo_selected();
    }

    #[cfg(test)]
    pub(crate) fn replace_current_for_tests(self: &Rc<Self>) {
        self.workspace.replace_current_for_tests();
    }

    #[cfg(test)]
    pub(crate) fn replace_all_for_tests(self: &Rc<Self>) {
        self.workspace.replace_all_for_tests();
    }

    #[cfg(test)]
    pub(crate) fn set_replace_text_for_tests(&self, text: &str) {
        self.workspace.set_replace_text_for_tests(text);
    }

    #[cfg(test)]
    pub(crate) fn set_line_numbers_for_tests(&self, enabled: bool) {
        self.settings.set_show_line_numbers(enabled);
        self.shell.line_numbers_row.set_active(enabled);
        self.refresh_line_numbers();
    }

    #[cfg(test)]
    pub(crate) fn set_minimap_for_tests(&self, enabled: bool) {
        self.settings.set_show_minimap(enabled);
        self.shell.minimap_row.set_active(enabled);
        self.refresh_minimap();
    }

    #[cfg(test)]
    pub(crate) fn selected_minimap_visible_for_tests(&self) -> bool {
        self.workspace.selected_minimap_visible()
    }

    #[cfg(test)]
    pub(crate) fn selected_language_id_for_tests(&self) -> Option<String> {
        self.workspace.selected_language_id()
    }

    #[cfg(test)]
    pub(crate) fn selected_banner_visible_for_tests(&self) -> bool {
        self.workspace.selected_banner_visible()
    }

    #[cfg(test)]
    pub(crate) fn sync_selected_banner_for_tests(&self, window_active: bool) {
        self.workspace.sync_selected_banner_for_tests(window_active);
    }

    #[cfg(test)]
    pub(crate) fn trigger_selected_external_action_for_tests(&self) {
        self.workspace.trigger_selected_external_action_for_tests();
    }

    #[cfg(test)]
    pub(crate) fn inject_external_event_for_tests(
        self: &Rc<Self>,
        uri: &str,
        event: crate::editor_monitor::ExternalFileEvent,
    ) {
        self.workspace.inject_external_event_for_tests(uri, event);
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

        let settings = self.settings.clone();
        self.shell.theme_row.connect_selected_notify(move |row| {
            let theme = ThemePreference::from_index(row.selected());
            settings.set_theme(theme);
            settings.apply_theme();
        });

        let settings = self.settings.clone();
        let weak = Rc::downgrade(self);
        self.shell.word_wrap_row.connect_active_notify(move |row| {
            settings.set_word_wrap(row.is_active());
            if let Some(window) = weak.upgrade() {
                window.refresh_word_wrap();
            }
        });

        let settings = self.settings.clone();
        let weak = Rc::downgrade(self);
        self.shell
            .line_numbers_row
            .connect_active_notify(move |row| {
                settings.set_show_line_numbers(row.is_active());
                if let Some(window) = weak.upgrade() {
                    window.refresh_line_numbers();
                }
            });

        let settings = self.settings.clone();
        let weak = Rc::downgrade(self);
        self.shell.minimap_row.connect_active_notify(move |row| {
            settings.set_show_minimap(row.is_active());
            if let Some(window) = weak.upgrade() {
                window.refresh_minimap();
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

    fn refresh_word_wrap(&self) {
        self.workspace.apply_word_wrap_to_tabs();
    }

    fn refresh_line_numbers(&self) {
        self.workspace.apply_line_numbers_to_tabs();
    }

    fn refresh_minimap(&self) {
        self.workspace.apply_minimap_to_tabs();
    }

    fn persist_window_size(&self) {
        self.settings
            .set_window_size(self.shell.window.width(), self.shell.window.height());
    }
}
