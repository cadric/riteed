use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gio, glib::SList, prelude::*};

use super::{
    VisibleBannerState, Writability,
    compare::{CompareController, ReviewSession},
    minimap_diff::MinimapDiffAttachment,
};
use crate::document::DocumentState;
use crate::editor_format::SavedTextFormat;
use crate::editor_monitor::{MonitorBinding, PendingExternalState};
use crate::large_file::viewer::LargeFileViewer;

#[derive(Default)]
pub(super) struct EditorTabState {
    pub(super) document: DocumentRuntimeState,
    pub(super) io: EditorIoState,
    pub(super) external: ExternalFileState,
    pub(super) autosave: AutosaveState,
    pub(super) large_file: LargeFileAttachment,
    pub(super) compare: CompareAttachment,
    pub(super) review: ReviewAttachment,
    pub(super) ui: UiState,
    dirty_generation: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum DocumentSurface {
    #[default]
    Editor,
    LargeFileViewer,
    RestorePlaceholder,
}

impl EditorTabState {
    #[must_use]
    pub(super) fn is_dirty(&self, buffer_modified: bool) -> bool {
        buffer_modified || self.document.format_is_dirty()
    }

    #[must_use]
    pub(super) const fn dirty_generation(&self) -> u64 {
        self.dirty_generation
    }

    pub(super) fn mark_dirty_generation(&mut self) {
        self.dirty_generation = self.dirty_generation.saturating_add(1);
    }
}

#[derive(Default)]
pub(super) struct LargeFileAttachment {
    pub(super) surface: DocumentSurface,
    pub(super) widget: Option<gtk4::Widget>,
    pub(super) viewer: Option<Rc<LargeFileViewer>>,
    pub(super) file_size: Option<u64>,
}

impl LargeFileAttachment {
    pub(super) fn cancel_operations(&self) {
        if let Some(viewer) = self.viewer.as_ref() {
            viewer.cancel();
        }
    }

    pub(super) fn clear_surface(&mut self) -> Option<gtk4::Widget> {
        if let Some(viewer) = self.viewer.take() {
            viewer.cancel();
        }
        let widget = self.widget.take();
        self.surface = DocumentSurface::Editor;
        self.file_size = None;
        widget
    }
}

pub(super) struct DocumentRuntimeState {
    pub(super) document: DocumentState,
    pub(super) saved_format: SavedTextFormat,
    pub(super) source_file: Option<sourceview5::File>,
    pub(super) content_type: Option<String>,
    pub(super) language_id: Option<String>,
    pub(super) language_request_generation: u64,
}

impl DocumentRuntimeState {
    #[must_use]
    pub(super) fn format_is_dirty(&self) -> bool {
        self.document.format() != &self.saved_format
    }

    #[must_use]
    pub(super) fn next_language_request_generation(&mut self) -> u64 {
        self.language_request_generation = self.language_request_generation.saturating_add(1);
        self.language_request_generation
    }
}

impl Default for DocumentRuntimeState {
    fn default() -> Self {
        Self {
            document: DocumentState::new_empty(),
            saved_format: SavedTextFormat::new_document_defaults(),
            source_file: None,
            content_type: None,
            language_id: None,
            language_request_generation: 0,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct PendingApplyRestore {
    pub(super) editable: bool,
    pub(super) undo: bool,
}

pub(super) struct PendingApplySource {
    pub(super) source: gtk4::glib::SourceId,
    pub(super) restore: PendingApplyRestore,
    pub(super) on_cancelled: Rc<dyn Fn()>,
}

#[derive(Default)]
pub(super) struct EditorIoState {
    pub(super) generation: u64,
    pub(super) cancellable: Option<gio::Cancellable>,
    pub(super) candidate_encodings: Option<SList<sourceview5::Encoding>>,
    pub(super) loading: bool,
    pub(super) external_reload_in_progress: bool,
    pub(super) pending_apply: Option<PendingApplySource>,
}

impl EditorIoState {
    pub(super) fn take_pending_apply(&mut self) -> Option<PendingApplySource> {
        self.pending_apply.take()
    }

    pub(super) fn cancel_request(&mut self) -> Option<gio::Cancellable> {
        self.candidate_encodings = None;
        self.generation = self.generation.saturating_add(1);
        self.cancellable.take()
    }

    pub(super) fn start_request(
        &mut self,
        candidate_encodings: Option<SList<sourceview5::Encoding>>,
    ) -> (u64, gio::Cancellable) {
        if let Some(cancellable) = self.cancellable.take() {
            cancellable.cancel();
        }
        self.generation = self.generation.saturating_add(1);
        let cancellable = gio::Cancellable::new();
        self.cancellable = Some(cancellable.clone());
        self.candidate_encodings = candidate_encodings;
        (self.generation, cancellable)
    }

    pub(super) fn finish_request(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            return false;
        }
        self.cancellable = None;
        self.candidate_encodings = None;
        true
    }
}

pub(super) struct ExternalFileState {
    pub(super) monitor: Option<MonitorBinding>,
    pub(super) pending: PendingExternalState,
    pub(super) writability: Writability,
    pub(super) writability_generation: u64,
    pub(super) writability_cancellable: Option<gio::Cancellable>,
}

impl ExternalFileState {
    pub(super) fn cancel_writability_request(&mut self) -> Option<gio::Cancellable> {
        self.writability_cancellable.take()
    }

    pub(super) fn start_writability_request(&mut self) -> (u64, gio::Cancellable) {
        if let Some(cancellable) = self.writability_cancellable.take() {
            cancellable.cancel();
        }
        self.writability_generation = self.writability_generation.saturating_add(1);
        let cancellable = gio::Cancellable::new();
        self.writability_cancellable = Some(cancellable.clone());
        (self.writability_generation, cancellable)
    }
}

impl Default for ExternalFileState {
    fn default() -> Self {
        Self {
            monitor: None,
            pending: PendingExternalState::Idle,
            writability: Writability::Unknown,
            writability_generation: 0,
            writability_cancellable: None,
        }
    }
}

#[derive(Default)]
pub(super) struct AutosaveState {
    pub(super) generation: u64,
    pub(super) paused_message: Option<String>,
}

impl AutosaveState {
    #[must_use]
    pub(super) fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }

    pub(super) fn pause(&mut self, message: String) {
        self.paused_message = Some(message);
    }

    pub(super) fn clear_pause(&mut self) -> bool {
        let changed = self.paused_message.is_some();
        self.paused_message = None;
        changed
    }
}

#[derive(Default)]
pub(super) struct CompareAttachment {
    pub(super) active: Option<CompareController>,
    pub(super) request_generation: u64,
}

impl CompareAttachment {
    #[must_use]
    pub(super) fn next_generation(&mut self) -> u64 {
        self.request_generation = self.request_generation.saturating_add(1);
        self.request_generation
    }
}

#[derive(Default)]
pub(super) struct ReviewAttachment {
    pub(super) session: Option<Rc<RefCell<ReviewSession>>>,
    pub(super) load_cancellable: Option<gio::Cancellable>,
}

#[derive(Default)]
pub(super) struct MarkdownPreviewAttachment {
    pub(super) active: bool,
    pub(super) generation: u64,
    pub(super) debounce: Option<gtk4::glib::SourceId>,
    pub(super) links: Vec<crate::markdown::RenderedLink>,
}

#[derive(Default)]
pub(super) struct UiState {
    pub(super) suppress_changes: bool,
    pub(super) external_prompt_active: bool,
    pub(super) banner_syncing: bool,
    pub(super) visible_banner: VisibleBannerState,
    pub(super) markdown_preview: MarkdownPreviewAttachment,
    pub(super) minimap_diff: MinimapDiffAttachment,
    // 0 means "not applied yet"; readers fall back to the settings font.
    pub(super) scroll_past_end_floor: i32,
}

#[cfg(test)]
mod tests {
    use super::{
        AutosaveState, CompareAttachment, DocumentRuntimeState, EditorIoState, EditorTabState,
        ExternalFileState, PendingApplyRestore, PendingApplySource,
    };
    use crate::editor_format::LineEndingMode;
    use gtk4::prelude::*;

