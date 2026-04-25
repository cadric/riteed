use std::rc::Rc;

use gtk4::{gio, glib::SList, prelude::*};
use sourceview5::prelude::*;

use crate::document::DocumentState;
use crate::editor_io::{LoadedDocument, SavedDocument};
use crate::editor_language::{self, LanguageDetection};
use crate::editor_monitor::PendingExternalState;
use crate::editor_tab::{EditorTab, ReloadCause, Writability};
use crate::editor_view::ReloadSnapshot;

impl EditorTab {
    pub fn cancel_io(&self) {
        let cancellable = {
            let mut state = self.state.borrow_mut();
            state.io.candidate_encodings = None;
            state.io.cancellable.take()
        };
        if let Some(cancellable) = cancellable {
            cancellable.cancel();
        }
        if let Some(cancellable) = self
            .state
            .borrow_mut()
            .safety
            .writability_cancellable
            .take()
        {
            cancellable.cancel();
        }
    }

    #[must_use]
    pub fn current_format(&self) -> crate::editor_format::SavedTextFormat {
        self.state.borrow().document.format().clone()
    }

    #[must_use]
    pub fn current_format_summary(&self) -> String {
        self.state.borrow().document.format().summary()
    }

    #[must_use]
    pub fn current_line_ending_mode(&self) -> crate::editor_format::LineEndingMode {
        self.state.borrow().document.format().line_ending_mode()
    }

    #[must_use]
    pub fn can_reopen_with_encoding(&self) -> bool {
        self.saved_file().is_some()
            && !self.pending_external_state().is_missing()
            && !self.is_loading()
            && !self.state.borrow().ui.external_prompt_active
    }

