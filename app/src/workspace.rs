use std::cell::{OnceCell, RefCell};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gdk, gio, glib, prelude::*};
use libadwaita as adw;

use crate::close_flow::CloseCoordinator;
use crate::editor_format::{EncodingInfo, LineEndingMode};
use crate::editor_search::{EditorSearch, SearchTarget};
use crate::editor_status::EditorStatusBar;
use crate::editor_tab::{EditorTab, SaveKind, SaveResult};
use crate::settings::AppSettings;

mod autosave;
mod recent;
mod selection;
mod session_state;
pub(crate) mod tabs;
#[cfg(test)]
mod testing;
#[cfg(test)]
pub(crate) use autosave::AutosaveRequestForTests;

type FormatPreferencesHandler = Rc<dyn Fn(Option<Rc<EditorTab>>)>;
type CompareActionSyncHandler = Rc<dyn Fn(Option<Rc<EditorTab>>)>;
type DocumentToolsSyncHandler = Rc<dyn Fn(Option<Rc<EditorTab>>)>;
type GitActionSyncHandler = Rc<dyn Fn(Option<Rc<EditorTab>>)>;
type ReviewRefreshHandler = Rc<dyn Fn(Rc<EditorTab>)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenSource {
    Dialog,
    AppOpen,
    Recent,
    SessionRestore,
    ProjectTree,
    SourceControl,
    Drop,
}

pub(crate) struct WorkspaceState {
    pub(crate) tabs: Vec<Rc<EditorTab>>,
    pub(crate) recent_files: Vec<String>,
    pub(crate) stored_session_files: Vec<String>,
    pub(crate) stored_selected_file: String,
    pub(crate) persist_session: bool,
    pub(crate) restoring_session: bool,
    pub(crate) close_flow: Option<Rc<CloseCoordinator>>,
    pub(crate) allow_window_close: bool,
}

pub struct Workspace {
    pub(crate) shell: adw::ApplicationWindow,
    pub(crate) title_widget: adw::WindowTitle,
    pub(crate) toast_overlay: adw::ToastOverlay,
    pub(crate) menu_button: gtk4::MenuButton,
    pub(crate) save_action: gio::SimpleAction,
    pub(crate) save_as_action: gio::SimpleAction,
    pub(crate) close_action: gio::SimpleAction,
    pub(crate) settings: AppSettings,
    pub(crate) tab_view: adw::TabView,
    #[cfg(test)]
    tab_bar: adw::TabBar,
    pub(crate) tab_controls: tabs::TabControls,
    pub(crate) search: Rc<EditorSearch>,
    pub(crate) status_bar: EditorStatusBar,
    format_preferences_handler: OnceCell<FormatPreferencesHandler>,
    compare_action_sync_handler: OnceCell<CompareActionSyncHandler>,
    document_tools_sync_handler: OnceCell<DocumentToolsSyncHandler>,
    git_action_sync_handler: OnceCell<GitActionSyncHandler>,
    review_refresh_handler: OnceCell<ReviewRefreshHandler>,
    save_notification_handler: OnceCell<Rc<dyn Fn(gio::File)>>,
    pub(crate) state: RefCell<WorkspaceState>,
}

#[derive(Clone, Copy)]
pub struct WorkspaceParts<'a> {
    pub shell: &'a adw::ApplicationWindow,
    pub toolbar_view: &'a adw::ToolbarView,
    pub title_widget: &'a adw::WindowTitle,
    pub toast_overlay: &'a adw::ToastOverlay,
    pub workspace_box: &'a gtk4::Box,
    pub menu_button: &'a gtk4::MenuButton,
    pub save_action: &'a gio::SimpleAction,
    pub save_as_action: &'a gio::SimpleAction,
    pub close_action: &'a gio::SimpleAction,
    pub settings: &'a AppSettings,
    pub persist_session: bool,
}

