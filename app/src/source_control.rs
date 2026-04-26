use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use gettextrs::{gettext, pgettext};
use gtk4::accessible::Property;
use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::git_process::{GitIdentity, GitProcess, GitProcessError};
use crate::git_status::{GitAttrs, GitCapabilities, GitPath, GitStatusSnapshot};
use crate::settings::AppSettings;
use crate::workspace::Workspace;

mod actions;

pub(super) type SourceStateRef = Rc<RefCell<SourceControlState>>;
type GitStatusHandler = Rc<dyn Fn(Vec<(String, String)>)>;

#[derive(Clone)]
pub(crate) struct SourceControlController {
    state: SourceStateRef,
}

pub(super) struct SourceControlState {
    pub(super) root: adw::ToolbarView,
    pub(super) title: adw::WindowTitle,
    pub(super) status_label: gtk4::Label,
    pub(super) list: gtk4::ListBox,
    pub(super) commit_entry: gtk4::Entry,
    pub(super) commit_button: gtk4::Button,
    pub(super) settings: AppSettings,
    pub(super) workspace: Weak<Workspace>,
    pub(super) repo: Option<PathBuf>,
    pub(super) process: Option<GitProcess>,
    pub(super) capabilities: GitCapabilities,
    pub(super) attrs: GitAttrs,
    pub(super) snapshot: GitStatusSnapshot,
    pub(super) cancellable: Option<gio::Cancellable>,
    pub(super) status_stale: bool,
    pub(super) action_generation: u64,
    pub(super) self_weak: Weak<RefCell<SourceControlState>>,
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

        let status_label = gtk4::Label::new(Some(&gettext("Open a folder to see Git status.")));
        status_label.set_xalign(0.0);
        status_label.set_wrap(true);
        content.append(&status_label);

        let list = gtk4::ListBox::new();
        list.add_css_class("boxed-list");
        list.set_activate_on_single_click(true);
        content.append(&list);

        let commit_entry = gtk4::Entry::builder()
            .placeholder_text(pgettext("git commit", "Commit Message"))
            .build();
        content.append(&commit_entry);

        let commit_button = gtk4::Button::with_label(&pgettext("git commit", "Commit"));
        commit_button.add_css_class("suggested-action");
        commit_button.set_sensitive(false);
        content.append(&commit_button);

        let hooks = gtk4::Label::new(Some(&gettext("Pre-commit hooks are not run from Riteed.")));
        hooks.add_css_class("caption");
        hooks.set_xalign(0.0);
        hooks.set_wrap(true);
        content.append(&hooks);

