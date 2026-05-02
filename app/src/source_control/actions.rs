use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};

use crate::dialogs::{self, GitDiscardResponse};
use crate::editor_tab::EditorTab;
use crate::git_process::GitProcessError;
use crate::git_status::{
    GitActionState, GitAttrState, GitFileStatus, GitPath, GitStatusEntry, GitStatusSnapshot,
};
use crate::source_control::{
    SourceControlState, SourceStateRef, git_attrs_unavailable_text, git_error_text,
};
use crate::workspace::{OpenSource, Workspace};

use super::refresh::{finish_error, refresh_status};

#[derive(Clone, Copy)]
pub(super) enum GitRowAction {
    Diff,
    Stage,
    Unstage,
    Discard,
}

pub(super) fn apply_entry_actions(state: &mut SourceControlState) {
    let repo = state.repo.clone();
    let dirty_uris = dirty_open_uris(state);
    for entry in &mut state.snapshot.entries {
        let disabled = entry_disabled_reason(repo.as_deref(), entry, &state.attrs, &dirty_uris);
        if let Some(reason) = disabled {
            entry.stage_action = GitActionState::Disabled(reason.clone());
            entry.unstage_action = GitActionState::Disabled(reason.clone());
            entry.discard_action = GitActionState::Disabled(reason.clone());
            entry.diff_action = GitActionState::Disabled(reason);
            continue;
        }
        entry.stage_action = if entry.unstaged {
            GitActionState::Enabled
        } else {
            GitActionState::Disabled(pgettext("git action disabled", "No unstaged change"))
        };
        entry.unstage_action = if entry.staged {
            GitActionState::Enabled
        } else {
            GitActionState::Disabled(pgettext("git action disabled", "No staged change"))
        };
        entry.discard_action = discard_state(entry);
        entry.diff_action = GitActionState::Enabled;
    }
    let can_commit = commit_sensitive(&state.snapshot, &state.attrs, state.status_stale);
    state.commit_button.set_sensitive(can_commit);
}

fn commit_sensitive(
    snapshot: &GitStatusSnapshot,
    attrs: &GitAttrState,
    status_stale: bool,
) -> bool {
    !status_stale
        && !attrs.is_unavailable()
        && snapshot
            .entries
            .iter()
            .any(|entry| entry.staged && entry.unstage_action.enabled())
}

pub(super) fn run_path_action(state: &SourceStateRef, path: &[u8], action: GitRowAction) {
    let entry = state
        .borrow()
        .snapshot
        .entries
        .iter()
        .find(|entry| entry.path.raw() == path)
        .cloned();
    let Some(entry) = entry else {
        return;
    };
    if let Some(reason) = action_disabled_reason(&entry, action) {
        state.borrow().status_label.set_label(reason);
        return;
    }
    match action {
        GitRowAction::Diff => diff_entry(state, entry),
        GitRowAction::Stage => stage_entry(state, &entry),
        GitRowAction::Unstage => unstage_entry(state, &entry),
        GitRowAction::Discard => confirm_discard_entry(state, entry),
    }
}

fn discard_state(entry: &GitStatusEntry) -> GitActionState {
    if entry.status == GitFileStatus::Untracked {
        GitActionState::Disabled(pgettext("git action disabled", "Untracked file"))
    } else if !entry.unstaged {
        GitActionState::Disabled(pgettext("git action disabled", "No unstaged change"))
    } else {
        GitActionState::Enabled
    }
}

fn action_disabled_reason(entry: &GitStatusEntry, action: GitRowAction) -> Option<&str> {
    let state = match action {
        GitRowAction::Diff => &entry.diff_action,
        GitRowAction::Stage => &entry.stage_action,
        GitRowAction::Unstage => &entry.unstage_action,
        GitRowAction::Discard => &entry.discard_action,
    };
    match state {
        GitActionState::Enabled => None,
        GitActionState::Disabled(reason) => Some(reason),
    }
}

fn stage_entry(state: &SourceStateRef, entry: &GitStatusEntry) {
    let (process, repo, cancellable, _generation) = begin_action(state);
    let Some(process) = process else {
        return;
    };
    if entry.status == GitFileStatus::Deleted && entry.unstaged {
        process.remove_from_index(&entry.path, &cancellable, action_callback(state));
        return;
    }
    let Some(mode) = repo
        .as_deref()
        .and_then(|repo| mode_for_path(repo, &entry.path))
    else {
        finish_error(
            state,
            &gettext("This file type cannot be staged from Riteed."),
        );
        return;
    };
    let process_for_index = process.clone();
    let path_for_index = entry.path.clone();
    let cancellable_for_index = cancellable.clone();
    let weak = Rc::downgrade(state);
    process.hash_file_no_filters(
        &entry.path,
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            match result {
                Ok(oid) => process_for_index.stage_blob_index_info(
                    mode,
                    &oid,
                    &path_for_index,
                    &cancellable_for_index,
                    action_callback(&state),
                ),
                Err(error) => finish_error(&state, &git_error_text(&error)),
            }
        }),
    );
}

