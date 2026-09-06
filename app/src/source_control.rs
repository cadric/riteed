use std::cell::RefCell;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gettextrs::gettext;
use gtk4::accessible::Property;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;

use crate::editor_tab::EditorTab;
#[cfg(test)]
use crate::git_process::{GitCallback, GitRepoContext};
use crate::git_process::{GitProcess, GitProcessError};
use crate::git_status::{GitAttrState, GitCapabilities, GitStatusSnapshot};
use crate::settings::AppSettings;
#[cfg(test)]
use crate::settings::SourceControlViewMode;
use crate::workspace::Workspace;

pub(crate) mod action_widgets;
pub(crate) mod actions;
mod active_row;
mod commit;
mod history;
mod list_view;
mod live;
mod live_scheduler;
mod minimap;
mod operation_bridge;
mod path_target;
mod refresh;
mod review;
mod review_loader;
mod root;
mod row_popover;
mod row_widgets;
mod slots;
mod status_style;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
pub(crate) mod tree_model;
mod tree_view;
mod ui;
mod view_mode;
mod weak;

use history::SourceControlHistory;
use live::SourceControlLiveRefresh;
use refresh::{RefreshOrigin, refresh_status_with_origin};
use slots::{OperationSlots, SnapshotId};
use view_mode::SourceControlViews;

use operation_bridge::{
    cancel_minimap_requests, cancel_minimap_requests_for_tab, cancel_review_requests,
    remove_minimap_cancellable, remove_review_cancellable, track_minimap_cancellable,
    track_review_cancellable,
};

pub(super) type SourceStateRef = Rc<RefCell<SourceControlState>>;
#[cfg(test)]
pub(crate) type DetectRepoForTests =
    Rc<dyn Fn(&Path, &gio::Cancellable, GitCallback<GitRepoContext>)>;
type GitStatusHandler = Rc<dyn Fn(Vec<(String, String)>)>;

pub(super) struct MinimapRequest {
    tab: Weak<EditorTab>,
    source: String,
    cancellable: gio::Cancellable,
}

#[derive(Clone)]
pub(crate) struct SourceControlController {
    state: SourceStateRef,
}

pub(super) struct SourceControlState {
    pub(super) root: adw::ToolbarView,
    pub(super) title: adw::WindowTitle,
    pub(super) status_label: gtk4::Label,
    views: Rc<SourceControlViews>,
    pub(super) active_uri: Option<String>,
    history: Rc<SourceControlHistory>,
    history_split: gtk4::Paned,
    pub(super) commit_revealer: gtk4::Revealer,
    pub(super) commit_entry: gtk4::Entry,
    pub(super) commit_button: gtk4::Button,
    pub(super) settings: AppSettings,
    pub(super) workspace: Weak<Workspace>,
    pub(super) repo: Option<PathBuf>,
    pub(super) process: Option<GitProcess>,
    pub(super) capabilities: GitCapabilities,
    pub(super) attrs: GitAttrState,
    pub(super) snapshot: GitStatusSnapshot,
    pub(super) snapshot_id: Option<SnapshotId>,
    pub(super) operations: OperationSlots,
    pub(super) review_cancellables: Vec<gio::Cancellable>,
    pub(super) minimap_cancellables: Vec<MinimapRequest>,
    pub(super) live_refresh: Option<Rc<RefCell<SourceControlLiveRefresh>>>,
    pub(super) recovery_source: Option<glib::SourceId>,
    pub(super) status_stale: bool,
    pub(super) review_generation: u64,
    pub(super) review_staged_action: gio::SimpleAction,
    pub(super) review_unstaged_action: gio::SimpleAction,
    pub(super) state_change_handler: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    detect_repo: DetectRepoForTests,
    status_handler: Option<GitStatusHandler>,
}

impl Drop for SourceControlState {
    fn drop(&mut self) {
        if let Some(source) = self.recovery_source.take() {
            source.remove();
        }
        self.operations.cancel_refresh();
        let mut cancellations = self.operations.drain_for_teardown();
        cancellations.extend(operation_bridge::take_review_cancellations(self));
        cancellations.extend(operation_bridge::take_minimap_cancellations(self));
        slots::cancel_queued(cancellations);
    }
}

impl SourceControlController {
    #[must_use]
    pub(crate) fn new(
        window: &adw::ApplicationWindow,
        settings: &AppSettings,
        workspace: &Rc<Workspace>,
    ) -> Self {
        let refresh = gio::SimpleAction::new("git-refresh", None);
        window.add_action(&refresh);
        let review_staged_action = gio::SimpleAction::new("git-review-staged", None);
        let review_unstaged_action = gio::SimpleAction::new("git-review-unstaged", None);
        review_staged_action.set_enabled(false);
        review_unstaged_action.set_enabled(false);

        let root = adw::ToolbarView::new();
        let header = adw::HeaderBar::new();
        header.set_show_start_title_buttons(false);
        header.set_show_end_title_buttons(false);
        let title = adw::WindowTitle::new(&gettext("Source Control"), "");
        header.set_title_widget(Some(&title));

        let refresh_tooltip = gettext("Refresh Source Control");
        let refresh_button = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text(&refresh_tooltip)
            .build();
        refresh_button.update_property(&[Property::Label(&refresh_tooltip)]);
        refresh_button.set_action_name(Some("win.git-refresh"));
        header.pack_end(&refresh_button);
        let review_tooltip = gettext("Review Source Control Changes");
        let review_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&review::review_menu_model())
            .tooltip_text(&review_tooltip)
            .build();
        review_button.update_property(&[Property::Label(&review_tooltip)]);
        header.pack_end(&review_button);
        root.add_top_bar(&header);

        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.set_vexpand(true);

