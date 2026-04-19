use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use gettextrs::pgettext;
use gtk4::{gdk, gio, prelude::*};
use libadwaita as adw;
use sourceview5::prelude::*;

use crate::document::DocumentState;
use crate::editor_monitor::{MonitorBinding, PendingExternalState};
use crate::editor_view::EditorView;
use crate::error::AppError;
use crate::settings::AppSettings;

mod runtime;

#[cfg(test)]
use crate::editor_monitor::ExternalFileEvent;

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
}

struct EditorTabState {
    document: DocumentState,
    content_type: Option<String>,
    language_id: Option<String>,
    monitor: Option<MonitorBinding>,
    pending_external: PendingExternalState,
    progress: ProgressState,
    ui: UiState,
    language_request_generation: u64,
}

#[derive(Default)]
struct ProgressState {
    loading: bool,
    external_reload_in_progress: bool,
}

#[derive(Default)]
struct UiState {
    suppress_changes: bool,
    external_prompt_active: bool,
    banner_syncing: bool,
}

pub struct EditorTab {
    root: gtk4::Box,
    banner: adw::Banner,
    minimap_holder: gtk4::Box,
    scrolled: gtk4::ScrolledWindow,
    text_view: sourceview5::View,
    text_buffer: sourceview5::Buffer,
    settings: AppSettings,
    state: RefCell<EditorTabState>,
    page: OnceCell<adw::TabPage>,
    on_file_drop: OnceCell<Rc<dyn Fn(Vec<gio::File>)>>,
    on_visual_change: OnceCell<Rc<dyn Fn()>>,
    on_external_state_change: OnceCell<Rc<dyn Fn()>>,
    on_external_action: OnceCell<Rc<dyn Fn()>>,
}