    pub fn set_current_line_ending_mode(
        &self,
        line_ending_mode: crate::editor_format::LineEndingMode,
    ) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.document.format().line_ending_mode() == line_ending_mode {
                false
            } else {
                state.document.set_line_ending_mode(line_ending_mode);
                true
            }
        };
        if changed {
            self.sync_presentation();
        }
    }

    pub fn set_current_encoding(&self, encoding: crate::editor_format::EncodingInfo) {
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.document.format().encoding() == &encoding {
                false
            } else {
                state.document.set_encoding(encoding);
                true
            }
        };
        if changed {
            self.sync_presentation();
        }
    }

    pub fn handle_external_event(self: &Rc<Self>, event: crate::editor_monitor::ExternalFileEvent) {
        match event {
            crate::editor_monitor::ExternalFileEvent::Moved { new_file } => {
                if let Ok(path_info) = crate::editor_io::local_path_info(&new_file) {
                    {
                        let mut state = self.state.borrow_mut();
                        state
                            .document
                            .set_saved_with_display_path(path_info.path, path_info.display_path);
                        state.pending_external = PendingExternalState::Idle;
                    }
                    self.swap_monitor(&new_file);
                    self.refresh_language_for_file(&new_file);
                    self.refresh_writability_for_file(&new_file);
                    self.set_attention(false);
                    self.sync_presentation();
                    self.notify_external_state_change();
                } else {
                    self.set_pending_external(crate::editor_monitor::next_pending_state(
                        &self.pending_external_state(),
                        crate::editor_monitor::ExternalFileEvent::ContentPossiblyChanged,
                    ));
                }
            }
            other => {
                let next = crate::editor_monitor::next_pending_state(
                    &self.pending_external_state(),
                    other,
                );
                self.set_pending_external(next);
            }
        }
    }

    pub fn swap_monitor(self: &Rc<Self>, file: &gio::File) {
        self.clear_monitor();
        let weak = Rc::downgrade(self);
        if let Ok(binding) = crate::editor_monitor::MonitorBinding::new(
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

    pub(super) fn refresh_language_for_file(self: &Rc<Self>, file: &gio::File) {
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
        let should_apply = {
            let mut state = self.state.borrow_mut();
            let matches_request = state.language_request_generation == generation
                && state.document.uri().as_deref() == Some(identity);
            if matches_request {
                state.content_type.clone_from(&detection.content_type);
                state.language_id.clone_from(&detection.language_id);
                true
            } else {
                false
            }
        };
        if should_apply {
            editor_language::apply_detection(&self.text_buffer, detection);
            self.sync_presentation();
        }
    }

    pub(super) fn saved_file(&self) -> Option<gio::File> {
        self.state.borrow().document.path().map(gio::File::for_path)
    }

    pub(super) fn source_file(&self) -> Option<sourceview5::File> {
        self.state.borrow().source_file.clone()
    }

    pub(super) fn apply_loaded_document(
        self: &Rc<Self>,
        document: LoadedDocument,
        snapshot: Option<&ReloadSnapshot>,
    ) {
        self.exit_compare();
        {
            let mut state = self.state.borrow_mut();
            state.document = DocumentState::from_loaded_with_display_path(
                document.path,
                document.display_path,
                document.format.clone(),
            );
            state.saved_format = document.format.clone();
            state.source_file = Some(document.source_file);
            state.pending_external = PendingExternalState::Idle;
            state.safety.writability = Writability::Unknown;
            state.safety.autosave_paused = None;
            state.ui.external_prompt_active = false;
            state.ui.visible_banner = crate::editor_tab::VisibleBannerState::None;
            state.content_type = None;
            state.language_id = None;
            state.ui.suppress_changes = true;
        }
        self.replace_buffer_text(&document.text, document.format.implicit_trailing_newline());
        if let Some(snapshot) = snapshot {
            snapshot.apply(&self.text_buffer);
        }
        self.text_buffer.set_modified(false);
        self.state.borrow_mut().ui.suppress_changes = false;
        self.set_attention(false);
        self.set_banner_revealed(false);
        if let Some(file) = self.saved_file() {
            self.refresh_writability_for_file(&file);
        }
    }

    pub(super) fn apply_saved_document(self: &Rc<Self>, saved: SavedDocument) {
        let saved_uri = saved.uri.clone();
        let saved_file = gio::File::for_path(&saved.path);
        let mut state = self.state.borrow_mut();
        state
            .document
            .set_saved_with_display_path(saved.path, saved.display_path.clone());
        state.document.set_format(saved.format.clone());
        state.saved_format = saved.format.clone();
        state.source_file = Some(saved.source_file);
        state.safety.writability = Writability::Writable;
        state.safety.autosave_paused = None;
        self.text_buffer
            .set_implicit_trailing_newline(saved.format.implicit_trailing_newline());
        self.text_buffer.set_modified(false);
        drop(state);
        self.sync_compare_reference_after_save(&saved_uri);
        self.refresh_writability_for_file(&saved_file);
    }

    pub(super) fn refresh_writability_for_file(self: &Rc<Self>, file: &gio::File) {
        let expected_uri = file.uri().to_string();
        let (generation, cancellable) = {
            let mut state = self.state.borrow_mut();
            if let Some(cancellable) = state.safety.writability_cancellable.take() {
                cancellable.cancel();
            }
            state.safety.writability_generation += 1;
            let generation = state.safety.writability_generation;
            let cancellable = gio::Cancellable::new();
            state.safety.writability_cancellable = Some(cancellable.clone());
            (generation, cancellable)
        };
        let weak = Rc::downgrade(self);
        file.query_info_async(
            "access::can-write",
            gio::FileQueryInfoFlags::NONE,
            gtk4::glib::Priority::default(),
            Some(&cancellable),
            move |result| {
                let Some(tab) = weak.upgrade() else {
                    return;
                };
                let writability = match result {
                    Ok(info) if info.boolean("access::can-write") => Writability::Writable,
                    Ok(_) => Writability::Unwritable,
                    Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => return,
                    Err(_) => Writability::Unknown,
                };
                let should_apply = {
                    let mut state = tab.state.borrow_mut();
                    if state.safety.writability_generation != generation
                        || state.document.uri().as_deref() != Some(expected_uri.as_str())
                    {
                        return;
                    }
                    state.safety.writability_cancellable = None;
                    state.safety.writability = writability;
                    true
                };
                if should_apply {
                    tab.sync_external_banner(true, true);
                    tab.notify_external_state_change();
                }
            },
        );
    }

    pub(super) fn can_apply_reload(
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

    pub(super) fn finish_reload(&self, applied: bool) {
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

    fn replace_buffer_text(&self, text: &str, implicit_trailing_newline: bool) {
        let undo_enabled = self.text_buffer.enables_undo();
        self.text_buffer.set_enable_undo(false);
        self.text_buffer
            .set_implicit_trailing_newline(implicit_trailing_newline);
        self.text_buffer.set_text(text);
        self.text_buffer.set_enable_undo(undo_enabled);
    }

    pub(super) fn start_io_request(
        &self,
        candidate_encodings: Option<SList<sourceview5::Encoding>>,
    ) -> (u64, gio::Cancellable) {
        let new_cancellable = gio::Cancellable::new();
        let generation = {
            let mut state = self.state.borrow_mut();
            if let Some(cancellable) = state.io.cancellable.take() {
                cancellable.cancel();
            }
            state.io.generation += 1;
            state.io.cancellable = Some(new_cancellable.clone());
            state.io.candidate_encodings = candidate_encodings;
            state.io.generation
        };
        (generation, new_cancellable)
    }

    pub(super) fn finish_io_request(&self, generation: u64) -> bool {
        let mut state = self.state.borrow_mut();
        if state.io.generation != generation {
            return false;
        }
        state.io.cancellable = None;
        state.io.candidate_encodings = None;
        true
    }

    pub(super) fn with_io_candidate_encodings<T>(
        &self,
        apply: impl FnOnce(Option<&SList<sourceview5::Encoding>>) -> T,
    ) -> T {
        let state = self.state.borrow();
        apply(state.io.candidate_encodings.as_ref())
    }

    fn set_pending_external(&self, pending: PendingExternalState) {
        let is_idle = {
            let mut state = self.state.borrow_mut();
            state.pending_external = pending;
            state.pending_external.is_idle()
        };
        self.set_attention(!is_idle);
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