    #[test]
    fn document_runtime_tracks_saved_format_dirty_state() {
        let mut state = DocumentRuntimeState::default();
        assert!(!state.format_is_dirty());

        state.document.set_line_ending_mode(LineEndingMode::CrLf);
        assert!(state.format_is_dirty());

        state.saved_format = state.document.format().clone();
        assert!(!state.format_is_dirty());
    }

    #[test]
    fn editor_tab_state_tracks_dirty_generation() {
        let mut state = EditorTabState::default();
        assert_eq!(state.dirty_generation(), 0);

        state.mark_dirty_generation();

        assert_eq!(state.dirty_generation(), 1);
    }

    #[test]
    fn editor_io_rejects_stale_generation() {
        let mut state = EditorIoState::default();
        let (first_generation, first_cancellable) = state.start_request(None);
        let (second_generation, _) = state.start_request(None);

        assert!(first_cancellable.is_cancelled());
        assert!(!state.finish_request(first_generation));
        assert!(state.finish_request(second_generation));

        let (third_generation, third_cancellable) = state.start_request(None);
        assert_eq!(state.cancel_request(), Some(third_cancellable));
        assert!(!state.finish_request(third_generation));
    }

    #[test]
    fn editor_io_take_pending_apply_clears_stored_source() {
        let mut state = EditorIoState::default();
        assert!(state.take_pending_apply().is_none());

        let source = gtk4::glib::timeout_add_local_once(std::time::Duration::from_hours(1), || {});
        state.pending_apply = Some(PendingApplySource {
            source,
            restore: PendingApplyRestore {
                editable: true,
                undo: true,
            },
            on_cancelled: std::rc::Rc::new(|| {}),
        });
        let pending = state.take_pending_apply();
        assert!(pending.is_some());
        if let Some(pending) = pending {
            pending.source.remove();
        }
        assert!(state.take_pending_apply().is_none());
    }

    #[test]
    fn external_file_state_tracks_writability_generation() {
        let mut state = ExternalFileState::default();
        let (first_generation, first_cancellable) = state.start_writability_request();
        let (second_generation, _) = state.start_writability_request();

        assert!(first_cancellable.is_cancelled());
        assert_ne!(first_generation, second_generation);
    }

    #[test]
    fn autosave_state_tracks_generation_and_pause() {
        let mut state = AutosaveState::default();
        assert_eq!(state.next_generation(), 1);
        state.pause(String::from("paused"));
        assert_eq!(state.paused_message.as_deref(), Some("paused"));
        assert!(state.clear_pause());
        assert!(!state.clear_pause());
    }

    #[test]
    fn compare_attachment_tracks_request_generation() {
        let mut state = CompareAttachment::default();
        assert_eq!(state.next_generation(), 1);
        assert_eq!(state.next_generation(), 2);
    }
}