fn unstage_entry(state: &SourceStateRef, entry: &GitStatusEntry) {
    let (process, _repo, cancellable, _generation) = begin_action(state);
    let Some(process) = process else {
        return;
    };
    process.unstage_path(&entry.path, &cancellable, action_callback(state));
}

fn confirm_discard_entry(state: &SourceStateRef, entry: GitStatusEntry) {
    let parent = state.borrow().root.clone();
    let name = entry.path.display().to_string();
    let weak = Rc::downgrade(state);
    dialogs::confirm_git_discard(&parent, &name, move |response| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        if response == GitDiscardResponse::Discard {
            discard_entry(&state, &entry);
        }
    });
}

fn discard_entry(state: &SourceStateRef, entry: &GitStatusEntry) {
    let (process, _repo, cancellable, _generation) = begin_action(state);
    let Some(process) = process else {
        return;
    };
    process.restore_worktree_path(&entry.path, &cancellable, action_callback(state));
}

fn diff_entry(state: &SourceStateRef, entry: GitStatusEntry) {
    let (process, repo, cancellable, generation) = begin_diff_action(state);
    let Some(process) = process else {
        return;
    };
    if entry.status == GitFileStatus::Untracked {
        compare_with_text(state, &entry, String::new(), generation);
        return;
    }
    let Some(oid) = reference_oid(&entry) else {
        finish_error(state, &gettext("Diff unavailable for this Git state."));
        return;
    };
    let weak = Rc::downgrade(state);
    process.cat_blob(
        &oid,
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if !generation_matches(&state, generation) {
                return;
            }
            match result.and_then(reference_text) {
                Ok(text) => compare_with_text(&state, &entry, text, generation),
                Err(error) => finish_error(&state, &git_error_text(&error)),
            }
        }),
    );
    drop(repo);
}

fn compare_with_text(
    state: &SourceStateRef,
    entry: &GitStatusEntry,
    text: String,
    generation: u64,
) {
    if !entry_matches_snapshot(&state.borrow(), entry) {
        return;
    }
    let Some((workspace, file)) = workspace_file_for_entry(&state.borrow(), entry) else {
        finish_error(state, &gettext("Open the file before comparing it."));
        return;
    };
    let uri = file.uri().to_string();
    let Some(tab) = workspace
        .ordered_tabs()
        .into_iter()
        .find(|tab| tab.uri().as_deref() == Some(uri.as_str()))
    else {
        let weak = Rc::downgrade(state);
        let entry = entry.clone();
        let text = Rc::new(text);
        workspace.request_open_file_then(
            &file,
            OpenSource::SourceControl,
            Rc::new(move |result| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if !generation_matches(&state, generation)
                    || !entry_matches_snapshot(&state.borrow(), &entry)
                {
                    return;
                }
                match result {
                    Ok(tab) => start_git_compare(&tab, (*text).clone(), &state, generation),
                    Err(_error) => {
                        finish_error(&state, &gettext("Unable to open file for compare."));
                    }
                }
            }),
        );
        return;
    };
    start_git_compare(&tab, text, state, generation);
}

fn start_git_compare(tab: &Rc<EditorTab>, text: String, state: &SourceStateRef, generation: u64) {
    let weak = Rc::downgrade(state);
    tab.start_compare_with_reference_text(
        pgettext("compare source", "Git Version"),
        text,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if result.is_err() && generation_matches(&state, generation) {
                finish_error(&state, &gettext("Unable to start Git compare."));
            }
        }),
    );
}

fn begin_action(
    state: &SourceStateRef,
) -> (
    Option<crate::git_process::GitProcess>,
    Option<PathBuf>,
    gio::Cancellable,
    u64,
) {
    let cancellable = gio::Cancellable::new();
    let (process, repo, generation) = {
        let mut state = state.borrow_mut();
        if let Some(previous) = state.cancellable.take() {
            previous.cancel();
        }
        state.action_generation = state.action_generation.wrapping_add(1);
        state.cancellable = Some(cancellable.clone());
        state.status_stale = true;
        state.commit_button.set_sensitive(false);
        (
            state.process.clone(),
            state.repo.clone(),
            state.action_generation,
        )
    };
    (process, repo, cancellable, generation)
}