        let scroller = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&content)
            .build();
        root.set_content(Some(&scroller));

        let state = Rc::new(RefCell::new(SourceControlState {
            root,
            title,
            status_label,
            list,
            commit_entry,
            commit_button,
            settings: settings.clone(),
            workspace: Rc::downgrade(workspace),
            repo: None,
            process: None,
            capabilities: GitCapabilities::default(),
            attrs: GitAttrs::default(),
            snapshot: GitStatusSnapshot::default(),
            cancellable: None,
            status_stale: true,
            action_generation: 0,
            self_weak: Weak::new(),
            status_handler: None,
        }));
        state.borrow_mut().self_weak = Rc::downgrade(&state);

        let weak = Rc::downgrade(&state);
        refresh.connect_activate(move |_, _| {
            if let Some(state) = weak.upgrade() {
                refresh_status(&state);
            }
        });

        let weak = Rc::downgrade(&state);
        state.borrow().commit_button.connect_clicked(move |_| {
            if let Some(state) = weak.upgrade() {
                commit(&state);
            }
        });

        let weak = Rc::downgrade(&state);
        state.borrow().list.connect_row_activated(move |_, row| {
            if let Some(state) = weak.upgrade() {
                actions::activate_row(&state, row.index());
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

    #[cfg(test)]
    pub(crate) fn status_label_for_tests(&self) -> String {
        self.state.borrow().status_label.label().to_string()
    }

    #[cfg(test)]
    pub(crate) fn row_count_for_tests(&self) -> usize {
        let list = self.state.borrow().list.clone();
        let mut count = 0;
        let mut child = list.first_child();
        while let Some(row) = child {
            count += 1;
            child = row.next_sibling();
        }
        count
    }

    #[cfg(test)]
    pub(crate) fn activate_path_for_tests(&self, path: &str) -> bool {
        let (list, index) = {
            let state = self.state.borrow();
            let index = state
                .snapshot
                .entries
                .iter()
                .position(|entry| entry.path.as_utf8() == Some(path));
            (state.list.clone(), index)
        };
        let Some(row) = index
            .and_then(|index| i32::try_from(index).ok())
            .and_then(|index| list.row_at_index(index))
        else {
            return false;
        };
        list.emit_by_name::<()>("row-activated", &[&row]);
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
}

fn set_project_root(state: &SourceStateRef, folder: Option<gio::File>) {
    cancel_refresh(state);
    let Some(folder) = folder else {
        let mut state = state.borrow_mut();
        state.repo = None;
        state.process = None;
        state.snapshot = GitStatusSnapshot::default();
        state
            .status_label
            .set_label(&gettext("Open a folder to see Git status."));
        emit_project_statuses(&state);
        actions::rebuild_rows(&mut state);
        return;
    };
    let Some(path) = folder.path() else {
        let mut state = state.borrow_mut();
        state.repo = None;
        state.process = None;
        state
            .status_label
            .set_label(&gettext("Only local Git folders are supported."));
        emit_project_statuses(&state);
        actions::rebuild_rows(&mut state);
        return;
    };
    if !has_git_metadata_candidate(&path) {
        let mut state = state.borrow_mut();
        state.repo = None;
        state.process = None;
        state
            .status_label
            .set_label(&gettext("This folder is not a Git repository."));
        emit_project_statuses(&state);
        actions::rebuild_rows(&mut state);
        return;
    }
    let cancellable = gio::Cancellable::new();
    state.borrow_mut().cancellable = Some(cancellable.clone());
    let weak = Rc::downgrade(state);
    GitProcess::detect_repo(
        &path,
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if let Ok(repo) = result {
                {
                    let mut state = state.borrow_mut();
                    state.repo = Some(repo.clone());
                    state.process = Some(GitProcess::new(repo));
                    state.status_stale = true;
                }
                refresh_status(&state);
            } else {
                let mut state = state.borrow_mut();
                state.repo = None;
                state.process = None;
                state
                    .status_label
                    .set_label(&gettext("This folder is not a Git repository."));
                emit_project_statuses(&state);
                actions::rebuild_rows(&mut state);
            }
        }),
    );
}

fn has_git_metadata_candidate(path: &Path) -> bool {
    path.ancestors()
        .any(|ancestor| ancestor.join(".git").exists())
}

pub(super) fn refresh_status(state: &SourceStateRef) {
    cancel_refresh(state);
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let cancellable = gio::Cancellable::new();
    {
        let mut state = state.borrow_mut();
        state.cancellable = Some(cancellable.clone());
        state.status_stale = true;
        state
            .status_label
            .set_label(&gettext("Refreshing Git status..."));
    }
    let weak = Rc::downgrade(state);
    process.check_repo_capabilities(
        &cancellable,
        Rc::new(move |capabilities| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let Ok(capabilities) = capabilities else {
                finish_error(
                    &state,
                    &gettext("Unable to read Git repository capabilities."),
                );
                return;
            };
            state.borrow_mut().capabilities = capabilities;
            if !capabilities.object_format_supported || !capabilities.eol_supported {
                finish_unsupported_repo(&state);
                return;
            }
            refresh_status_entries(&state);
        }),
    );
}

fn refresh_status_entries(state: &SourceStateRef) {
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let Some(cancellable) = state.borrow().cancellable.clone() else {
        return;
    };
    let weak = Rc::downgrade(state);
    process.status(
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let Ok(snapshot) = result else {
                finish_error(&state, &gettext("Unable to refresh Git status."));
                return;
            };
            let paths = snapshot.changed_paths();
            if paths.is_empty() {
                let mut state = state.borrow_mut();
                state.snapshot = snapshot;
                state.attrs = GitAttrs::default();
                state.status_stale = false;
                state.status_label.set_label(&gettext("No changes."));
                emit_project_statuses(&state);
                actions::rebuild_rows(&mut state);
                return;
            }
            refresh_attrs(&state, snapshot, &paths);
        }),
    );
}

