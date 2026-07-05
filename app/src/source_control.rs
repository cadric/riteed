use std::cell::RefCell;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gettextrs::gettext;
use gtk4::accessible::Property;
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::editor_tab::EditorTab;
#[cfg(test)]
use crate::git_process::{GitCallback, GitRepoContext};
use crate::git_process::{GitIdentity, GitProcess, GitProcessError};
use crate::git_status::{GitAttrState, GitCapabilities, GitStatusSnapshot};
use crate::settings::AppSettings;
#[cfg(test)]
use crate::settings::SourceControlViewMode;
use crate::workspace::Workspace;

pub(crate) mod action_widgets;
pub(crate) mod actions;
mod active_row;
mod history;
mod list_view;
mod live;
mod live_scheduler;
mod minimap;
mod path_target;
mod refresh;
mod review;
mod review_loader;
mod root;
mod row_popover;
mod row_widgets;
mod status_style;
#[cfg(test)]
mod testing;
#[cfg(test)]
mod tests;
pub(crate) mod tree_model;
mod tree_view;
mod ui;
mod view_mode;

use history::SourceControlHistory;
use live::SourceControlLiveRefresh;
use refresh::{RefreshOrigin, finish_error, refresh_status, refresh_status_with_origin};
use view_mode::SourceControlViews;

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
    views: SourceControlViews,
    pub(super) active_uri: Option<String>,
    history: SourceControlHistory,
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
    pub(super) review_cancellables: Vec<gio::Cancellable>,
    pub(super) minimap_cancellables: Vec<MinimapRequest>,
    pub(super) live_refresh: Option<SourceControlLiveRefresh>,
    pub(super) status_stale: bool,
    pub(super) action_generation: u64,
    pub(super) review_generation: u64,
    pub(super) review_staged_action: gio::SimpleAction,
    pub(super) review_unstaged_action: gio::SimpleAction,
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

        let views = SourceControlViews::new(settings);
        changes_pane.append(&views.widget());

        let (commit_revealer, commit_entry, commit_button) = ui::build_commit_controls();
        changes_pane.append(&commit_revealer);

        let history = SourceControlHistory::new();
        let history_split = ui::build_history_split(&changes_pane, &history);
        content.append(&history_split);
        root.set_content(Some(&content));

        let state = Rc::new(RefCell::new(SourceControlState {
            root,
            title,
            status_label,
            views,
            active_uri: None,
            history,
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
            cancellable: None,
            review_cancellables: Vec::new(),
            minimap_cancellables: Vec::new(),
            live_refresh: None,
            status_stale: true,
            action_generation: 0,
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
        state
            .borrow()
            .views
            .connect_activation(Rc::downgrade(&state));
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
            commit(&state);
        }
    });
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

fn cancel_refresh(state: &SourceStateRef) {
    if let Some(cancellable) = state.borrow_mut().cancellable.take() {
        cancellable.cancel();
    }
}

pub(super) fn track_review_cancellable(state: &SourceStateRef, cancellable: &gio::Cancellable) {
    state
        .borrow_mut()
        .review_cancellables
        .push(cancellable.clone());
}

pub(super) fn remove_review_cancellable(state: &SourceStateRef, cancellable: &gio::Cancellable) {
    state
        .borrow_mut()
        .review_cancellables
        .retain(|active| active != cancellable);
}

pub(super) fn track_minimap_cancellable(
    state: &SourceStateRef,
    tab: &Rc<EditorTab>,
    source: &str,
    cancellable: &gio::Cancellable,
) {
    state
        .borrow_mut()
        .minimap_cancellables
        .push(MinimapRequest {
            tab: Rc::downgrade(tab),
            source: source.to_string(),
            cancellable: cancellable.clone(),
        });
}

pub(super) fn remove_minimap_cancellable(state: &SourceStateRef, cancellable: &gio::Cancellable) {
    state
        .borrow_mut()
        .minimap_cancellables
        .retain(|active| &active.cancellable != cancellable);
}

pub(super) fn cancel_minimap_requests_for_tab(
    state: &SourceStateRef,
    tab: &Rc<EditorTab>,
    source: Option<&str>,
) {
    let mut state = state.borrow_mut();
    let mut retained = Vec::new();
    for request in state.minimap_cancellables.drain(..) {
        let same_tab = request
            .tab
            .upgrade()
            .is_some_and(|active| Rc::ptr_eq(&active, tab));
        let same_source = source.is_none_or(|source| request.source == source);
        if same_tab && same_source {
            request.cancellable.cancel();
        } else {
            retained.push(request);
        }
    }
    state.minimap_cancellables = retained;
}

pub(super) fn cancel_minimap_requests(state: &SourceStateRef) {
    cancel_minimap_requests_locked(&mut state.borrow_mut());
}

pub(super) fn cancel_review_requests_locked(state: &mut SourceControlState) {
    for cancellable in state.review_cancellables.drain(..) {
        cancellable.cancel();
    }
}

pub(super) fn cancel_minimap_requests_locked(state: &mut SourceControlState) {
    for request in state.minimap_cancellables.drain(..) {
        request.cancellable.cancel();
    }
}
