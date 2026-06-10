use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::{gio, glib::SList, prelude::*};
use sourceview5::prelude::*;

use crate::document::DocumentState;
use crate::editor_io::{LoadedDocument, SavedDocument};
use crate::editor_language::{self, LanguageDetection};
use crate::editor_monitor::PendingExternalState;
use crate::editor_tab::{EditorTab, ReloadCause, Writability};
use crate::editor_view::ReloadSnapshot;
use crate::large_file::usize_to_u64;

use super::DocumentSurface;

impl EditorTab {
    pub fn cancel_io(&self) {
        let (cancellable, pending_apply) = {
            let mut state = self.state.borrow_mut();
            let pending_apply = state.io.take_pending_apply();
            (state.io.cancel_request(), pending_apply)
        };
        if let Some(pending_apply) = pending_apply {
            self.cancel_pending_apply(pending_apply);
        }
        if let Some(cancellable) = cancellable {
            cancellable.cancel();
        }
        if let Some(cancellable) = self
            .state
            .borrow_mut()
            .external
            .cancel_writability_request()
        {
            cancellable.cancel();
        }
        self.state.borrow().large_file.cancel_operations();
        self.cancel_review_load();
    }

    #[must_use]
    pub fn current_format(&self) -> crate::editor_format::SavedTextFormat {
        self.state.borrow().document.document.format().clone()
    }

    #[must_use]
    pub fn current_format_summary(&self) -> String {
        self.state.borrow().document.document.format().summary()
    }

    #[must_use]
    pub fn current_line_ending_mode(&self) -> crate::editor_format::LineEndingMode {
        self.state
            .borrow()
            .document
            .document
            .format()
            .line_ending_mode()
    }

    #[must_use]
    pub fn can_reopen_with_encoding(&self) -> bool {
        self.is_document()
            && self.saved_file().is_some()
            && !self.pending_external_state().is_missing()
            && !self.is_loading()
            && !self.state.borrow().ui.external_prompt_active
    }

