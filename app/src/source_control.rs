use std::cell::RefCell;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::git_process::{GitCallback, GitIdentity, GitProcess, GitProcessError, GitRepoContext};
use crate::git_status::{GitAttrState, GitCapabilities, GitStatusSnapshot};
use crate::settings::AppSettings;
#[cfg(test)]
use crate::settings::SourceControlViewMode;
use crate::workspace::Workspace;

pub(crate) mod action_widgets;
pub(crate) mod actions;
mod history;
mod list_view;
mod live;
mod path_target;
mod refresh;
mod row_popover;
mod status_style;
#[cfg(test)]
mod tests;
pub(crate) mod tree_model;
mod tree_view;
mod view_mode;

use history::SourceControlHistory;
use live::SourceControlLiveRefresh;
use refresh::{
    RefreshOrigin, ellipsis_label, emit_project_statuses, finish_error, rebuild_views,
    refresh_status, refresh_status_with_origin,
};
use view_mode::SourceControlViews;

pub(super) type SourceStateRef = Rc<RefCell<SourceControlState>>;
#[cfg(test)]
pub(crate) type DetectRepoForTests =
    Rc<dyn Fn(&Path, &gio::Cancellable, GitCallback<GitRepoContext>)>;
type GitStatusHandler = Rc<dyn Fn(Vec<(String, String)>)>;
const HISTORY_SPLIT_DEFAULT_POSITION: i32 = 360;

#[derive(Clone)]
pub(crate) struct SourceControlController {
    state: SourceStateRef,
}

pub(super) struct SourceControlState {
    pub(super) root: adw::ToolbarView,
    pub(super) title: adw::WindowTitle,
    pub(super) status_label: gtk4::Label,
    views: SourceControlViews,
    history: SourceControlHistory,
    #[cfg(test)]
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
    pub(super) cancellable: Option<gio::Cancellable>,
    pub(super) live_refresh: Option<SourceControlLiveRefresh>,
    pub(super) status_stale: bool,
    pub(super) action_generation: u64,
    pub(super) state_change_handler: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    detect_repo: DetectRepoForTests,
    status_handler: Option<GitStatusHandler>,
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

        let views = SourceControlViews::new(settings);
        changes_pane.append(&views.widget());

        let (commit_revealer, commit_entry, commit_button) = build_commit_controls();
        changes_pane.append(&commit_revealer);

        let history = SourceControlHistory::new();
        let history_split = build_history_split(&changes_pane, &history);
        #[cfg(test)]
        let history_split_for_tests = history_split.clone();
        content.append(&history_split);
        root.set_content(Some(&content));

        let state = Rc::new(RefCell::new(SourceControlState {
            root,
            title,
            status_label,
            views,
            history,
            #[cfg(test)]
            history_split: history_split_for_tests,
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
            cancellable: None,
            live_refresh: None,
            status_stale: true,
            action_generation: 0,
            state_change_handler: None,
            #[cfg(test)]
            detect_repo: Rc::new(|path, cancellable, callback| {
                GitProcess::detect_repo(path, cancellable, callback);
            }),
            status_handler: None,
        }));
        state
            .borrow()
            .views
            .connect_activation(Rc::downgrade(&state));

        let weak = Rc::downgrade(&state);
        refresh.connect_activate(move |_, _| {
            if let Some(state) = weak.upgrade() {
                refresh_status_with_origin(&state, RefreshOrigin::Manual);
            }
        });

        let weak = Rc::downgrade(&state);
        state.borrow().commit_button.connect_clicked(move |_| {
            if let Some(state) = weak.upgrade() {
                commit(&state);
            }
        });