impl Workspace {
    #[must_use]
    pub fn new(parts: WorkspaceParts<'_>) -> Rc<Self> {
        let tab_view = adw::TabView::new();
        tab_view.set_hexpand(true);
        tab_view.set_shortcuts(adw::TabViewShortcuts::ALL_SHORTCUTS);
        tab_view.set_vexpand(true);

        let tab_bar = adw::TabBar::new();
        tab_bar.set_autohide(true);
        tab_bar.set_view(Some(&tab_view));
        parts.toolbar_view.add_top_bar(&tab_bar);
        parts.workspace_box.set_hexpand(true);
        parts.workspace_box.set_vexpand(true);
        parts.workspace_box.append(&tab_view);

        let search = EditorSearch::new(parts.shell);
        parts.toolbar_view.add_top_bar(search.widget());

        let status_bar = EditorStatusBar::new();
        parts.toolbar_view.add_bottom_bar(status_bar.widget());

        let workspace = Rc::new(Self {
            shell: parts.shell.clone(),
            title_widget: parts.title_widget.clone(),
            toast_overlay: parts.toast_overlay.clone(),
            menu_button: parts.menu_button.clone(),
            save_action: parts.save_action.clone(),
            save_as_action: parts.save_as_action.clone(),
            close_action: parts.close_action.clone(),
            settings: parts.settings.clone(),
            tab_view,
            #[cfg(test)]
            tab_bar,
            tab_controls: tabs::TabControls::new(),
            search,
            status_bar,
            format_preferences_handler: OnceCell::new(),
            compare_action_sync_handler: OnceCell::new(),
            document_tools_sync_handler: OnceCell::new(),
            git_action_sync_handler: OnceCell::new(),
            review_refresh_handler: OnceCell::new(),
            save_notification_handler: OnceCell::new(),
            state: RefCell::new(WorkspaceState {
                tabs: Vec::new(),
                recent_files: parts.settings.recent_files(),
                stored_session_files: parts.settings.session_files(),
                stored_selected_file: parts.settings.session_selected_file(),
                persist_session: parts.persist_session,
                restoring_session: false,
                close_flow: None,
                allow_window_close: false,
            }),
        });
        tabs::install(&workspace);
        workspace.install_callbacks(parts.workspace_box);
        workspace.rebuild_primary_menu();
        workspace.refresh_selected_state();
        workspace
    }

    pub fn ensure_default_tab(self: &Rc<Self>) {
        if self.tab_view.n_pages() == 0 {
            let _tab = self.add_empty_tab(true);
            self.refresh_selected_state();
            self.persist_session_state_if_needed();
        }
    }

    pub fn request_new_tab(self: &Rc<Self>) {
        let _tab = self.add_empty_tab(true);
        self.refresh_selected_state();
        self.persist_session_state_if_needed();
    }

    pub fn request_open_dialog(self: &Rc<Self>, parent: &adw::ApplicationWindow) {
        crate::workspace_open::request_open_dialog(self, parent);
    }

    pub fn request_open_recent(self: &Rc<Self>, uri: &str) {
        self.ensure_default_tab();
        self.request_open_files(vec![gio::File::for_uri(uri)], OpenSource::Recent);
    }

    pub fn request_open_files(self: &Rc<Self>, files: Vec<gio::File>, source: OpenSource) {
        crate::workspace_open::open_files_internal(self, files, source, None);
    }

    pub(crate) fn request_open_file_then(
        self: &Rc<Self>,
        file: &gio::File,
        source: OpenSource,
        callback: Rc<dyn Fn(Result<Rc<EditorTab>, crate::error::AppError>)>,
    ) {
        crate::workspace_open::request_open_file_then(self, file, source, callback);
    }

    pub fn restore_session(self: &Rc<Self>) {
        let (session_files, selected_uri) = {
            let state = self.state.borrow();
            (
                state.stored_session_files.clone(),
                if state.stored_selected_file.is_empty() {
                    None
                } else {
                    Some(state.stored_selected_file.clone())
                },
            )
        };
        if session_files.is_empty() {
            self.ensure_default_tab();
            return;
        }

        self.state.borrow_mut().restoring_session = true;
        self.ensure_default_tab();
        crate::workspace_open::open_files_internal(
            self,
            session_files
                .into_iter()
                .map(|uri| gio::File::for_uri(&uri))
                .collect(),
            OpenSource::SessionRestore,
            selected_uri,
        );
    }

    pub fn request_save_selected(self: &Rc<Self>, force_save_as: bool) {
        if let Some(tab) = self.selected_tab() {
            self.request_save_tab(&tab, force_save_as, Rc::new(|_result| {}));
        }
    }

