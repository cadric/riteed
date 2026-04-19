use std::cell::{OnceCell, RefCell};
use std::path::Path;
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gdk, gio, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::document::DocumentState;
use crate::editor_io::{self, LoadedDocument, SavedDocument};
use crate::error::AppError;
use crate::settings::AppSettings;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveOutcome {
    pub old_uri: Option<String>,
    pub new_uri: String,
}

struct EditorTabState {
    document: DocumentState,
    loading: bool,
    suppress_changes: bool,
}

pub struct EditorTab {
    root: gtk4::ScrolledWindow,
    text_view: sourceview5::View,
    text_buffer: sourceview5::Buffer,
    settings: AppSettings,
    state: RefCell<EditorTabState>,
    page: OnceCell<adw::TabPage>,
    on_file_drop: OnceCell<Rc<dyn Fn(Vec<gio::File>)>>,
    on_visual_change: OnceCell<Rc<dyn Fn()>>,
}

impl EditorTab {
    #[must_use]
    pub fn new(settings: &AppSettings) -> Rc<Self> {
        let text_buffer = sourceview5::Buffer::builder().enable_undo(true).build();
        let text_view = sourceview5::View::with_buffer(&text_buffer);
        text_view.set_accepts_tab(true);
        text_view.set_bottom_margin(12);
        text_view.set_left_margin(12);
        text_view.set_monospace(true);
        text_view.set_right_margin(12);
        text_view.set_show_line_numbers(settings.show_line_numbers());
        text_view.set_top_margin(12);
        settings.apply_word_wrap(&text_view);

        let root = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Automatic)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&text_view)
            .build();
        root.set_hexpand(true);
        root.set_vexpand(true);

        let tab = Rc::new(Self {
            root,
            text_view,
            text_buffer,
            settings: settings.clone(),
            state: RefCell::new(EditorTabState {
                document: DocumentState::new_empty(),
                loading: false,
                suppress_changes: false,
            }),
            page: OnceCell::new(),
            on_file_drop: OnceCell::new(),
            on_visual_change: OnceCell::new(),
        });
        tab.install_callbacks();
        tab.sync_presentation();
        tab
    }

    pub fn attach(self: &Rc<Self>, tab_view: &adw::TabView) -> adw::TabPage {
        let page = tab_view.append(&self.root);
        page.set_indicator_activatable(false);
        let _set_page = self.page.set(page.clone());
        self.sync_presentation();
        page
    }

    pub fn set_visual_change_handler(&self, callback: Rc<dyn Fn()>) {
        let _set_callback = self.on_visual_change.set(callback);
    }

    pub fn set_file_drop_handler(&self, callback: Rc<dyn Fn(Vec<gio::File>)>) {
        let _set_callback = self.on_file_drop.set(callback);
    }

    #[must_use]
    pub fn page(&self) -> Option<adw::TabPage> {
        self.page.get().cloned()
    }

    #[must_use]
    pub fn uri(&self) -> Option<String> {
        self.state.borrow().document.uri()
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.text_buffer.is_modified()
    }

    #[must_use]
    pub fn is_clean_untitled(&self) -> bool {
        self.state.borrow().document.path().is_none() && !self.text_buffer.is_modified()
    }

    #[must_use]
    pub fn title(&self) -> String {
        self.state
            .borrow()
            .document
            .file_name()
            .unwrap_or_else(|| pgettext("document title", "Untitled"))
    }

    #[must_use]
    pub fn subtitle(&self) -> String {
        self.state
            .borrow()
            .document
            .path_display()
            .unwrap_or_else(|| pgettext("document subtitle", "Plain Text Document"))
    }

    #[must_use]
    pub fn save_name_suggestion(&self) -> String {
        self.state
            .borrow()
            .document
            .file_name()
            .unwrap_or_else(|| pgettext("save file name", "Untitled.txt"))
    }

    #[must_use]
    pub fn buffer_text(&self) -> String {
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        self.text_buffer.text(&start, &end, true).to_string()
    }

    pub fn grab_focus(&self) {
        self.text_view.grab_focus();
    }

    pub fn apply_word_wrap(&self) {
        self.settings.apply_word_wrap(&self.text_view);
    }

    pub fn apply_line_numbers(&self) {
        self.text_view
            .set_show_line_numbers(self.settings.show_line_numbers());
    }

    #[must_use]
    pub fn text_buffer(&self) -> sourceview5::Buffer {
        self.text_buffer.clone()
    }

    #[must_use]
    pub fn text_view(&self) -> sourceview5::View {
        self.text_view.clone()
    }

    #[must_use]
    pub fn single_line_selection_text(&self) -> Option<String> {
        let (start, end) = self.text_buffer.selection_bounds()?;
        if start.line() != end.line() || start.offset() == end.offset() {
            return None;
        }
        Some(self.text_buffer.text(&start, &end, true).to_string())
    }

    #[must_use]
    pub fn cursor_position(&self) -> (u32, u32) {
        let iter = self
            .text_buffer
            .iter_at_mark(&self.text_buffer.get_insert());
        (
            (iter.line() + 1).cast_unsigned(),
            (iter.line_offset() + 1).cast_unsigned(),
        )
    }

    pub fn load_file(
        self: &Rc<Self>,
        file: &gio::File,
        callback: Rc<dyn Fn(Result<String, AppError>)>,
    ) {
        self.set_loading(true);
        let weak = Rc::downgrade(self);
        editor_io::load_utf8_file(
            file,
            Rc::new(move |result| {
                if let Some(tab) = weak.upgrade() {
                    match result {
                        Ok(LoadedDocument { path, text, uri }) => {
                            {
                                let mut state = tab.state.borrow_mut();
                                state.document = DocumentState::from_loaded(path);
                                state.suppress_changes = true;
                            }
                            let undo_enabled = tab.text_buffer.enables_undo();
                            tab.text_buffer.set_enable_undo(false);
                            tab.text_buffer.set_text(&text);
                            tab.text_buffer.set_enable_undo(undo_enabled);
                            tab.text_buffer.set_modified(false);
                            tab.state.borrow_mut().suppress_changes = false;
                            tab.set_loading(false);
                            tab.sync_presentation();
                            tab.grab_focus();
                            callback(Ok(uri));
                        }
                        Err(error) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(Err(error));
                        }
                    }
                }
            }),
        );
    }

    pub fn request_save(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        force_save_as: bool,
        callback: Rc<dyn Fn(Result<SaveOutcome, AppError>)>,
    ) {
        let current_path = self.state.borrow().document.path();
        if !force_save_as && let Some(path) = current_path {
            self.save_to_path(&path, callback);
            return;
        }
        self.show_save_dialog(parent, callback);
    }

    #[cfg(test)]
    pub(crate) fn set_text_for_tests(&self, text: &str) {
        self.text_buffer.set_text(text);
        self.sync_presentation();
    }

    #[cfg(test)]
    pub(crate) fn select_offsets_for_tests(&self, start: i32, end: i32) {
        let start_iter = self.text_buffer.iter_at_offset(start);
        let end_iter = self.text_buffer.iter_at_offset(end);
        self.text_buffer.select_range(&start_iter, &end_iter);
    }

    #[cfg(test)]
    pub(crate) fn undo_for_tests(&self) {
        self.text_buffer.undo();
    }

    #[cfg(test)]
    pub(crate) fn shows_line_numbers_for_tests(&self) -> bool {
        self.text_view.shows_line_numbers()
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.text_buffer.connect_changed(move |_| {
            if let Some(tab) = weak.upgrade() {
                let should_update = !tab.state.borrow().suppress_changes;
                if should_update {
                    tab.sync_presentation();
                }
            }
        });

        let weak = Rc::downgrade(self);
        self.text_buffer.connect_modified_changed(move |_| {
            if let Some(tab) = weak.upgrade() {
                let should_update = !tab.state.borrow().suppress_changes;
                if should_update {
                    tab.sync_presentation();
                }
            }
        });

        let weak = Rc::downgrade(self);
        self.text_buffer.connect_cursor_moved(move |_| {
            if let Some(tab) = weak.upgrade()
                && let Some(callback) = tab.on_visual_change.get()
            {
                callback();
            }
        });

        let weak = Rc::downgrade(self);
        install_file_drop_target(&self.root, &weak);
        install_file_drop_target(&self.text_view, &weak);
    }

    fn show_save_dialog(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        callback: Rc<dyn Fn(Result<SaveOutcome, AppError>)>,
    ) {
        let dialog = gtk4::FileDialog::builder()
            .title(pgettext("file dialog title", "Save the Document"))
            .accept_label(pgettext("file dialog action", "Save"))
            .modal(true)
            .build();
        dialog.set_initial_name(Some(&self.save_name_suggestion()));
        if let Some(path) = self.state.borrow().document.path() {
            dialog.set_initial_file(Some(&gio::File::for_path(path)));
        }
        apply_text_filters(&dialog);

        let weak = Rc::downgrade(self);
        dialog.save(Some(parent), None::<&gio::Cancellable>, move |result| {
            if let Some(tab) = weak.upgrade() {
                match result {
                    Ok(file) => match editor_io::local_path(&file) {
                        Ok(path) => {
                            let normalized = DocumentState::normalized_save_path(&path);
                            tab.save_to_path(&normalized, callback.clone());
                        }
                        Err(error) => callback(Err(error)),
                    },
                    Err(error) => {
                        if error.matches(gtk4::DialogError::Dismissed) {
                            callback(Err(AppError::Cancelled));
                        } else {
                            callback(Err(AppError::from(error)));
                        }
                    }
                }
            }
        });
    }

    fn save_to_path(
        self: &Rc<Self>,
        path: &Path,
        callback: Rc<dyn Fn(Result<SaveOutcome, AppError>)>,
    ) {
        let old_uri = self.uri();
        let text = self.buffer_text();
        self.set_loading(true);
        let weak = Rc::downgrade(self);
        editor_io::save_utf8_file(
            path,
            &text,
            Rc::new(move |result| {
                if let Some(tab) = weak.upgrade() {
                    match result {
                        Ok(SavedDocument { path, uri }) => {
                            tab.state.borrow_mut().document.set_saved(path);
                            tab.text_buffer.set_modified(false);
                            tab.set_loading(false);
                            tab.sync_presentation();
                            tab.grab_focus();
                            callback(Ok(SaveOutcome {
                                old_uri: old_uri.clone(),
                                new_uri: uri,
                            }));
                        }
                        Err(error) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(Err(error));
                        }
                    }
                }
            }),
        );
    }

    fn set_loading(&self, loading: bool) {
        self.state.borrow_mut().loading = loading;
        if let Some(page) = self.page() {
            page.set_loading(loading);
        }
    }

    fn sync_presentation(&self) {
        if let Some(page) = self.page() {
            page.set_title(&self.title());
            page.set_tooltip(&self.subtitle());
            if self.is_dirty() {
                let icon = gio::ThemedIcon::new("document-modified-symbolic");
                page.set_indicator_icon(Some(&icon));
            } else {
                page.set_indicator_icon(Option::<&gio::Icon>::None);
            }
        }

        if let Some(callback) = self.on_visual_change.get() {
            callback();
        }
    }
}

fn install_file_drop_target(widget: &impl IsA<gtk4::Widget>, weak: &std::rc::Weak<EditorTab>) {
    let drop_target = gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
    drop_target.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let weak = weak.clone();
    drop_target.connect_drop(move |_, value, _, _| {
        let Some(tab) = weak.upgrade() else {
            return false;
        };
        let Some(handler) = tab.on_file_drop.get() else {
            return false;
        };
        match value.get::<gdk::FileList>() {
            Ok(file_list) => {
                handler(file_list.files());
                true
            }
            Err(_) => false,
        }
    });
    widget.add_controller(drop_target);
}

fn apply_text_filters(dialog: &gtk4::FileDialog) {
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
}