        Self { state }
    }

    #[must_use]
    pub(crate) fn widget(&self) -> adw::ToolbarView {
        self.state.borrow().root.clone()
    }

    pub(crate) fn set_project_root(&self, folder: Option<gio::File>) {
        set_project_root(&self.state, folder);
    }

    pub(crate) fn root_change_handler(&self) -> Rc<dyn Fn(Option<gio::File>)> {
        let weak = Rc::downgrade(&self.state);
        Rc::new(move |root| {
            if let Some(state) = weak.upgrade() {
                set_project_root(&state, root);
            }
        })
    }

    pub(crate) fn set_status_handler(&self, handler: GitStatusHandler) {
        self.state.borrow_mut().status_handler = Some(handler);
    }

    pub(crate) fn set_state_change_handler(&self, handler: Rc<dyn Fn()>) {
        self.state.borrow_mut().state_change_handler = Some(handler);
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
            && state.history_split.position() == HISTORY_SPLIT_DEFAULT_POSITION
    }

    #[cfg(test)]
    pub(crate) fn set_detect_repo_for_tests(&self, detect_repo: DetectRepoForTests) {
        self.state.borrow_mut().detect_repo = detect_repo;
    }
}

fn build_commit_controls() -> (gtk4::Revealer, gtk4::Entry, gtk4::Button) {
    let commit_box = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
    let commit_entry = gtk4::Entry::builder()
        .placeholder_text(pgettext("git commit", "Commit Message"))
        .build();
    commit_box.append(&commit_entry);

    let commit_button = gtk4::Button::with_label(&pgettext("git commit", "Commit"));
    commit_button.add_css_class("suggested-action");
    commit_button.set_sensitive(false);
    commit_box.append(&commit_button);

    let commit_revealer = gtk4::Revealer::builder()
        .transition_type(gtk4::RevealerTransitionType::SlideDown)
        .child(&commit_box)
        .build();
    (commit_revealer, commit_entry, commit_button)
}

fn build_history_split(changes_pane: &gtk4::Box, history: &SourceControlHistory) -> gtk4::Paned {
    let history_split = gtk4::Paned::new(gtk4::Orientation::Vertical);
    history_split.set_vexpand(true);
    history_split.set_wide_handle(true);
    history_split.set_resize_start_child(true);
    history_split.set_resize_end_child(true);
    history_split.set_shrink_start_child(false);
    history_split.set_shrink_end_child(false);
    history_split.set_start_child(Some(changes_pane));
    history_split.set_end_child(Some(&history.widget()));
    history_split.set_position(HISTORY_SPLIT_DEFAULT_POSITION);
    history_split
}

fn set_project_root(state: &SourceStateRef, folder: Option<gio::File>) {
    cancel_refresh(state);
    live::cancel(state);
    let Some(folder) = folder else {
        reset_project_state(state, &gettext("Open a folder to see Git status."), false);
        return;
    };
    let Some(path) = folder.path() else {
        reset_project_state(
            state,
            &gettext("Only local Git folders are supported."),
            false,
        );
        return;
    };
    let cancellable = gio::Cancellable::new();
    {
        let mut state = state.borrow_mut();
        state.cancellable = Some(cancellable.clone());
        state.repo = None;
        state.process = None;
        state.attrs = GitAttrState::default();
        state.snapshot = GitStatusSnapshot::default();
        state.status_stale = true;
        state.action_generation = state.action_generation.wrapping_add(1);
        state
            .status_label
            .set_label(&ellipsis_label(gettext("Refreshing Git status")));
        set_commit_controls_enabled(&state, false);
        emit_project_statuses(&state);
        state.history.clear();
        rebuild_views(&state);
    }
    actions::fire_state_change_handler(state);
    let weak = Rc::downgrade(state);
    let cancellable_for_callback = cancellable.clone();
    let callback: GitCallback<GitRepoContext> = Rc::new(move |result| {
        if cancellable_for_callback.is_cancelled() {
            return;
        }
        let Some(state) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(repo_context) => {
                {
                    let mut state = state.borrow_mut();
                    state.repo = Some(repo_context.work_tree.clone());
                    state.process = Some(GitProcess::new(repo_context));
                    state.attrs = GitAttrState::default();
                    state.snapshot = GitStatusSnapshot::default();
                    state.status_stale = true;
                }
                live::install(&state);
                refresh_status_with_origin(&state, RefreshOrigin::Initial);
            }
            Err(error) if git_error_is_cancelled(&error) => {}
            Err(_error) => {
                reset_project_state(
                    &state,
                    &gettext("This folder is not a Git repository."),
                    false,
                );
            }
        }
    });
    #[cfg(test)]
    {
        let detect_repo = state.borrow().detect_repo.clone();
        detect_repo(&path, &cancellable, callback);
    }
    #[cfg(not(test))]
    GitProcess::detect_repo(&path, &cancellable, callback);
}

