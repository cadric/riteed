use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::dialogs::{self, UnsavedResponse};
use crate::document::DocumentState;
use crate::error::AppError;
use crate::settings::{AppSettings, ThemePreference};

#[derive(Clone)]
enum PendingAction {
    NewDocument,
    OpenDialog,
    OpenFile(gio::File),
    CloseWindow,
}

pub struct Window {
    shell: adw::ApplicationWindow,
    title_widget: adw::WindowTitle,
    toast_overlay: adw::ToastOverlay,
    text_view: gtk4::TextView,
    text_buffer: gtk4::TextBuffer,
    preferences_dialog: adw::PreferencesDialog,
    save_action: gio::SimpleAction,
    settings: AppSettings,
    document: RefCell<DocumentState>,
    pending_action: RefCell<Option<PendingAction>>,
    closing_allowed: RefCell<bool>,
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

    fn build(app: &adw::Application, settings: AppSettings) -> Result<Rc<Self>, AppError> {
        let builder = gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/window.ui");
        let window: adw::ApplicationWindow = builder_object(&builder, "window")?;
        let window_title: adw::WindowTitle = builder_object(&builder, "window_title")?;
        let toast_overlay: adw::ToastOverlay = builder_object(&builder, "toast_overlay")?;
        let text_view: gtk4::TextView = builder_object(&builder, "text_view")?;
        let primary_menu_button: gtk4::MenuButton =
            builder_object(&builder, "primary_menu_button")?;

        let preferences_builder =
            gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/preferences.ui");
        let preferences_dialog: adw::PreferencesDialog =
            builder_object(&preferences_builder, "preferences_dialog")?;
        let theme_row: adw::ComboRow = builder_object(&preferences_builder, "theme_row")?;
        let word_wrap_row: adw::SwitchRow = builder_object(&preferences_builder, "word_wrap_row")?;

        let shortcuts_builder =
            gtk4::Builder::from_resource("/io/github/cadric/Riteed/ui/shortcuts.ui");
        let shortcuts_window: gtk4::ShortcutsWindow =
            builder_object(&shortcuts_builder, "shortcuts_window")?;

        window.set_application(Some(app));
        window.set_help_overlay(Some(&shortcuts_window));

        let text_buffer = text_view.buffer();
        let save_action = gio::SimpleAction::new("save", None);
        let save_as_action = gio::SimpleAction::new("save-as", None);
        let close_action = gio::SimpleAction::new("close", None);
        window.add_action(&save_action);
        window.add_action(&save_as_action);
        window.add_action(&close_action);

        let menu = primary_menu_model();
        primary_menu_button.set_menu_model(Some(&menu));

        let (width, height) = settings.window_size();
        window.set_default_size(width, height);

        let themes = gtk4::StringList::new(&[
            &pgettext("theme choice", "System Default"),
            &pgettext("theme choice", "Light"),
            &pgettext("theme choice", "Dark"),
        ]);
        theme_row.set_model(Some(&themes));
        theme_row.set_selected(settings.theme().index());
        word_wrap_row.set_active(settings.word_wrap());
        settings.apply_theme();
        settings.apply_word_wrap(&text_view);

        let controller = Rc::new(Self {
            shell: window,
            title_widget: window_title,
            toast_overlay,
            text_view,
            text_buffer,
            preferences_dialog,
            save_action,
            settings,
            document: RefCell::new(DocumentState::new_empty()),
            pending_action: RefCell::new(None),
            closing_allowed: RefCell::new(false),
        });

        controller.install_callbacks(&theme_row, &word_wrap_row, &save_as_action, &close_action);
        controller.refresh_document_state();
        Ok(controller)
    }

    #[must_use]
    pub fn widget(&self) -> &adw::ApplicationWindow {
        &self.shell
    }

    pub fn present(&self) {
        self.shell.present();
        self.text_view.grab_focus();
    }

    pub fn request_new(self: &Rc<Self>) {
        if self.is_dirty() {
            self.replace_pending_action(PendingAction::NewDocument);
            self.confirm_unsaved();
            return;
        }
        self.new_document();
    }

    pub fn request_open_dialog(self: &Rc<Self>) {
        if self.is_dirty() {
            self.replace_pending_action(PendingAction::OpenDialog);
            self.confirm_unsaved();
            return;
        }
        self.show_open_dialog();
    }

