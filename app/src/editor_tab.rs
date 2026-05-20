use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gdk, gio, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::editor_monitor::PendingExternalState;
use crate::editor_view::EditorView;
use crate::error::AppError;
use crate::settings::AppSettings;

mod banner;
mod compare;
mod open;
mod review;
mod runtime;
mod save;
mod state;
mod view;

pub(crate) use compare::{ReviewFileInput, ReviewScrollTarget};
pub use review::{
    ReviewFileId, ReviewFileSpec, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec, TabKind,
};
use state::EditorTabState;

#[cfg(test)]
pub(crate) fn compare_row_count_for_texts_for_tests(
    editable_text: &str,
    reference_text: &str,
) -> usize {
    compare::row_count_for_texts_for_tests(editable_text, reference_text)
}

type FileDropHandler = Rc<dyn Fn(Vec<gio::File>)>;
type TabCallback = Rc<dyn Fn()>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SaveOutcome {
    pub old_uri: Option<String>,
    pub new_uri: String,
}

#[derive(Clone, Debug)]
pub enum SaveResult {
    Saved(SaveOutcome),
    CancelledByUser,
    Failed(AppError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveKind {
    Manual,
    Autosave,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReloadCause {
    Automatic,
    UserRequested,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReloadResult {
    Applied,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BannerActionKind {
    Reload,
    Save,
    SaveAs,
    RefreshReview,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Writability {
    #[default]
    Unknown,
    Writable,
    Unwritable,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VisibleBannerState {
    External,
    Missing,
    ReadOnly,
    AutosavePaused,
    ReviewStale,
    #[default]
    None,
}

pub struct EditorTab {
    root: gtk4::Box,
    banner: adw::Banner,
    minimap: sourceview5::Map,
    minimap_holder: gtk4::Box,
    content: gtk4::Box,
    scrolled: gtk4::ScrolledWindow,
    text_view: sourceview5::View,
    text_buffer: sourceview5::Buffer,
    settings: AppSettings,
    kind: TabKind,
    review_spec: Option<ReviewTabSpec>,
    state: RefCell<EditorTabState>,
    page: OnceCell<adw::TabPage>,
    on_file_drop: RefCell<Option<FileDropHandler>>,
    on_visual_change: RefCell<Option<TabCallback>>,
    on_external_state_change: RefCell<Option<TabCallback>>,
    on_external_action: RefCell<Option<TabCallback>>,
    preview_buffer: gtk4::TextBuffer,
    preview_view: gtk4::TextView,
    preview_scrolled: gtk4::ScrolledWindow,
}

impl EditorTab {
    #[must_use]
    pub fn new(settings: &AppSettings) -> Rc<Self> {
        Self::new_document(settings)
    }

    #[must_use]
    pub fn new_document(settings: &AppSettings) -> Rc<Self> {
        Self::build(settings, TabKind::Document, None)
    }

    #[must_use]
    pub fn new_git_review(settings: &AppSettings, spec: ReviewTabSpec) -> Rc<Self> {
        let tab = Self::build(settings, TabKind::GitReview, Some(spec));
        if let Some(spec) = tab.review_spec().cloned() {
            tab.state.borrow_mut().review.session = Some(Rc::new(RefCell::new(
                compare::ReviewSession::from_spec(&spec),
            )));
        }
        let toolbar = compare::review_toolbar();
        tab.root.insert_child_after(&toolbar, Some(&tab.banner));
        tab.text_view.set_editable(false);
        let wrap_mode = if settings.compare_word_wrap() {
            gtk4::WrapMode::WordChar
        } else {
            gtk4::WrapMode::None
        };
        tab.text_view.set_wrap_mode(wrap_mode);
        tab.text_buffer.set_modified(false);
        tab.sync_presentation();
        tab
    }

    fn build(
        settings: &AppSettings,
        kind: TabKind,
        review_spec: Option<ReviewTabSpec>,
    ) -> Rc<Self> {
        let view = EditorView::new(settings);
        let tab = Rc::new(Self {
            root: view.root,
            banner: view.banner,
            minimap: view.minimap,
            minimap_holder: view.minimap_holder,
            content: view.content,
            scrolled: view.scrolled,
            text_view: view.text_view,
            text_buffer: view.text_buffer,
            settings: settings.clone(),
            kind,
            review_spec,
            state: RefCell::new(EditorTabState::default()),
            page: OnceCell::new(),
            on_file_drop: RefCell::new(None),
            on_visual_change: RefCell::new(None),
            on_external_state_change: RefCell::new(None),
            on_external_action: RefCell::new(None),
            preview_buffer: view.preview_buffer,
            preview_view: view.preview_view,
            preview_scrolled: view.preview_scrolled,
        });
        tab.install_callbacks();
        tab.apply_minimap_visibility();
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
        self.on_visual_change.replace(Some(callback));
    }

    pub fn set_file_drop_handler(&self, callback: Rc<dyn Fn(Vec<gio::File>)>) {
        self.on_file_drop.replace(Some(callback));
    }

    pub fn set_external_state_handler(&self, callback: Rc<dyn Fn()>) {
        self.on_external_state_change.replace(Some(callback));
    }

    pub fn set_external_action_handler(&self, callback: Rc<dyn Fn()>) {
        self.on_external_action.replace(Some(callback));
    }

    #[must_use]
    pub fn page(&self) -> Option<adw::TabPage> {
        self.page.get().cloned()
    }

    #[must_use]
    pub fn uri(&self) -> Option<String> {
        self.document_uri()
    }

    #[must_use]
    pub fn document_uri(&self) -> Option<String> {
        if !self.is_document() {
            return None;
        }
        self.state.borrow().document.document.uri()
    }

    #[must_use]
    pub fn kind(&self) -> TabKind {
        self.kind
    }

    #[must_use]
    pub fn is_document(&self) -> bool {
        self.kind == TabKind::Document
    }

    #[must_use]
    pub fn review_spec(&self) -> Option<&ReviewTabSpec> {
        self.review_spec.as_ref()
    }

    #[must_use]
    pub fn is_dirty(&self) -> bool {
        if !self.is_document() {
            return false;
        }
        self.state.borrow().is_dirty(self.text_buffer.is_modified())
    }

    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.state.borrow().io.loading
    }

    #[must_use]
    pub fn is_autosave_eligible(&self) -> bool {
        if !self.is_document() {
            return false;
        }
        let state = self.state.borrow();
        let is_dirty = state.is_dirty(self.text_buffer.is_modified());
        self.settings.autosave_enabled()
            && is_dirty
            && state.document.document.path().is_some()
            && state.external.pending.is_idle()
            && state.external.writability == Writability::Writable
            && !state.io.loading
            && state.compare.active.is_none()
    }

    #[must_use]
    pub fn next_autosave_generation(&self) -> u64 {
        self.state.borrow_mut().autosave.next_generation()
    }

    #[must_use]
    pub fn autosave_generation(&self) -> u64 {
        self.state.borrow().autosave.generation
    }

    #[must_use]
    pub fn is_clean_untitled(&self) -> bool {
        self.is_document()
            && self.state.borrow().document.document.path().is_none()
            && !self.is_dirty()
    }

    #[must_use]
    pub fn title(&self) -> String {
        if let Some(spec) = self.review_spec() {
            return match spec.review_kind {
                ReviewKind::Staged => pgettext("document title", "Staged Changes Review"),
                ReviewKind::Unstaged => pgettext("document title", "Unstaged Changes Review"),
            };
        }
        self.state
            .borrow()
            .document
            .document
            .file_name()
            .unwrap_or_else(|| pgettext("document title", "Untitled"))
    }

    #[must_use]
    pub fn subtitle(&self) -> String {
        if let Some(spec) = self.review_spec() {
            return spec.repo_root.display().to_string();
        }
        self.state
            .borrow()
            .document
            .document
            .path_display()
            .unwrap_or_else(|| pgettext("document subtitle", "Plain Text Document"))
    }

    #[must_use]
    pub fn path_display(&self) -> Option<String> {
        if !self.is_document() {
            return None;
        }
        self.state.borrow().document.document.path_display()
    }

    #[must_use]
    pub fn save_name_suggestion(&self) -> String {
        if !self.is_document() {
            return pgettext("save file name", "Source Control Review");
        }
        self.state
            .borrow()
            .document
            .document
            .file_name()
            .unwrap_or_else(|| pgettext("save file name", "Untitled"))
    }

    #[must_use]
    pub fn buffer_text(&self) -> String {
        let start = self.text_buffer.start_iter();
        let end = self.text_buffer.end_iter();
        self.text_buffer.text(&start, &end, true).to_string()
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
    pub fn supports_search(&self) -> bool {
        crate::document_limits::buffer_supports_search(&self.text_buffer)
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

    #[must_use]
    pub fn language_id(&self) -> Option<String> {
        self.state.borrow().document.language_id.clone()
    }

    #[must_use]
    pub fn pending_external_state(&self) -> PendingExternalState {
        self.state.borrow().external.pending.clone()
    }

    #[must_use]
    pub fn writability(&self) -> Writability {
        self.state.borrow().external.writability
    }

    #[cfg(test)]
    pub fn set_writability_for_tests(&self, writability: Writability) {
        self.state.borrow_mut().external.writability = writability;
    }

    #[must_use]
    pub fn should_show_stale_save_conflict(&self) -> bool {
        self.state.borrow().external.pending.is_content_changed()
    }

    #[must_use]
    pub fn should_auto_reload(&self, is_selected: bool, window_active: bool) -> bool {
        self.is_document()
            && !self.is_dirty()
            && !self.is_loading()
            && !self.is_compare_active()
            && (!is_selected || !window_active)
    }

    #[must_use]
    pub fn monitor_target_matches_current(&self) -> bool {
        let current = self.document_uri();
        let target = self
            .state
            .borrow()
            .external
            .monitor
            .as_ref()
            .map(|binding| binding.target_uri().to_string());
        current == target
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.text_buffer.connect_changed(move |_| {
            if let Some(tab) = weak.upgrade()
                && !tab.state.borrow().ui.suppress_changes
            {
                tab.sync_presentation();
                tab.schedule_markdown_preview_update();
            }
        });

        let weak = Rc::downgrade(self);
        self.text_buffer.connect_modified_changed(move |_| {
            if let Some(tab) = weak.upgrade()
                && !tab.state.borrow().ui.suppress_changes
            {
                tab.sync_presentation();
            }
        });

        let weak = Rc::downgrade(self);
        self.text_buffer.connect_cursor_moved(move |_| {
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let callback = tab.on_visual_change.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        });

        let weak = Rc::downgrade(self);
        self.banner.connect_button_clicked(move |_| {
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let callback = tab.on_external_action.borrow().clone();
            if let Some(callback) = callback {
                callback();
            }
        });

        let weak = Rc::downgrade(self);
        self.banner.connect_revealed_notify(move |banner| {
            if banner.is_revealed() {
                return;
            }
            let Some(tab) = weak.upgrade() else {
                return;
            };
            let should_ack = {
                let state = tab.state.borrow();
                !state.ui.banner_syncing
                    && matches!(
                        state.ui.visible_banner,
                        VisibleBannerState::External | VisibleBannerState::Missing
                    )
            };
            if should_ack {
                tab.acknowledge_pending_external();
            }
        });

        let weak = Rc::downgrade(self);
        install_file_drop_target(&self.root, &weak);
        install_file_drop_target(&self.text_view, &weak);
        install_file_drop_target(&self.preview_view, &weak);
        self.install_markdown_preview_link_handler();
    }

    fn sync_presentation(&self) {
        if let Some(page) = self.page() {
            page.set_title(&self.title());
            page.set_tooltip(&self.subtitle());
            if self.is_dirty() {
                let icon = gio::ThemedIcon::new("document-edit-symbolic");
                page.set_indicator_icon(Some(&icon));
            } else {
                page.set_indicator_icon(Option::<&gio::Icon>::None);
            }
        }

        let callback = self.on_visual_change.borrow().clone();
        if let Some(callback) = callback {
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
        let handler = tab.on_file_drop.borrow().clone();
        let Some(handler) = handler else {
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

    let markdown_filter = gtk4::FileFilter::new();
    markdown_filter.set_name(Some(&pgettext("file filter", "Markdown Source Files")));
    markdown_filter.add_mime_type("text/markdown");
    markdown_filter.add_suffix("md");
    markdown_filter.add_suffix("markdown");

    let any_filter = gtk4::FileFilter::new();
    any_filter.set_name(Some(&pgettext("file filter", "All Files")));
    any_filter.add_pattern("*");

    let filters: gio::ListStore = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&text_filter);
    filters.append(&markdown_filter);
    filters.append(&any_filter);

    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&text_filter));
}