    // PARSER-BOUNDARY: id=save_search_ui
    pub fn open_search(self: &Rc<Self>, replace_mode: bool) {
        let selected = self.selected_tab();
        let target = selected.as_ref().map_or(SearchTarget::Source, |tab| {
            tab.capture_search_target_for_open()
        });
        let prefill = selected
            .as_ref()
            .and_then(|tab| tab.single_line_search_selection_text(target));
        self.search.open(selected, target, replace_mode, prefill);
    }

    pub fn find_next(self: &Rc<Self>) {
        self.search.find_next();
    }

    pub fn find_previous(self: &Rc<Self>) {
        self.search.find_previous();
    }

    pub fn handle_window_close_request(self: &Rc<Self>) -> glib::Propagation {
        crate::workspace_close::handle_window_close_request(self)
    }

    #[must_use]
    pub fn allow_window_close(&self) -> bool {
        self.state.borrow().allow_window_close
    }

    pub fn apply_word_wrap_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.apply_word_wrap();
        }
    }

    pub fn apply_line_numbers_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.apply_line_numbers();
        }
    }

    pub fn apply_minimap_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.apply_minimap_visibility();
        }
    }

    pub fn apply_current_line_highlight_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.apply_current_line_highlight();
        }
    }

    pub fn apply_autosave_setting_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.sync_external_banner(true, self.shell.is_active());
        }
        self.refresh_selected_state();
    }

    pub fn apply_indentation_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.apply_indentation();
        }
    }

    pub(crate) fn apply_source_style_scheme_to_tabs(&self) {
        for tab in &self.state.borrow().tabs {
            tab.apply_source_style_scheme();
        }
        self.search.refresh_preview_search_style();
    }

    fn install_callbacks(self: &Rc<Self>, workspace_box: &gtk4::Box) {
        let weak = Rc::downgrade(self);
        self.tab_view.connect_selected_page_notify(move |_| {
            if let Some(workspace) = weak.upgrade() {
                workspace.handle_selected_tab_changed();
                workspace.persist_session_state_if_needed();
            }
        });

        let weak = Rc::downgrade(self);
        self.tab_view.connect_close_page(move |_, page| {
            weak.upgrade()
                .map_or(glib::Propagation::Proceed, |workspace| {
                    crate::workspace_close::on_close_page(&workspace, page)
                })
        });

        let weak = Rc::downgrade(self);
        self.tab_view
            .connect_page_detached(move |_, page, _position| {
                if let Some(workspace) = weak.upgrade() {
                    crate::workspace_close::on_page_detached(&workspace, page);
                }
            });

        let weak = Rc::downgrade(self);
        self.tab_view.connect_page_reordered(move |_, _, _| {
            if let Some(workspace) = weak.upgrade() {
                workspace.persist_session_state_if_needed();
            }
        });

        let weak = Rc::downgrade(self);
        self.shell.connect_is_active_notify(move |_| {
            if let Some(workspace) = weak.upgrade() {
                crate::workspace_monitor::on_selected_tab_changed(&workspace);
            }
        });

        let drop_target =
            gtk4::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        drop_target.connect_drop({
            let weak = Rc::downgrade(self);
            move |_, value, _, _| {
                let Some(workspace) = weak.upgrade() else {
                    return false;
                };
                match value.get::<gdk::FileList>() {
                    Ok(file_list) => {
                        workspace.request_open_files(file_list.files(), OpenSource::Drop);
                        true
                    }
                    Err(_) => false,
                }
            }
        });
        workspace_box.add_controller(drop_target);
    }

    pub(crate) fn request_save_tab(
        self: &Rc<Self>,
        tab: &Rc<EditorTab>,
        force_save_as: bool,
        callback: Rc<dyn Fn(SaveResult)>,
    ) {
        self.request_save_tab_kind(tab, force_save_as, SaveKind::Manual, callback);
    }

    pub(crate) fn request_save_tab_kind(
        self: &Rc<Self>,
        tab: &Rc<EditorTab>,
        force_save_as: bool,
        save_kind: SaveKind,
        callback: Rc<dyn Fn(SaveResult)>,
    ) {
        let weak = Rc::downgrade(self);
        let saved_tab = tab.clone();
        tab.request_save(
            &self.shell,
            force_save_as,
            save_kind,
            Rc::new(move |result| {
                if let Some(workspace) = weak.upgrade() {
                    if let SaveResult::Saved(outcome) = &result
                        && let Some(callback) = workspace.save_notification_handler.get()
                    {
                        callback(gio::File::for_uri(&outcome.new_uri));
                    }
                    if save_kind == SaveKind::Manual {
                        match &result {
                            SaveResult::Saved(outcome) => {
                                workspace.remember_recent_uri(&outcome.new_uri);
                                workspace.persist_session_state_if_needed();
                                workspace.refresh_selected_state();
                                if saved_tab.is_dirty() {
                                    autosave::reschedule_tab_autosave(&workspace, &saved_tab);
                                }
                                workspace.show_toast(&gettext("The Document Was Saved."));
                            }
                            SaveResult::CancelledByUser => {}
                            SaveResult::Failed(error) => {
                                crate::dialogs::present_error(&workspace.shell, error);
                            }
                        }
                    } else {
                        workspace.refresh_selected_state();
                    }
                    callback(result);
                }
            }),
        );
    }

    pub(crate) fn rebuild_primary_menu(&self) {
        let popover = crate::workspace_menu::build_primary_popover();
        self.menu_button.set_popover(Some(&popover));
    }

    pub(crate) fn set_format_preferences_handler(&self, callback: FormatPreferencesHandler) {
        let _set_callback = self.format_preferences_handler.set(callback);
    }

    pub(crate) fn set_compare_action_sync_handler(&self, callback: CompareActionSyncHandler) {
        let _set_callback = self.compare_action_sync_handler.set(callback);
    }

    pub(crate) fn set_document_tools_sync_handler(&self, callback: DocumentToolsSyncHandler) {
        let _set_callback = self.document_tools_sync_handler.set(callback);
    }

    pub(crate) fn set_git_action_sync_handler(&self, callback: GitActionSyncHandler) {
        let _set_callback = self.git_action_sync_handler.set(callback);
    }

    pub(crate) fn set_review_refresh_handler(&self, callback: ReviewRefreshHandler) {
        let _set_callback = self.review_refresh_handler.set(callback);
    }

    pub(crate) fn refresh_review_tab(&self, tab: &Rc<EditorTab>) {
        if let Some(callback) = self.review_refresh_handler.get() {
            callback(Rc::clone(tab));
        } else {
            tab.refresh_review_session();
        }
    }

    pub(crate) fn set_save_notification_handler(&self, callback: Rc<dyn Fn(gio::File)>) {
        let _set_callback = self.save_notification_handler.set(callback);
    }

    pub(crate) fn set_selected_line_ending_mode(&self, line_ending_mode: LineEndingMode) {
        if let Some(tab) = self.selected_tab() {
            if !tab.is_document() {
                return;
            }
            tab.set_current_line_ending_mode(line_ending_mode);
            self.refresh_selected_state();
        }
    }

    pub(crate) fn request_selected_encoding_action(self: &Rc<Self>) {
        let Some(tab) = self.selected_tab() else {
            return;
        };

        if !tab.is_document() {
            return;
        }

        if tab.document_uri().is_some() {
            let weak = Rc::downgrade(self);
            tab.request_reopen_with_encoding(
                &self.shell,
                Rc::new(move |result| {
                    if let Some(workspace) = weak.upgrade() {
                        match result {
                            Ok(()) => {
                                workspace.refresh_selected_state();
                                workspace.show_toast(&gettext("The File Was Reloaded."));
                            }
                            Err(crate::error::AppError::Cancelled) => {}
                            Err(error) => crate::dialogs::present_error(&workspace.shell, &error),
                        }
                    }
                }),
            );
            return;
        }

        let candidates = sourceview5::Encoding::default_candidates();
        let current = tab.current_format().encoding().to_source_encoding();
        let weak = Rc::downgrade(self);
        crate::dialogs::choose_encoding(
            &self.shell,
            &gettext("Choose a Text Encoding"),
            &gettext("Choose the encoding to use for the next save."),
            &candidates,
            current.as_ref(),
            &pgettext("dialog button", "Apply"),
            move |selection| {
                if let Some(workspace) = weak.upgrade()
                    && let Some(tab) = workspace.selected_tab()
                    && let Some(encoding) = selection
                {
                    tab.set_current_encoding(EncodingInfo::from_encoding(&encoding));
                    workspace.refresh_selected_state();
                }
            },
        );
    }
}