    pub fn request_open_file(self: &Rc<Self>, file: gio::File) {
        if self.is_dirty() {
            self.replace_pending_action(PendingAction::OpenFile(file));
            self.confirm_unsaved();
            return;
        }
        self.load_file(&file);
    }

    pub fn request_save(self: &Rc<Self>) {
        if let Some(path) = self.document.borrow().path() {
            self.save_to_path(path);
        } else {
            self.show_save_dialog();
        }
    }

    pub fn request_save_as(self: &Rc<Self>) {
        self.show_save_dialog();
    }

    pub fn show_preferences(&self) {
        self.preferences_dialog.present(Some(&self.shell));
    }

    pub fn show_about(&self) {
        dialogs::show_about(&self.shell);
    }

    pub fn show_help(&self) {
        dialogs::launch_help(&self.shell, {
            let shell = self.shell.clone();
            move |error| dialogs::present_error(&shell, &error)
        });
    }

    #[cfg(test)]
    pub(crate) fn buffer_text_for_tests(&self) -> String {
        self.buffer_text()
    }

    #[cfg(test)]
    pub(crate) fn is_dirty_for_tests(&self) -> bool {
        self.is_dirty()
    }

    #[cfg(test)]
    pub(crate) fn set_text_for_tests(&self, text: &str) {
        self.text_buffer.set_text(text);
        self.refresh_document_state();
    }

    #[cfg(test)]
    pub(crate) fn close_request_for_tests(self: &Rc<Self>) -> glib::Propagation {
        self.on_close_request()
    }

    #[cfg(test)]
    pub(crate) fn save_to_path_for_tests(self: &Rc<Self>, path: PathBuf) {
        self.save_to_path(path);
    }

    #[cfg(test)]
    pub(crate) fn size_for_tests(&self) -> (i32, i32) {
        self.settings.window_size()
    }

    fn install_callbacks(
        self: &Rc<Self>,
        theme_row: &adw::ComboRow,
        word_wrap_row: &adw::SwitchRow,
        save_as_action: &gio::SimpleAction,
        close_action: &gio::SimpleAction,
    ) {
        let weak = Rc::downgrade(self);
        self.text_buffer.connect_changed(move |_| {
            if let Some(window) = weak.upgrade() {
                window.refresh_document_state();
            }
        });

        let weak = Rc::downgrade(self);
        self.shell.connect_close_request(move |_| {
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
        save_as_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.request_save_as();
            }
        });

        let weak = Rc::downgrade(self);
        close_action.connect_activate(move |_, _| {
            if let Some(window) = weak.upgrade() {
                window.widget().close();
            }
        });

        let settings = self.settings.clone();
        let text_view = self.text_view.clone();
        theme_row.connect_selected_notify(move |row| {
            let theme = ThemePreference::from_index(row.selected());
            settings.set_theme(theme);
            settings.apply_theme();
            settings.apply_word_wrap(&text_view);
        });

