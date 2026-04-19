use std::path::Path;
use std::rc::Rc;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::document::DocumentState;
use crate::editor_io::{self, LoadedDocument, SavedDocument};
use crate::editor_language::{self, LanguageDetection};
use crate::editor_monitor::{self, ExternalFileEvent, MonitorBinding, PendingExternalState};
use crate::editor_tab::{EditorTab, ReloadCause, ReloadResult, SaveOutcome, SaveResult};
use crate::editor_view::ReloadSnapshot;
use crate::error::AppError;
use gettextrs::pgettext;

impl EditorTab {
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
                            tab.apply_loaded_text(path.clone(), &text);
                            let monitored_file = gio::File::for_path(&path);
                            tab.swap_monitor(&monitored_file);
                            tab.refresh_language_for_file(&gio::File::for_path(path));
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

    pub fn reload_from_disk(
        self: &Rc<Self>,
        cause: ReloadCause,
        should_apply: Rc<dyn Fn() -> bool>,
        callback: Rc<dyn Fn(Result<ReloadResult, AppError>)>,
    ) {
        let Some(saved_file) = self.saved_file() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        let Some(expected_uri) = self.uri() else {
            callback(Err(AppError::MissingSavePath));
            return;
        };
        let snapshot = ReloadSnapshot::capture(&self.text_buffer);
        let already_loading = {
            let mut state = self.state.borrow_mut();
            if state.progress.loading || state.progress.external_reload_in_progress {
                true
            } else {
                state.progress.loading = true;
                state.progress.external_reload_in_progress = true;
                false
            }
        };
        if already_loading {
            callback(Ok(ReloadResult::Deferred));
            return;
        }
        if let Some(page) = self.page() {
            page.set_loading(true);
        }

        let weak = Rc::downgrade(self);
        editor_io::load_utf8_file(
            &saved_file,
            Rc::new(move |result| {
                if let Some(tab) = weak.upgrade() {
                    match result {
                        Ok(LoadedDocument { path, text, .. }) => {
                            if !tab.can_apply_reload(cause, &expected_uri, &should_apply) {
                                tab.finish_reload(false);
                                callback(Ok(ReloadResult::Deferred));
                                return;
                            }
                            tab.apply_reloaded_text(path.clone(), &text, &snapshot);
                            tab.refresh_language_for_file(&gio::File::for_path(path));
                            tab.finish_reload(true);
                            callback(Ok(ReloadResult::Applied));
                        }
                        Err(error) => {
                            tab.finish_reload(false);
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
        callback: Rc<dyn Fn(SaveResult)>,
    ) {
        let current_path = self.state.borrow().document.path();
        if !force_save_as && let Some(path) = current_path {
            if self.should_show_stale_save_conflict() {
                let weak = Rc::downgrade(self);
                crate::dialogs::confirm_stale_save(parent, &self.title(), move |choice| {
                    if let Some(tab) = weak.upgrade() {
                        match choice {
                            crate::dialogs::StaleSaveResponse::SaveAnyway => {
                                tab.save_to_path(&path, callback.clone());
                            }
                            crate::dialogs::StaleSaveResponse::Cancel => {
                                callback(SaveResult::CancelledByUser);
                            }
                        }
                    }
                });
                return;
            }
            self.save_to_path(&path, callback);
            return;
        }
        self.show_save_dialog(parent, callback);
    }

    pub fn handle_external_event(self: &Rc<Self>, event: ExternalFileEvent) {
        match event {
            ExternalFileEvent::Moved { new_file } => {
                if let Ok(path) = editor_io::local_path(&new_file) {
                    {
                        let mut state = self.state.borrow_mut();
                        state.document.set_saved(path);
                        state.pending_external = PendingExternalState::Idle;
                    }
                    self.swap_monitor(&new_file);
                    self.refresh_language_for_file(&new_file);
                    self.set_attention(false);
                    self.sync_presentation();
                    self.notify_external_state_change();
                } else {
                    self.set_pending_external(editor_monitor::next_pending_state(
                        &self.pending_external_state(),
                        ExternalFileEvent::ContentPossiblyChanged,
                    ));
                }
            }
            other => {
                let next =
                    editor_monitor::next_pending_state(&self.pending_external_state(), other);
                self.set_pending_external(next);
            }
        }
    }

    pub fn swap_monitor(self: &Rc<Self>, file: &gio::File) {
        self.clear_monitor();
        let weak = Rc::downgrade(self);
        if let Ok(binding) = MonitorBinding::new(
            file,
            Rc::new(move |event| {
                if let Some(tab) = weak.upgrade() {
                    tab.handle_external_event(event);
                }
            }),
        ) {
            self.state.borrow_mut().monitor = Some(binding);
        }
    }

    pub fn clear_monitor(&self) {
        if let Some(binding) = self.state.borrow_mut().monitor.take() {
            binding.cancel();
        }
    }

    fn show_save_dialog(
        self: &Rc<Self>,
        parent: &adw::ApplicationWindow,
        callback: Rc<dyn Fn(SaveResult)>,
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
        super::apply_text_filters(&dialog);

        let weak = Rc::downgrade(self);
        dialog.save(Some(parent), None::<&gio::Cancellable>, move |result| {
            if let Some(tab) = weak.upgrade() {
                match result {
                    Ok(file) => match editor_io::local_path(&file) {
                        Ok(path) => {
                            let normalized = DocumentState::normalized_save_path(&path);
                            tab.save_to_path(&normalized, callback.clone());
                        }
                        Err(error) => callback(SaveResult::Failed(error)),
                    },
                    Err(error) if error.matches(gtk4::DialogError::Dismissed) => {
                        callback(SaveResult::CancelledByUser);
                    }
                    Err(error) => callback(SaveResult::Failed(AppError::from(error))),
                }
            }
        });
    }

    fn save_to_path(self: &Rc<Self>, path: &Path, callback: Rc<dyn Fn(SaveResult)>) {
        let old_uri = self.uri();
        let previous_file = self.saved_file();
        self.clear_monitor();
        self.set_loading(true);
        let text = self.buffer_text();
        let weak = Rc::downgrade(self);
        editor_io::save_utf8_file(
            path,
            &text,
            Rc::new(move |result| {
                if let Some(tab) = weak.upgrade() {
                    match result {
                        Ok(SavedDocument { path, uri }) => {
                            tab.state.borrow_mut().document.set_saved(path.clone());
                            tab.text_buffer.set_modified(false);
                            tab.resolve_pending_external();
                            tab.swap_monitor(&gio::File::for_path(&path));
                            tab.refresh_language_for_file(&gio::File::for_path(path));
                            tab.set_loading(false);
                            tab.sync_presentation();
                            tab.grab_focus();
                            callback(SaveResult::Saved(SaveOutcome {
                                old_uri: old_uri.clone(),
                                new_uri: uri,
                            }));
                        }
                        Err(error) => {
                            if let Some(previous_file) = previous_file.clone() {
                                tab.swap_monitor(&previous_file);
                            }
                            tab.set_loading(false);
                            tab.sync_presentation();
                            callback(SaveResult::Failed(error));
                        }
                    }
                }
            }),
        );
    }

    fn refresh_language_for_file(self: &Rc<Self>, file: &gio::File) {
        let identity = file.uri().to_string();
        let generation = {
            let mut state = self.state.borrow_mut();
            state.language_request_generation += 1;
            state.language_request_generation
        };
        let weak = Rc::downgrade(self);
        editor_language::detect_for_file(
            file,
            Rc::new(move |detection| {
                if let Some(tab) = weak.upgrade() {
                    tab.apply_language_detection(generation, &identity, &detection);
                }
            }),
        );
    }

    fn apply_language_detection(
        &self,
        generation: u64,
        identity: &str,
        detection: &LanguageDetection,
    ) {
        let current_uri = self.state.borrow().document.uri();
        let should_apply = {
            let mut state = self.state.borrow_mut();
            if state.language_request_generation != generation
                || current_uri.as_deref() != Some(identity)
            {
                false
            } else {
                state.content_type.clone_from(&detection.content_type);
                state.language_id.clone_from(&detection.language_id);
                true
            }
        };
        if should_apply {
            editor_language::apply_detection(&self.text_buffer, detection);
            self.sync_presentation();
        }
    }

    fn saved_file(&self) -> Option<gio::File> {
        self.state.borrow().document.path().map(gio::File::for_path)
    }

    fn apply_loaded_text(&self, path: std::path::PathBuf, text: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.document = DocumentState::from_loaded(path);
            state.pending_external = PendingExternalState::Idle;
            state.ui.external_prompt_active = false;
            state.content_type = None;
            state.language_id = None;
            state.ui.suppress_changes = true;
        }
        self.replace_buffer_text(text);
        self.text_buffer.set_modified(false);
        self.state.borrow_mut().ui.suppress_changes = false;
        self.set_attention(false);
        self.set_banner_revealed(false);
    }

    fn apply_reloaded_text(&self, path: std::path::PathBuf, text: &str, snapshot: &ReloadSnapshot) {
        {
            let mut state = self.state.borrow_mut();
            state.document = DocumentState::from_loaded(path);
            state.pending_external = PendingExternalState::Idle;
            state.ui.external_prompt_active = false;
            state.ui.suppress_changes = true;
        }
        self.replace_buffer_text(text);
        snapshot.apply(&self.text_buffer);
        self.text_buffer.set_modified(false);
        self.state.borrow_mut().ui.suppress_changes = false;
        self.set_attention(false);
        self.set_banner_revealed(false);
    }

    fn can_apply_reload(
        &self,
        cause: ReloadCause,
        expected_uri: &str,
        should_apply: &Rc<dyn Fn() -> bool>,
    ) -> bool {
        self.uri().as_deref() == Some(expected_uri)
            && self.monitor_target_matches_current()
            && match cause {
                ReloadCause::Automatic => !self.is_dirty() && should_apply(),
                ReloadCause::UserRequested => should_apply(),
            }
    }

    fn finish_reload(&self, applied: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.progress.loading = false;
            state.progress.external_reload_in_progress = false;
            if !applied && matches!(state.pending_external, PendingExternalState::Idle) {
                state.pending_external = PendingExternalState::ContentPossiblyChanged {
                    acknowledged: false,
                };
            }
        }
        if let Some(page) = self.page() {
            page.set_loading(false);
        }
        self.sync_presentation();
        self.notify_external_state_change();
    }

    fn replace_buffer_text(&self, text: &str) {
        let undo_enabled = self.text_buffer.enables_undo();
        self.text_buffer.set_enable_undo(false);
        self.text_buffer.set_text(text);
        self.text_buffer.set_enable_undo(undo_enabled);
    }

    fn set_pending_external(&self, pending: PendingExternalState) {
        self.state.borrow_mut().pending_external = pending;
        self.set_attention(!self.state.borrow().pending_external.is_idle());
        self.notify_external_state_change();
    }

    pub(super) fn notify_external_state_change(&self) {
        if let Some(callback) = self.on_external_state_change.get() {
            callback();
        }
    }

    pub(super) fn set_attention(&self, needs_attention: bool) {
        if let Some(page) = self.page() {
            page.set_needs_attention(needs_attention);
        }
    }

    pub(super) fn set_banner_revealed(&self, revealed: bool) {
        self.state.borrow_mut().ui.banner_syncing = true;
        self.banner.set_revealed(revealed);
        self.state.borrow_mut().ui.banner_syncing = false;
    }

    pub(super) fn set_loading(&self, loading: bool) {
        self.state.borrow_mut().progress.loading = loading;
        if let Some(page) = self.page() {
            page.set_loading(loading);
        }
    }
}