fn refresh_attrs(state: &SourceStateRef, snapshot: GitStatusSnapshot, paths: &[GitPath]) {
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let Some(cancellable) = state.borrow().cancellable.clone() else {
        return;
    };
    let weak = Rc::downgrade(state);
    process.check_attrs(
        paths,
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let attrs = result.unwrap_or_default();
            let mut state = state.borrow_mut();
            state.snapshot = snapshot.clone();
            state.attrs = attrs;
            state.status_stale = false;
            let branch = state
                .snapshot
                .branch
                .clone()
                .unwrap_or_else(|| pgettext("git branch", "Detached"));
            state.title.set_subtitle(&branch);
            state.status_label.set_label(&gettext("Changed files"));
            actions::apply_entry_actions(&mut state);
            emit_project_statuses(&state);
            actions::rebuild_rows(&mut state);
        }),
    );
}

pub(super) fn finish_error(state: &SourceStateRef, message: &str) {
    let mut state = state.borrow_mut();
    state.status_stale = true;
    state.status_label.set_label(message);
    state.commit_button.set_sensitive(false);
    emit_project_statuses(&state);
    actions::rebuild_rows(&mut state);
}

fn finish_unsupported_repo(state: &SourceStateRef) {
    let mut state = state.borrow_mut();
    state.status_stale = false;
    state.commit_button.set_sensitive(false);
    state.status_label.set_label(&gettext(
        "This Git repository uses unsupported object or EOL settings.",
    ));
    emit_project_statuses(&state);
    actions::rebuild_rows(&mut state);
}

fn emit_project_statuses(state: &SourceControlState) {
    let Some(handler) = state.status_handler.as_ref() else {
        return;
    };
    let Some(repo) = state.repo.as_ref() else {
        handler(Vec::new());
        return;
    };
    let statuses = state
        .snapshot
        .entries
        .iter()
        .filter_map(|entry| {
            let path = entry.path.as_utf8()?;
            let uri = gio::File::for_path(repo.join(path)).uri().to_string();
            Some((uri, String::from(entry.status.badge())))
        })
        .collect();
    handler(statuses);
}

fn commit(state: &SourceStateRef) {
    let (process, message, settings_identity) = {
        let state = state.borrow();
        let Some(process) = state.process.clone() else {
            return;
        };
        let message = state.commit_entry.text().to_string();
        let identity = state.settings.git_identity();
        (process, message, identity)
    };
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
            let Some(state) = weak.upgrade() else {
                return;
            };
            let identity = identity.ok().flatten().or_else(|| {
                GitIdentity::new(settings_identity.0.clone(), settings_identity.1.clone()).ok()
            });
            let Some(identity) = identity else {
                finish_error(
                    &state,
                    &gettext("Set a Git identity in Preferences before committing."),
                );
                return;
            };
            let weak = Rc::downgrade(&state);
            process_for_commit.commit(
                &identity,
                &message,
                &cancellable_for_commit,
                Rc::new(move |result| {
                    let Some(state) = weak.upgrade() else {
                        return;
                    };
                    match result {
                        Ok(()) => {
                            state.borrow().commit_entry.set_text("");
                            refresh_status(&state);
                        }
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
        _ => gettext("The Git operation failed."),
    }
}

fn cancel_refresh(state: &SourceStateRef) {
    if let Some(cancellable) = state.borrow_mut().cancellable.take() {
        cancellable.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::{GitProcessError, git_error_text};

    #[test]
    fn git_errors_map_to_user_copy() {
        assert_eq!(
            git_error_text(&GitProcessError::InvalidIdentity),
            "The Git identity is not valid."
        );
        assert_eq!(
            git_error_text(&GitProcessError::OutputTooLarge),
            "Git output was too large to process safely."
        );
        assert_eq!(
            git_error_text(&GitProcessError::ParseFailed),
            "The Git operation failed."
        );
    }
}