        let settings = self.settings.clone();
        let text_view = self.text_view.clone();
        word_wrap_row.connect_active_notify(move |row| {
            settings.set_word_wrap(row.is_active());
            settings.apply_word_wrap(&text_view);
        });
    }

    fn on_close_request(self: &Rc<Self>) -> glib::Propagation {
        if *self.closing_allowed.borrow() {
            self.persist_window_size();
            return glib::Propagation::Proceed;
        }
        if self.is_dirty() {
            self.replace_pending_action(PendingAction::CloseWindow);
            self.confirm_unsaved();
            return glib::Propagation::Stop;
        }
        self.persist_window_size();
        glib::Propagation::Proceed
    }

    fn replace_pending_action(&self, action: PendingAction) {
        *self.pending_action.borrow_mut() = Some(action);
    }

    fn clear_pending_action(&self) {
        *self.pending_action.borrow_mut() = None;
    }

    fn take_pending_action(&self) -> Option<PendingAction> {
        self.pending_action.borrow_mut().take()
    }

    fn confirm_unsaved(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        dialogs::confirm_unsaved_changes(&self.shell, move |response| {
            if let Some(window) = weak.upgrade() {
                match response {
                    UnsavedResponse::Cancel => window.clear_pending_action(),
                    UnsavedResponse::Discard => window.perform_pending_action(),
                    UnsavedResponse::Save => window.request_save(),
                }
            }
        });
    }

    fn perform_pending_action(self: &Rc<Self>) {
        let action = self.take_pending_action();
        match action {
            Some(PendingAction::NewDocument) => self.new_document(),
            Some(PendingAction::OpenDialog) => self.show_open_dialog(),
            Some(PendingAction::OpenFile(file)) => self.load_file(&file),
            Some(PendingAction::CloseWindow) => {
                *self.closing_allowed.borrow_mut() = true;
                self.widget().close();
            }
            None => {}
        }
    }

    fn new_document(&self) {
        self.document.borrow_mut().replace_with_new();
        self.text_buffer.set_text("");
        self.refresh_document_state();
        self.text_view.grab_focus();
    }

    fn open_dialog() -> gtk4::FileDialog {
        text_file_dialog(
            &pgettext("file dialog title", "Open a Text File"),
            &pgettext("file dialog action", "Open"),
        )
    }

    fn show_open_dialog(self: &Rc<Self>) {
        let dialog = Self::open_dialog();
        let weak = Rc::downgrade(self);
        dialog.open(
            Some(&self.shell),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(window) = weak.upgrade() {
                    match result {
                        Ok(file) => window.load_file(&file),
                        Err(error) => {
                            window.clear_pending_action();
                            if !error.matches(gtk4::DialogError::Dismissed) {
                                dialogs::present_error(&window.shell, &AppError::from(error));
                            }
                        }
                    }
                }
            },
        );
    }

    fn save_dialog(&self) -> gtk4::FileDialog {
        let dialog = text_file_dialog(
            &pgettext("file dialog title", "Save the Document"),
            &pgettext("file dialog action", "Save"),
        );
        dialog.set_initial_name(Some(&self.document.borrow().display_name()));
        if let Some(path) = self.document.borrow().path() {
            dialog.set_initial_file(Some(&gio::File::for_path(path)));
        }
        dialog
    }

    fn show_save_dialog(self: &Rc<Self>) {
        let dialog = self.save_dialog();
        let weak = Rc::downgrade(self);
        dialog.save(
            Some(&self.shell),
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(window) = weak.upgrade() {
                    match result {
                        Ok(file) => match local_path(&file) {
                            Ok(path) => {
                                window.save_to_path(DocumentState::normalized_save_path(&path));
                            }
                            Err(error) => {
                                window.clear_pending_action();
                                dialogs::present_error(&window.shell, &error);
                            }
                        },
                        Err(error) => {
                            window.clear_pending_action();
                            if !error.matches(gtk4::DialogError::Dismissed) {
                                dialogs::present_error(&window.shell, &AppError::from(error));
                            }
                        }
                    }
                }
            },
        );
    }

    fn load_file(self: &Rc<Self>, file: &gio::File) {
        let path = match local_path(file) {
            Ok(path) => path,
            Err(error) => {
                self.clear_pending_action();
                dialogs::present_error(&self.shell, &error);
                return;
            }
        };
        let file = file.clone();
        let weak = Rc::downgrade(self);
        file.load_contents_async(None::<&gio::Cancellable>, move |result| {
            if let Some(window) = weak.upgrade() {
                match result {
                    Ok((bytes, _etag)) => match String::from_utf8(bytes.as_ref().to_vec()) {
                        Ok(text) => {
                            *window.document.borrow_mut() =
                                DocumentState::from_loaded(path.clone(), text.clone());
                            window.text_buffer.set_text(&text);
                            window.refresh_document_state();
                            window.text_view.grab_focus();
                            window.clear_pending_action();
                        }
                        Err(_error) => {
                            window.clear_pending_action();
                            dialogs::present_error(
                                &window.shell,
                                &AppError::InvalidUtf8(path.clone()),
                            );
                        }
                    },
                    Err(error) => {
                        window.clear_pending_action();
                        dialogs::present_error(
                            &window.shell,
                            &AppError::ReadFailed(path.clone(), error.message().to_string()),
                        );
                    }
                }
            }
        });
    }

    fn save_to_path(self: &Rc<Self>, path: PathBuf) {
        let text = self.buffer_text();
        let bytes = text.clone().into_bytes();
        let file = gio::File::for_path(&path);
        let weak = Rc::downgrade(self);
        file.replace_contents_async(
            bytes,
            None,
            false,
            gio::FileCreateFlags::REPLACE_DESTINATION,
            None::<&gio::Cancellable>,
            move |result| {
                if let Some(window) = weak.upgrade() {
                    match result {
                        Ok((_etag, _)) => {
                            window
                                .document
                                .borrow_mut()
                                .set_saved(path.clone(), text.clone());
                            window.refresh_document_state();
                            window
                                .toast_overlay
                                .add_toast(adw::Toast::new(&gettext("The Document Was Saved.")));
                            window.perform_pending_action();
                        }
                        Err((_bytes, error)) => {
                            window.clear_pending_action();
                            dialogs::present_error(
                                &window.shell,
                                &AppError::WriteFailed(path.clone(), error.message().to_string()),
                            );
                        }
                    }
                }
            },
        );
    }

    fn is_dirty(&self) -> bool {
        self.document.borrow().is_dirty(&self.buffer_text())
    }

    fn buffer_text(&self) -> String {
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        self.text_buffer.text(&start, &end, true).to_string()
    }

    fn refresh_document_state(&self) {
        let current_text = self.buffer_text();
        let document = self.document.borrow();
        let title = if document.is_dirty(&current_text) {
            format!("{} *", document.display_name())
        } else {
            document.display_name()
        };
        self.title_widget.set_title(&title);
        self.title_widget.set_subtitle(&document.subtitle());
        self.save_action
            .set_enabled(document.is_dirty(&current_text));
    }

    fn persist_window_size(&self) {
        self.settings
            .set_window_size(self.shell.width(), self.shell.height());
    }
}