    pub fn set_current_line_ending_mode(
        &self,
        line_ending_mode: crate::editor_format::LineEndingMode,
    ) {
        if !self.is_document() {
            return;
        }
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.document.document.format().line_ending_mode() == line_ending_mode {
                false
            } else {
                state
                    .document
                    .document
                    .set_line_ending_mode(line_ending_mode);
                state.mark_dirty_generation();
                true
            }
        };
        if changed {
            self.sync_presentation();
        }
    }

    pub fn set_current_encoding(&self, encoding: crate::editor_format::EncodingInfo) {
        if !self.is_document() {
            return;
        }
        let changed = {
            let mut state = self.state.borrow_mut();
            if state.document.document.format().encoding() == &encoding {
                false
            } else {
                state.document.document.set_encoding(encoding);
                state.mark_dirty_generation();
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
                    let moved_access_path = path_info.path.clone();
                    {
                        let mut state = self.state.borrow_mut();
                        state
                            .document
                            .document
                            .set_saved_with_display_path(path_info.path, path_info.display_path);
                        state.external.pending = PendingExternalState::Idle;
                    }
                    self.swap_monitor(&new_file);
                    self.refresh_language_for_file(&new_file);
                    self.refresh_writability_for_file(&new_file);
                    self.resolve_display_path_for_access_path(&moved_access_path);
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
            self.state.borrow_mut().external.monitor = Some(binding);
        }
    }

    pub fn clear_monitor(&self) {
        if let Some(binding) = self.state.borrow_mut().external.monitor.take() {
            binding.cancel();
        }
    }

    pub(super) fn refresh_language_for_file(self: &Rc<Self>, file: &gio::File) {
        let identity = file.uri().to_string();
        let generation = {
            let mut state = self.state.borrow_mut();
            state.document.next_language_request_generation()
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
        self: &Rc<Self>,
        generation: u64,
        identity: &str,
        detection: &LanguageDetection,
    ) {
        let (should_apply, heavy_features_enabled) = {
            let full_feature_limit = self.settings.large_file_thresholds().full_feature;
            let mut state = self.state.borrow_mut();
            let matches_request = state.document.language_request_generation == generation
                && state.document.document.uri().as_deref() == Some(identity);
            if matches_request {
                let heavy_features_enabled = state
                    .large_file
                    .file_size
                    .is_none_or(|size| size < full_feature_limit);
                state
                    .document
                    .content_type
                    .clone_from(&detection.content_type);
                if heavy_features_enabled {
                    state
                        .document
                        .language_id
                        .clone_from(&detection.language_id);
                } else {
                    state.document.language_id = None;
                }
                (true, heavy_features_enabled)
            } else {
                (false, false)
            }
        };
        if should_apply {
            if heavy_features_enabled {
                editor_language::apply_detection(&self.text_buffer, detection);
            } else {
                self.text_buffer
                    .set_language(Option::<&sourceview5::Language>::None);
            }
            self.apply_compare_style();
            self.sync_markdown_preview_availability();
            self.sync_presentation();
        }
    }

    pub(super) fn saved_file(&self) -> Option<gio::File> {
        self.state
            .borrow()
            .document
            .document
            .path()
            .map(gio::File::for_path)
    }

    pub(super) fn source_file(&self) -> Option<sourceview5::File> {
        self.state.borrow().document.source_file.clone()
    }

    pub(super) fn begin_loaded_document_state(self: &Rc<Self>, document: &LoadedDocument) {
        self.exit_markdown_preview();
        self.exit_compare();
        self.clear_large_file_surface();
        self.content.set_visible(true);
        let loaded_size = loaded_document_gate_size(document.disk_size, document.text.len());
        {
            let mut state = self.state.borrow_mut();
            state.large_file.surface = DocumentSurface::Editor;
            state.large_file.file_size = Some(loaded_size);
            state.document.document = DocumentState::from_loaded_with_display_path(
                document.path.clone(),
                document.display_path.clone(),
                document.format.clone(),
            );
            state.document.saved_format = document.format.clone();
            state.document.source_file = Some(document.source_file.clone());
            state.external.pending = PendingExternalState::Idle;
            state.external.writability = Writability::Unknown;
            state.autosave.paused_message = None;
            state.ui.external_prompt_active = false;
            state.ui.visible_banner = crate::editor_tab::VisibleBannerState::None;
            state.document.content_type = None;
            state.document.language_id = None;
            state.ui.suppress_changes = true;
        }
    }

    pub(super) fn finish_loaded_document_presentation(
        self: &Rc<Self>,
        document: &LoadedDocument,
        snapshot: Option<&ReloadSnapshot>,
    ) {
        if let Some(snapshot) = snapshot {
            snapshot.apply(&self.text_buffer);
        }
        self.text_buffer.set_modified(false);
        self.state.borrow_mut().ui.suppress_changes = false;
        self.apply_minimap_visibility();
        self.sync_markdown_preview_availability();
        if !self.editor_heavy_features_enabled() {
            self.clear_source_control_minimap_diff();
        }
        self.set_attention(false);
        self.set_banner_revealed(false);
        if let Some(file) = self.saved_file() {
            self.refresh_writability_for_file(&file);
        }
        self.resolve_display_path_for_access_path(&document.path);
    }

    pub(super) fn dirty_generation(&self) -> u64 {
        self.state.borrow().dirty_generation()
    }

    pub(super) fn apply_saved_document(self: &Rc<Self>, saved: SavedDocument, clear_dirty: bool) {
        let saved_uri = saved.uri.clone();
        let saved_access_path = saved.path.clone();
        let saved_file = gio::File::for_path(&saved.path);
        {
            let mut state = self.state.borrow_mut();
            state
                .document
                .document
                .set_saved_with_display_path(saved.path, saved.display_path.clone());
            if clear_dirty {
                state.document.document.set_format(saved.format.clone());
            }
            state.document.saved_format = saved.format.clone();
            state.document.source_file = Some(saved.source_file);
            state.external.writability = Writability::Writable;
            state.autosave.paused_message = None;
        }
        if clear_dirty {
            self.text_buffer
                .set_implicit_trailing_newline(saved.format.implicit_trailing_newline());
            self.text_buffer.set_modified(false);
        } else {
            self.text_buffer.set_modified(true);
        }
        self.sync_markdown_preview_availability();
        self.sync_compare_reference_after_save(&saved_uri);
        self.refresh_writability_for_file(&saved_file);
        self.resolve_display_path_for_access_path(&saved_access_path);
    }

    pub(super) fn refresh_writability_for_file(self: &Rc<Self>, file: &gio::File) {
        let expected_uri = file.uri().to_string();
        let (generation, cancellable) = {
            let mut state = self.state.borrow_mut();
            state.external.start_writability_request()
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
                    if state.external.writability_generation != generation
                        || state.document.document.uri().as_deref() != Some(expected_uri.as_str())
                    {
                        return;
                    }
                    state.external.writability_cancellable = None;
                    state.external.writability = writability;
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
        self.document_uri().as_deref() == Some(expected_uri)
            && self.monitor_target_matches_current()
            && match cause {
                ReloadCause::Automatic => !self.is_dirty() && should_apply(),
                ReloadCause::UserRequested => should_apply(),
            }
    }

    pub(super) fn finish_reload(&self, applied: bool) {
        {
            let mut state = self.state.borrow_mut();
            state.io.loading = false;
            state.io.external_reload_in_progress = false;
            if !applied && matches!(state.external.pending, PendingExternalState::Idle) {
                state.external.pending = PendingExternalState::ContentPossiblyChanged {
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

    pub(super) fn start_io_request(
        &self,
        candidate_encodings: Option<SList<sourceview5::Encoding>>,
    ) -> (u64, gio::Cancellable) {
        let (result, pending_apply) = {
            let mut state = self.state.borrow_mut();
            let pending_apply = state.io.take_pending_apply();
            (state.io.start_request(candidate_encodings), pending_apply)
        };
        if let Some(pending_apply) = pending_apply {
            self.cancel_pending_apply(pending_apply);
        }
        result
    }

    pub(super) fn finish_io_request(&self, generation: u64) -> bool {
        let mut state = self.state.borrow_mut();
        state.io.finish_request(generation)
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
            state.external.pending = pending;
            state.external.pending.is_idle()
        };
        self.set_attention(!is_idle);
        self.notify_external_state_change();
    }

    pub(super) fn notify_external_state_change(&self) {
        let callback = self.on_external_state_change.borrow().clone();
        if let Some(callback) = callback {
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
        self.state.borrow_mut().io.loading = loading;
        if let Some(page) = self.page() {
            page.set_loading(loading);
        }
    }

    fn resolve_display_path_for_access_path(self: &Rc<Self>, access_path: &Path) {
        crate::document_portal::resolve_display_path_async(access_path, {
            let weak = Rc::downgrade(self);
            let expected_path = access_path.to_path_buf();
            move |display_path| {
                let Some(display_path) = display_path else {
                    return;
                };
                if let Some(tab) = weak.upgrade() {
                    tab.apply_resolved_display_path(&expected_path, display_path);
                }
            }
        });
    }

    fn apply_resolved_display_path(&self, access_path: &Path, display_path: PathBuf) {
        let changed = {
            let mut state = self.state.borrow_mut();
            state
                .document
                .document
                .set_display_path_for_access_path(access_path, Some(display_path))
        };
        if changed {
            self.sync_presentation();
        }
    }
}

fn loaded_document_gate_size(disk_size: Option<u64>, decoded_len: usize) -> u64 {
    disk_size.unwrap_or_else(|| usize_to_u64(decoded_len))
}

#[cfg(test)]
mod tests {
    use super::loaded_document_gate_size;

    #[test]
    fn loaded_document_gate_size_prefers_disk_size_over_larger_decoded_size() {
        assert_eq!(loaded_document_gate_size(Some(4), 8), 4);
    }

    #[test]
    fn loaded_document_gate_size_prefers_disk_size_over_smaller_decoded_size() {
        assert_eq!(loaded_document_gate_size(Some(8), 4), 8);
    }

    #[test]
    fn loaded_document_gate_size_falls_back_to_decoded_size() {
        assert_eq!(loaded_document_gate_size(None, 7), 7);
    }
}