fn reset_project_state(state: &SourceStateRef, label: &str, mark_stale: bool) {
    {
        let mut state = state.borrow_mut();
        state.repo = None;
        state.process = None;
        state.attrs = GitAttrState::default();
        state.snapshot = GitStatusSnapshot::default();
        state.status_stale = mark_stale;
        state.action_generation = state.action_generation.wrapping_add(1);
        state.status_label.set_label(label);
        set_commit_controls_enabled(&state, false);
        emit_project_statuses(&state);
        state.history.clear();
        rebuild_views(&state);
    }
    actions::fire_state_change_handler(state);
}

pub(super) fn set_commit_controls_enabled(state: &SourceControlState, enabled: bool) {
    state.commit_button.set_sensitive(enabled);
    state.commit_revealer.set_reveal_child(enabled);
}

pub(super) fn git_error_is_cancelled(error: &GitProcessError) -> bool {
    matches!(error, GitProcessError::Cancelled)
}

fn commit(state: &SourceStateRef) {
    let (process, message, settings_identity, attrs_unavailable) = {
        let state = state.borrow();
        let Some(process) = state.process.clone() else {
            return;
        };
        let message = state.commit_entry.text().to_string();
        let identity = state.settings.git_identity();
        (process, message, identity, state.attrs.is_unavailable())
    };
    if attrs_unavailable {
        state
            .borrow()
            .status_label
            .set_label(&git_attrs_unavailable_text());
        return;
    }
    if message.trim().is_empty() {
        state
            .borrow()
            .status_label
            .set_label(&gettext("Enter a commit message first."));
        return;
    }
    let cancellable = gio::Cancellable::new();
    state.borrow_mut().cancellable = Some(cancellable.clone());
    let weak = Rc::downgrade(state);
    let process_for_commit = process.clone();
    let cancellable_for_commit = cancellable.clone();
    process.read_git_identity(
        &cancellable,
        Rc::new(move |identity| {
            if cancellable_for_commit.is_cancelled() {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            let identity = identity.ok().flatten().or_else(|| {
                GitIdentity::new(settings_identity.0.clone(), settings_identity.1.clone()).ok()
            });
            let Some(identity) = identity else {
                finish_error(
                    &state,
                    &gettext(
                        "Set a Git identity in the Source Control preferences before committing.",
                    ),
                );
                return;
            };
            let weak = Rc::downgrade(&state);
            let cancellable_for_callback = cancellable_for_commit.clone();
            process_for_commit.commit(
                &identity,
                &message,
                &cancellable_for_commit,
                Rc::new(move |result| {
                    if cancellable_for_callback.is_cancelled() {
                        return;
                    }
                    let Some(state) = weak.upgrade() else {
                        return;
                    };
                    match result {
                        Ok(()) => {
                            state.borrow().commit_entry.set_text("");
                            refresh_status(&state);
                        }
                        Err(error) if git_error_is_cancelled(&error) => {}
                        Err(error) => finish_error(&state, &git_error_text(&error)),
                    }
                }),
            );
        }),
    );
}

pub(super) fn git_error_text(error: &GitProcessError) -> String {
    match error {
        GitProcessError::InvalidIdentity => gettext("The Git identity is not valid."),
        GitProcessError::OutputTooLarge => gettext("Git output was too large to process safely."),
        GitProcessError::BinaryContent => gettext("Binary files cannot be compared."),
        _ => gettext("The Git operation failed."),
    }
}

pub(super) fn git_attrs_unavailable_text() -> String {
    gettext("Unable to read Git attributes. Git actions are disabled.")
}

fn cancel_refresh(state: &SourceStateRef) {
    if let Some(cancellable) = state.borrow_mut().cancellable.take() {
        cancellable.cancel();
    }
}

fn saved_file_in_repo(state: &SourceControlState, file: &gio::File) -> bool {
    live::saved_file_in_repo(state, file)
}