        let changes_pane = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        changes_pane.set_vexpand(true);

        let status_label = gtk4::Label::new(Some(&gettext("Open a folder to see Git status.")));
        status_label.set_xalign(0.0);
        status_label.set_wrap(true);
        changes_pane.append(&status_label);

        let views = Rc::new(SourceControlViews::new(settings));
        changes_pane.append(&views.widget());

        let (commit_revealer, commit_entry, commit_button) = ui::build_commit_controls();
        changes_pane.append(&commit_revealer);

        let history = Rc::new(SourceControlHistory::new());
        let history_split = ui::build_history_split(&changes_pane, &history);
        content.append(&history_split);
        root.set_content(Some(&content));

        let state = Rc::new(RefCell::new(SourceControlState {
            root,
            title,
            status_label,
            views: Rc::clone(&views),
            active_uri: None,
            history: Rc::clone(&history),
            history_split,
            commit_revealer,
            commit_entry,
            commit_button,
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
            repo: None,
            process: None,
            capabilities: GitCapabilities::default(),
            attrs: GitAttrState::default(),
            snapshot: GitStatusSnapshot::default(),
            snapshot_id: None,
            operations: OperationSlots::new(),
            review_cancellables: Vec::new(),
            minimap_cancellables: Vec::new(),
            live_refresh: None,
            recovery_source: None,
            status_stale: true,
            review_generation: 0,
            review_staged_action,
            review_unstaged_action,
            state_change_handler: None,
            #[cfg(test)]
            detect_repo: Rc::new(|path, cancellable, callback| {
                GitProcess::detect_repo(path, cancellable, callback);
            }),
            status_handler: None,
        }));
        views.connect_activation(Rc::downgrade(&state));
        history::connect_toggle(&state);
        review::install_actions(&state, window);
        install_callbacks(&state, &refresh);