fn begin_diff_action(
    state: &SourceStateRef,
) -> (
    Option<crate::git_process::GitProcess>,
    Option<PathBuf>,
    gio::Cancellable,
    u64,
) {
    let cancellable = gio::Cancellable::new();
    let (process, repo, generation) = {
        let mut state = state.borrow_mut();
        if let Some(previous) = state.cancellable.take() {
            previous.cancel();
        }
        state.action_generation = state.action_generation.wrapping_add(1);
        state.cancellable = Some(cancellable.clone());
        (
            state.process.clone(),
            state.repo.clone(),
            state.action_generation,
        )
    };
    (process, repo, cancellable, generation)
}

fn action_callback(state: &SourceStateRef) -> Rc<dyn Fn(Result<(), GitProcessError>)> {
    let weak = Rc::downgrade(state);
    Rc::new(move |result| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        match result {
            Ok(()) => refresh_status(&state),
            Err(error) => finish_error(&state, &git_error_text(&error)),
        }
    })
}

fn generation_matches(state: &SourceStateRef, generation: u64) -> bool {
    state.borrow().action_generation == generation
}

fn entry_matches_snapshot(state: &SourceControlState, entry: &GitStatusEntry) -> bool {
    state.snapshot.entries.iter().any(|current| {
        current.path.raw() == entry.path.raw()
            && current.status == entry.status
            && current.head_oid == entry.head_oid
            && current.index_oid == entry.index_oid
            && current.staged == entry.staged
            && current.unstaged == entry.unstaged
    })
}

fn entry_disabled_reason(
    repo: Option<&Path>,
    entry: &GitStatusEntry,
    attrs: &GitAttrState,
    dirty_uris: &[String],
) -> Option<String> {
    if entry.path.as_utf8().is_none() {
        return Some(gettext("This path uses an unsupported encoding."));
    }
    if matches!(
        entry.status,
        GitFileStatus::Conflicted | GitFileStatus::Unsupported
    ) {
        return Some(gettext(
            "This Git state is visible but not editable in Riteed.",
        ));
    }
    if attrs.is_unavailable() {
        return Some(git_attrs_unavailable_text());
    }
    if attrs.blocks(entry.path.raw()) {
        return Some(gettext(
            "Git content filters or EOL conversion are configured for this file.",
        ));
    }
    let Some((repo, path)) = repo.zip(entry.path.as_utf8()) else {
        return Some(gettext("No Git repository is active."));
    };
    let full_path = repo.join(path);
    if symlink_or_unsupported_mode(&full_path) {
        return Some(gettext(
            "Symlinks and unsupported file modes are visible only.",
        ));
    }
    let uri = gio::File::for_path(full_path).uri().to_string();
    if dirty_uris.iter().any(|dirty| dirty == &uri) {
        return Some(gettext("Save the open document before using Git actions."));
    }
    None
}

fn dirty_open_uris(state: &SourceControlState) -> Vec<String> {
    let Some(workspace) = state.workspace.upgrade() else {
        return Vec::new();
    };
    workspace
        .ordered_tabs()
        .into_iter()
        .filter(|tab| tab.is_dirty())
        .filter_map(|tab| tab.uri())
        .collect()
}

fn mode_for_path(repo: &Path, path: &GitPath) -> Option<&'static str> {
    let full_path = repo.join(path.as_utf8()?);
    let metadata = std::fs::symlink_metadata(full_path).ok()?;
    if !metadata.file_type().is_file() {
        return None;
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        Some("100644")
    } else {
        Some("100755")
    }
}

fn symlink_or_unsupported_mode(path: &Path) -> bool {
    let Ok(metadata) = std::fs::symlink_metadata(path) else {
        return false;
    };
    metadata.file_type().is_symlink()
}

fn workspace_file_for_entry(
    state: &SourceControlState,
    entry: &GitStatusEntry,
) -> Option<(Rc<Workspace>, gio::File)> {
    let workspace = state.workspace.upgrade()?;
    let repo = state.repo.as_ref()?;
    let path = entry.path.as_utf8()?;
    Some((workspace, gio::File::for_path(repo.join(path))))
}

fn reference_oid(entry: &GitStatusEntry) -> Option<String> {
    if entry.staged && !entry.unstaged {
        return entry.head_oid.clone();
    }
    entry.index_oid.clone().or_else(|| entry.head_oid.clone())
}

fn reference_text(output: Vec<u8>) -> Result<String, GitProcessError> {
    if output.iter().take(8192).any(|byte| *byte == 0) {
        return Err(GitProcessError::OutputTooLarge);
    }
    String::from_utf8(output).map_err(|_| GitProcessError::ParseFailed)
}

#[cfg(test)]
mod tests;