impl EditorTab {
    #[must_use]
    pub fn new(settings: &AppSettings) -> Rc<Self> {
        let view = EditorView::new(settings);
        let tab = Rc::new(Self {
            root: view.root,
            banner: view.banner,
            minimap_holder: view.minimap_holder,
            scrolled: view.scrolled,
            text_view: view.text_view,
            text_buffer: view.text_buffer,
            settings: settings.clone(),
            state: RefCell::new(EditorTabState {
                document: DocumentState::new_empty(),
                content_type: None,
                language_id: None,
                monitor: None,
                pending_external: PendingExternalState::Idle,
                progress: ProgressState::default(),
                ui: UiState::default(),
                language_request_generation: 0,
            }),
            page: OnceCell::new(),
            on_file_drop: OnceCell::new(),
            on_visual_change: OnceCell::new(),
            on_external_state_change: OnceCell::new(),
            on_external_action: OnceCell::new(),
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

    pub fn set_external_state_handler(&self, callback: Rc<dyn Fn()>) {
        let _set_callback = self.on_external_state_change.set(callback);
    }

    pub fn set_external_action_handler(&self, callback: Rc<dyn Fn()>) {
        let _set_callback = self.on_external_action.set(callback);
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
    pub fn is_loading(&self) -> bool {
        self.state.borrow().progress.loading
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

    pub fn apply_minimap_visibility(&self) {
        let show_minimap = self.settings.show_minimap();
        self.minimap_holder.set_visible(show_minimap);
        let policy = if show_minimap {
            gtk4::PolicyType::External
        } else {
            gtk4::PolicyType::Automatic
        };
        self.scrolled.set_vscrollbar_policy(policy);
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

    #[must_use]
    pub fn language_id(&self) -> Option<String> {
        self.state.borrow().language_id.clone()
    }

    #[must_use]
    pub fn pending_external_state(&self) -> PendingExternalState {
        self.state.borrow().pending_external.clone()
    }

    #[must_use]
    pub fn should_present_dirty_reload_prompt(&self) -> bool {
        let state = self.state.borrow();
        self.is_dirty()
            && matches!(
                state.pending_external,
                PendingExternalState::ContentPossiblyChanged {
                    acknowledged: false
                }
            )
            && !state.ui.external_prompt_active
    }

    #[must_use]
    pub fn banner_action_kind(&self) -> Option<BannerActionKind> {
        match self.state.borrow().pending_external {
            PendingExternalState::ContentPossiblyChanged {
                acknowledged: false,
            } if !self.is_dirty() => Some(BannerActionKind::Reload),
            PendingExternalState::Missing {
                acknowledged: false,
            } => Some(BannerActionKind::Save),
            _ => None,
        }
    }

    #[must_use]
    pub fn should_show_stale_save_conflict(&self) -> bool {
        self.state.borrow().pending_external.is_content_changed()
    }

    #[must_use]
    pub fn should_auto_reload(&self, is_selected: bool, _window_active: bool) -> bool {
        !self.is_dirty() && !self.is_loading() && !is_selected
    }

    #[must_use]
    pub fn monitor_target_matches_current(&self) -> bool {
        let current = self.uri();
        let target = self
            .state
            .borrow()
            .monitor
            .as_ref()
            .map(|binding| binding.target_uri().to_string());
        current == target
    }

    pub fn mark_external_prompt_active(&self, active: bool) {
        self.state.borrow_mut().ui.external_prompt_active = active;
    }

    pub fn acknowledge_pending_external(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.pending_external.acknowledge();
            state.ui.external_prompt_active = false;
        }
        self.sync_external_banner(true, true);
        self.notify_external_state_change();
    }

    pub fn resolve_pending_external(&self) {
        {
            let mut state = self.state.borrow_mut();
            state.pending_external = PendingExternalState::Idle;
            state.ui.external_prompt_active = false;
            state.progress.external_reload_in_progress = false;
        }
        self.set_attention(false);
        self.set_banner_revealed(false);
        self.notify_external_state_change();
    }

    pub fn sync_external_banner(&self, is_selected: bool, window_active: bool) {
        let (title, action) = match self.state.borrow().pending_external.clone() {
            PendingExternalState::ContentPossiblyChanged {
                acknowledged: false,
            } if !self.is_dirty() && is_selected && window_active => (
                Some(pgettext("external banner", "This File Changed on Disk.")),
                Some(pgettext("external action", "Reload")),
            ),
            PendingExternalState::Missing {
                acknowledged: false,
            } if is_selected => (
                Some(pgettext("external banner", "This File Is Missing on Disk.")),
                Some(pgettext("external action", "Save")),
            ),
            PendingExternalState::Idle
            | PendingExternalState::Moved { .. }
            | PendingExternalState::ContentPossiblyChanged { .. }
            | PendingExternalState::Missing { .. } => (None, None),
        };

        if let Some(title) = title {
            self.banner.set_title(&title);
            self.banner.set_button_label(action.as_deref());
            self.set_banner_revealed(true);
        } else {
            self.banner.set_button_label(None);
            self.set_banner_revealed(false);
        }
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

    #[cfg(test)]
    pub(crate) fn minimap_visible_for_tests(&self) -> bool {
        self.minimap_holder.property::<bool>("visible")
    }

    #[cfg(test)]
    pub(crate) fn banner_visible_for_tests(&self) -> bool {
        self.banner.is_revealed()
    }

    #[cfg(test)]
    pub(crate) fn sync_banner_for_tests(&self, is_selected: bool, window_active: bool) {
        self.sync_external_banner(is_selected, window_active);
    }

    #[cfg(test)]
    pub(crate) fn trigger_external_action_for_tests(&self) {
        if let Some(callback) = self.on_external_action.get() {
            callback();
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_external_event_for_tests(self: &Rc<Self>, event: ExternalFileEvent) {
        self.handle_external_event(event);
    }

    fn install_callbacks(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.text_buffer.connect_changed(move |_| {
            if let Some(tab) = weak.upgrade()
                && !tab.state.borrow().ui.suppress_changes
            {
                tab.sync_presentation();
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
            if let Some(tab) = weak.upgrade()
                && let Some(callback) = tab.on_visual_change.get()
            {
                callback();
            }
        });

        let weak = Rc::downgrade(self);
        self.banner.connect_button_clicked(move |_| {
            if let Some(tab) = weak.upgrade()
                && let Some(callback) = tab.on_external_action.get()
            {
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
                !state.ui.banner_syncing && !state.pending_external.is_idle()
            };
            if should_ack {
                tab.acknowledge_pending_external();
            }
        });

        let weak = Rc::downgrade(self);
        install_file_drop_target(&self.root, &weak);
        install_file_drop_target(&self.text_view, &weak);
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