fn builder_object<T: IsA<glib::Object>>(builder: &gtk4::Builder, id: &str) -> Result<T, AppError> {
    builder
        .object(id)
        .ok_or_else(|| AppError::Internal(format!("Missing resource object `{id}`.")))
}

fn primary_menu_model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(Some(&pgettext("menu item", "New")), Some("app.new"));
    menu.append(Some(&pgettext("menu item", "Open")), Some("app.open"));
    menu.append(
        Some(&pgettext("menu item", "Keyboard Shortcuts")),
        Some("win.show-help-overlay"),
    );
    menu.append(
        Some(&pgettext("menu item", "Preferences")),
        Some("app.preferences"),
    );
    menu.append(Some(&pgettext("menu item", "Help")), Some("app.help"));
    menu.append(Some(&pgettext("menu item", "About")), Some("app.about"));
    menu.append(Some(&pgettext("menu item", "Quit")), Some("app.quit"));
    menu
}

fn text_file_dialog(title: &str, accept_label: &str) -> gtk4::FileDialog {
    let dialog = gtk4::FileDialog::builder()
        .title(title)
        .accept_label(accept_label)
        .modal(true)
        .build();

    let text_filter = gtk4::FileFilter::new();
    text_filter.set_name(Some(&pgettext("file filter", "Plain Text Files")));
    text_filter.add_mime_type("text/plain");
    text_filter.add_suffix("txt");

    let any_filter = gtk4::FileFilter::new();
    any_filter.set_name(Some(&pgettext("file filter", "All Files")));
    any_filter.add_pattern("*");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&text_filter);
    filters.append(&any_filter);

    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&text_filter));
    dialog
}

fn local_path(file: &gio::File) -> Result<PathBuf, AppError> {
    file.path().ok_or(AppError::NonLocalFile)
}

#[cfg(test)]
pub(crate) fn builder_object_for_tests() -> Result<gtk4::TextView, AppError> {
    builder_object(&gtk4::Builder::new(), "missing")
}

#[cfg(test)]
pub(crate) fn primary_menu_model_for_tests() -> gio::Menu {
    primary_menu_model()
}

#[cfg(test)]
pub(crate) fn text_file_dialog_for_tests(title: &str, accept_label: &str) -> gtk4::FileDialog {
    text_file_dialog(title, accept_label)
}

#[cfg(test)]
pub(crate) fn local_path_for_tests(file: &gio::File) -> Result<PathBuf, AppError> {
    local_path(file)
}