        Self { state }
    }

    #[must_use]
    pub(crate) fn widget(&self) -> adw::ToolbarView {
        self.state.borrow().root.clone()
    }

    pub(crate) fn set_project_root(&self, folder: Option<gio::File>) {
        root::set_project_root(&self.state, folder);
    }

    pub(crate) fn root_change_handler(&self) -> Rc<dyn Fn(Option<gio::File>)> {
        let weak = Rc::downgrade(&self.state);
        Rc::new(move |root| {
            if let Some(state) = weak.upgrade() {
                root::set_project_root(&state, root);
            }
        })
    }

    pub(crate) fn set_status_handler(&self, handler: GitStatusHandler) {
        self.state.borrow_mut().status_handler = Some(handler);
    }

    pub(crate) fn set_state_change_handler(&self, handler: Rc<dyn Fn()>) {
        self.state.borrow_mut().state_change_handler = Some(handler);
    }

    pub(crate) fn refresh_editor_minimap_diffs(&self) {
        minimap::refresh_open_tabs(&self.state);
    }

    pub(crate) fn refresh_editor_minimap_diff_for_tab(&self, tab: Option<Rc<EditorTab>>) {
        minimap::refresh_tab(&self.state, tab);
    }

    pub(crate) fn review_refresh_handler(&self) -> Rc<dyn Fn(Rc<EditorTab>)> {
        let weak = Rc::downgrade(&self.state);
        Rc::new(move |tab| {
            if let Some(state) = weak.upgrade() {
                review::refresh_open_review(&state, &tab);
            }
        })
    }

    #[must_use]
    pub(crate) fn entry_for_uri(&self, uri: &str) -> Option<crate::git_status::GitStatusEntry> {
        let state = self.state.borrow();
        let repo = state.repo.as_ref()?;
        let raw = path_target::raw_path_for_uri(repo, uri)?;
        state
            .snapshot
            .entries
            .iter()
            .find(|entry| entry.path.raw() == raw.as_slice())
            .cloned()
    }

    #[must_use]
    pub(crate) fn entry_action_state_for_uri(
        &self,
        uri: &str,
        action: actions::GitRowAction,
    ) -> Option<crate::git_status::GitActionState> {
        let entry = self.entry_for_uri(uri)?;
        Some(match action {
            actions::GitRowAction::Diff => entry.diff_action,
            actions::GitRowAction::Stage => entry.stage_action,
            actions::GitRowAction::Unstage => entry.unstage_action,
            actions::GitRowAction::Discard => entry.discard_action,
        })
    }

    pub(crate) fn run_action_for_uri(&self, uri: &str, action: actions::GitRowAction) {
        let raw = {
            let state = self.state.borrow();
            let Some(repo) = state.repo.as_ref() else {
                return;
            };
            let Some(raw) = path_target::raw_path_for_uri(repo, uri) else {
                return;
            };
            raw
        };
        actions::run_path_action(&self.state, &raw, action);
    }

    pub(crate) fn save_notification_handler(&self) -> Rc<dyn Fn(gio::File)> {
        let weak = Rc::downgrade(&self.state);
        Rc::new(move |file| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if saved_file_in_repo(&state.borrow(), &file) {
                live::schedule(&state);
            }
        })
    }

    #[cfg(test)]
    pub(crate) fn status_label_for_tests(&self) -> String {
        self.state.borrow().status_label.label().to_string()
    }

    #[cfg(test)]
    pub(crate) fn row_count_for_tests(&self) -> usize {
        self.state.borrow().views.row_count_for_tests()
    }

    #[cfg(test)]
    pub(crate) fn activate_path_for_tests(&self, path: &str) -> bool {
        let activation = self.state.borrow().views.activate_path_for_tests(path);
        let Some((list_view, position)) = activation else {
            return false;
        };
        list_view.emit_by_name::<()>("activate", &[&position]);
        true
    }

    #[cfg(test)]
    pub(crate) fn row_state_for_tests(&self, path: &str) -> Option<(String, bool, bool)> {
        self.state
            .borrow()
            .snapshot
            .entries
            .iter()
            .find(|entry| entry.path.as_utf8() == Some(path))
            .map(|entry| {
                (
                    String::from(entry.status.badge()),
                    entry.stage_action.enabled(),
                    entry.unstage_action.enabled(),
                )
            })
    }

    #[cfg(test)]
    pub(crate) fn set_view_mode_for_tests(&self, mode: SourceControlViewMode) {
        SourceControlViews::set_mode_for_tests(&self.state, mode);
    }

    #[cfg(test)]
    pub(crate) fn recent_commit_count_for_tests(&self) -> usize {
        self.state.borrow().history.row_count_for_tests()
    }

    #[cfg(test)]
    pub(crate) fn commit_controls_visible_for_tests(&self) -> bool {
        self.state.borrow().commit_revealer.reveals_child()
    }

    #[cfg(test)]
    pub(crate) fn history_split_resizable_for_tests(&self) -> bool {
        let state = self.state.borrow();
        state.history_split.orientation() == gtk4::Orientation::Vertical
            && state.history_split.is_wide_handle()
            && state.history_split.resizes_start_child()
            && state.history_split.resizes_end_child()
            && state.history_split.start_child().is_some()
            && state.history_split.end_child().is_some()
            && state.history_split.position() == ui::HISTORY_SPLIT_DEFAULT_POSITION
    }

    #[cfg(test)]
    pub(crate) fn set_detect_repo_for_tests(&self, detect_repo: DetectRepoForTests) {
        self.state.borrow_mut().detect_repo = detect_repo;
    }
}

fn install_callbacks(state: &SourceStateRef, refresh: &gio::SimpleAction) {
    let weak = Rc::downgrade(state);
    refresh.connect_activate(move |_, _| {
        if let Some(state) = weak.upgrade() {
            refresh_status_with_origin(&state, RefreshOrigin::Manual);
        }
    });

    let weak = Rc::downgrade(state);
    state.borrow().commit_button.connect_clicked(move |_| {
        if let Some(state) = weak.upgrade() {
            commit::run(&state);
        }
    });
}

pub(super) fn set_commit_controls_enabled(state: &SourceStateRef, enabled: bool) {
    let (button, revealer) = {
        let state = state.borrow();
        (state.commit_button.clone(), state.commit_revealer.clone())
    };
    button.set_sensitive(enabled);
    revealer.set_reveal_child(enabled);
}

pub(super) fn set_status_label(state: &SourceStateRef, text: &str) {
    let label = state.borrow().status_label.clone();
    label.set_label(text);
}

pub(super) fn set_title_subtitle(state: &SourceStateRef, text: &str) {
    let title = state.borrow().title.clone();
    title.set_subtitle(text);
}

pub(super) fn git_error_is_cancelled(error: &GitProcessError) -> bool {
    matches!(error, GitProcessError::Cancelled)
}

pub(super) fn git_error_text(error: &GitProcessError) -> String {
    match error {
        GitProcessError::InvalidIdentity => gettext("The Git identity is not valid."),
        GitProcessError::TimedOut => gettext("The Git operation timed out."),
        GitProcessError::OutputTooLarge => gettext("Git output was too large to process safely."),
        GitProcessError::BinaryContent => gettext("Binary files cannot be compared."),
        _ => gettext("The Git operation failed."),
    }
}

pub(super) fn git_attrs_unavailable_text() -> String {
    gettext("Unable to read Git attributes. Git actions are disabled.")
}

fn saved_file_in_repo(state: &SourceControlState, file: &gio::File) -> bool {
    root::saved_file_in_repo(state, file)
}
