use std::path::PathBuf;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, glib, prelude::*};

use crate::dialogs::{self, GitDiscardResponse};
use crate::editor_tab::EditorTab;
use crate::error::AppError;
use crate::git_process::GitProcessError;
use crate::git_status::{GitActionState, GitFileStatus, GitStatusEntry};
use crate::source_control::{
    SourceControlState, SourceStateRef, git_error_is_cancelled, git_error_text,
    set_commit_controls_enabled, set_status_label,
};
use crate::workspace::{OpenSource, Workspace};

use super::operation_bridge;
use super::refresh::finish_error;
use super::slots::{DiffTicket, MutationTicket, SnapshotId};

mod model;

use model::{
    commit_sensitive, discard_state, entry_disabled_reason, reference_oid, reference_text,
    should_stage_delete, stage_mode_for_entry, too_many_changes_text,
};

#[derive(Clone, Copy)]
pub(crate) enum GitRowAction {
    Diff,
    Stage,
    Unstage,
    Discard,
}

#[derive(Clone)]
struct ActionTarget {
    repo: PathBuf,
    snapshot: SnapshotId,
    entry: GitStatusEntry,
}

pub(super) fn apply_entry_actions(state: &mut SourceControlState, dirty_uris: &[String]) -> bool {
    let repo = state.repo.clone();
    let too_large = state.snapshot.too_large;
    let writes_enabled = writes_enabled(state);
    for entry in &mut state.snapshot.entries {
        let disabled = entry_disabled_reason(repo.as_deref(), entry, &state.attrs, dirty_uris);
        if let Some(reason) = disabled {
            entry.stage_action = GitActionState::Disabled(reason.clone());
            entry.unstage_action = GitActionState::Disabled(reason.clone());
            entry.discard_action = GitActionState::Disabled(reason.clone());
            entry.diff_action = GitActionState::Disabled(reason);
            continue;
        }
        if too_large {
            let reason = too_many_changes_text();
            entry.stage_action = GitActionState::Disabled(reason.clone());
            entry.unstage_action = GitActionState::Disabled(reason.clone());
            entry.discard_action = GitActionState::Disabled(reason);
            entry.diff_action = GitActionState::Enabled;
            continue;
        }
        if !writes_enabled {
            let reason = gettext("Refreshing Git status");
            entry.stage_action = GitActionState::Disabled(reason.clone());
            entry.unstage_action = GitActionState::Disabled(reason.clone());
            entry.discard_action = GitActionState::Disabled(reason);
            entry.diff_action = GitActionState::Enabled;
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
    commit_sensitive(&state.snapshot, &state.attrs, writes_enabled)
}

pub(super) fn commit_is_eligible(state: &SourceControlState) -> bool {
    commit_sensitive(&state.snapshot, &state.attrs, writes_enabled(state))
}

fn writes_enabled(state: &SourceControlState) -> bool {
    !state.status_stale
        && !state.operations.mutation_active()
        && state
            .snapshot_id
            .as_ref()
            .is_some_and(|snapshot| state.operations.is_snapshot_current(snapshot))
}

pub(crate) fn run_path_action(state: &SourceStateRef, path: &[u8], action: GitRowAction) {
    let target = {
        let state = state.borrow();
        let Some(repo) = state.repo.clone() else {
            return;
        };
        let Some(snapshot) = state.snapshot_id.clone() else {
            return;
        };
        if !state.operations.is_snapshot_current(&snapshot) {
            return;
        }
        let entry = state
            .snapshot
            .entries
            .iter()
            .find(|entry| entry.path.raw() == path)
            .cloned();
        entry.map(|entry| ActionTarget {
            repo,
            snapshot,
            entry,
        })
    };
    let Some(target) = target else {
        return;
    };
    if let Some(reason) = action_disabled_reason(&target.entry, action) {
        set_status_label(state, reason);
        return;
    }
    match action {
        GitRowAction::Diff => diff_entry(state, target),
        GitRowAction::Stage => stage_entry(state, &target),
        GitRowAction::Unstage => unstage_entry(state, &target),
        GitRowAction::Discard => confirm_discard_entry(state, target),
    }
}

pub(crate) fn fire_state_change_handler(state: &SourceStateRef) {
    super::review::sync_actions(state);
    let handler = { state.borrow().state_change_handler.as_ref().map(Rc::clone) };
    if let Some(handler) = handler {
        handler();
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

fn stage_entry(state: &SourceStateRef, target: &ActionTarget) {
    let Some((process, ticket)) = begin_action(state, target) else {
        return;
    };
    let cancellable = ticket.cancellable().clone();
    if should_stage_delete(&target.entry) {
        process.remove_from_index(
            &target.entry.path,
            &cancellable,
            action_callback(state, ticket),
        );
        return;
    }
    let Some(mode) = stage_mode_for_entry(&target.entry) else {
        finish_mutation_message(
            state,
            &ticket,
            &gettext("This file type cannot be staged from Riteed."),
        );
        return;
    };
    let process_for_index = process.clone();
    let path_for_index = target.entry.path.clone();
    let cancellable_for_index = cancellable.clone();
    let weak = Rc::downgrade(state);
    process.hash_file_no_filters(
        &target.entry.path,
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if !operation_bridge::is_mutation_current(&state, &ticket) {
                finish_mutation(&state, &ticket, None);
                return;
            }
            match result {
                Ok(oid) if mutation_can_spawn(&state, &ticket) => {
                    process_for_index.stage_blob_index_info(
                        mode,
                        &oid,
                        &path_for_index,
                        &cancellable_for_index,
                        action_callback(&state, ticket.clone()),
                    );
                }
                Ok(_oid) => finish_mutation(&state, &ticket, None),
                Err(error) => finish_mutation(&state, &ticket, Some(&error)),
            }
        }),
    );
}

fn unstage_entry(state: &SourceStateRef, target: &ActionTarget) {
    let Some((process, ticket)) = begin_action(state, target) else {
        return;
    };
    let cancellable = ticket.cancellable().clone();
    process.unstage_path(
        &target.entry.path,
        &cancellable,
        action_callback(state, ticket),
    );
}

fn confirm_discard_entry(state: &SourceStateRef, target: ActionTarget) {
    let parent = state.borrow().root.clone();
    let name = target.entry.path.display().to_string();
    let weak = Rc::downgrade(state);
    dialogs::confirm_git_discard(&parent, &name, move |response| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        if response == GitDiscardResponse::Discard && target_is_current(&state, &target) {
            discard_entry(&state, &target);
        }
    });
}

fn discard_entry(state: &SourceStateRef, target: &ActionTarget) {
    let Some((process, ticket)) = begin_action(state, target) else {
        return;
    };
    let cancellable = ticket.cancellable().clone();
    process.restore_worktree_path(
        &target.entry.path,
        &cancellable,
        action_callback(state, ticket),
    );
}

fn diff_entry(state: &SourceStateRef, target: ActionTarget) {
    let Some((process, ticket)) = begin_diff_action(state, &target) else {
        return;
    };
    let cancellable = ticket.cancellable().clone();
    if target.entry.status == GitFileStatus::Untracked {
        compare_with_text(state, &target, String::new(), ticket);
        return;
    }
    let Some(oid) = reference_oid(&target.entry) else {
        let _finished = operation_bridge::finish_diff(state, &ticket);
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
            if !operation_bridge::is_diff_current(&state, &ticket) {
                let _finished = operation_bridge::finish_diff(&state, &ticket);
                return;
            }
            match result.and_then(reference_text) {
                Ok(text) => compare_with_text(&state, &target, text, ticket.clone()),
                Err(error) if git_error_is_cancelled(&error) => {
                    let _finished = operation_bridge::finish_diff(&state, &ticket);
                }
                Err(error) => {
                    let _finished = operation_bridge::finish_diff(&state, &ticket);
                    finish_error(&state, &git_error_text(&error));
                }
            }
        }),
    );
}

fn compare_with_text(
    state: &SourceStateRef,
    target: &ActionTarget,
    text: String,
    ticket: DiffTicket,
) {
    if !operation_bridge::is_diff_current(state, &ticket) || !target_is_current(state, target) {
        let _finished = operation_bridge::finish_diff(state, &ticket);
        return;
    }
    let Some((workspace, file)) = workspace_file_for_target(state, target) else {
        let _finished = operation_bridge::finish_diff(state, &ticket);
        finish_error(state, &gettext("Open the file before comparing it."));
        return;
    };
    let uri = file.uri().to_string();
    let Some(tab) = workspace
        .ordered_tabs()
        .into_iter()
        .find(|tab| tab.session_uri().as_deref() == Some(uri.as_str()))
    else {
        let weak = Rc::downgrade(state);
        let target = target.clone();
        let text = Rc::new(text);
        workspace.request_open_file_then(
            &file,
            OpenSource::SourceControl,
            Rc::new(move |result| {
                let Some(state) = weak.upgrade() else {
                    return;
                };
                if !operation_bridge::is_diff_current(&state, &ticket)
                    || !target_is_current(&state, &target)
                {
                    let _finished = operation_bridge::finish_diff(&state, &ticket);
                    return;
                }
                match result {
                    Ok(tab) => queue_start_git_compare(
                        &tab,
                        (*text).clone(),
                        &state,
                        ticket.clone(),
                        target.clone(),
                    ),
                    Err(error) if should_finish_open_with_error(&error) => {
                        let _finished = operation_bridge::finish_diff(&state, &ticket);
                        finish_error(&state, &gettext("Unable to open file for compare."));
                    }
                    Err(_) => finish_cancelled_open(&state, &ticket),
                }
            }),
        );
        return;
    };
    queue_start_git_compare(&tab, text, state, ticket, target.clone());
}

fn finish_cancelled_open(state: &SourceStateRef, ticket: &DiffTicket) {
    if !operation_bridge::finish_diff(state, ticket) {
        return;
    }
    fire_state_change_handler(state);
}

fn should_finish_open_with_error(error: &AppError) -> bool {
    !matches!(error, AppError::DocumentChangedDuringRead)
}

fn start_git_compare(
    tab: &Rc<EditorTab>,
    text: String,
    state: &SourceStateRef,
    ticket: DiffTicket,
) {
    let weak = Rc::downgrade(state);
    tab.start_compare_with_reference_text(
        pgettext("compare source", "Git Version"),
        text,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            let current = operation_bridge::is_diff_current(&state, &ticket);
            let _finished = operation_bridge::finish_diff(&state, &ticket);
            if result.is_err() && current {
                finish_error(&state, &gettext("Unable to start Git compare."));
            }
        }),
    );
}

fn queue_start_git_compare(
    tab: &Rc<EditorTab>,
    text: String,
    state: &SourceStateRef,
    ticket: DiffTicket,
    target: ActionTarget,
) {
    let weak_state = Rc::downgrade(state);
    let weak_tab = Rc::downgrade(tab);
    let _source = glib::idle_add_local_once(move || {
        let Some(state) = weak_state.upgrade() else {
            return;
        };
        let Some(tab) = weak_tab.upgrade() else {
            let _finished = operation_bridge::finish_diff(&state, &ticket);
            return;
        };
        if !operation_bridge::is_diff_current(&state, &ticket)
            || !target_is_current(&state, &target)
        {
            let _finished = operation_bridge::finish_diff(&state, &ticket);
            return;
        }
        start_git_compare(&tab, text, &state, ticket);
    });
}

fn begin_action(
    state: &SourceStateRef,
    target: &ActionTarget,
) -> Option<(crate::git_process::GitProcess, MutationTicket)> {
    let process = {
        let state = state.borrow();
        if state.status_stale || !target_is_current_locked(&state, target) {
            return None;
        }
        state.process.clone()
    };
    let process = process?;
    if operation_bridge::native_index_lock_exists(state) {
        show_locked_wait(state);
        return None;
    }
    let ticket = operation_bridge::try_begin_mutation(state, &target.repo)?;
    set_commit_controls_enabled(state, false);
    fire_state_change_handler(state);
    if !mutation_can_spawn(state, &ticket) {
        finish_mutation(state, &ticket, None);
        return None;
    }
    Some((process, ticket))
}

fn begin_diff_action(
    state: &SourceStateRef,
    target: &ActionTarget,
) -> Option<(crate::git_process::GitProcess, DiffTicket)> {
    let process = {
        let state = state.borrow();
        if !target_is_current_locked(&state, target) {
            return None;
        }
        state.process.clone()?
    };
    let ticket = operation_bridge::begin_diff(state)?;
    if !operation_bridge::is_diff_current(state, &ticket) {
        let _finished = operation_bridge::finish_diff(state, &ticket);
        return None;
    }
    Some((process, ticket))
}

fn action_callback(
    state: &SourceStateRef,
    ticket: MutationTicket,
) -> Rc<dyn Fn(Result<(), GitProcessError>)> {
    let weak = Rc::downgrade(state);
    Rc::new(move |result| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        finish_mutation(&state, &ticket, result.as_ref().err());
    })
}

pub(super) fn mutation_can_spawn(state: &SourceStateRef, ticket: &MutationTicket) -> bool {
    !ticket.cancellable().is_cancelled()
        && operation_bridge::is_mutation_current(state, ticket)
        && operation_bridge::mutation_root_is_current(state, ticket)
        && state.borrow().repo.as_deref() == Some(ticket.repo())
}

pub(super) fn finish_mutation(
    state: &SourceStateRef,
    ticket: &MutationTicket,
    error: Option<&GitProcessError>,
) {
    let matched = operation_bridge::finish_mutation(state, ticket);
    let current = operation_bridge::mutation_root_is_current(state, ticket);
    if !matched {
        return;
    }
    if let (true, Some(error)) = (
        current,
        error.filter(|error| !git_error_is_cancelled(error)),
    ) {
        finish_error(state, &git_error_text(error));
    }
    super::live::schedule(state);
}

pub(super) fn finish_mutation_message(
    state: &SourceStateRef,
    ticket: &MutationTicket,
    message: &str,
) {
    let matched = operation_bridge::finish_mutation(state, ticket);
    let current = operation_bridge::mutation_root_is_current(state, ticket);
    if !matched {
        return;
    }
    if current {
        finish_error(state, message);
    }
    super::live::schedule(state);
}

fn entry_matches_snapshot(state: &SourceControlState, entry: &GitStatusEntry) -> bool {
    state.snapshot.entries.iter().any(|current| {
        current.path.raw() == entry.path.raw()
            && current.status == entry.status
            && current.head_oid == entry.head_oid
            && current.index_oid == entry.index_oid
            && current.staged == entry.staged
            && current.unstaged == entry.unstaged
            && current.worktree_mode == entry.worktree_mode
    })
}

fn target_is_current(state: &SourceStateRef, target: &ActionTarget) -> bool {
    target_is_current_locked(&state.borrow(), target)
}

fn target_is_current_locked(state: &SourceControlState, target: &ActionTarget) -> bool {
    state.repo.as_ref() == Some(&target.repo)
        && state.operations.is_snapshot_current(&target.snapshot)
        && entry_matches_snapshot(state, &target.entry)
}

pub(super) fn show_locked_wait(state: &SourceStateRef) {
    set_status_label(
        state,
        &super::refresh::ellipsis_label(gettext("Waiting for another Git operation to finish")),
    );
    super::live::schedule(state);
}

pub(super) fn dirty_open_uris(state: &SourceStateRef) -> Vec<String> {
    let workspace = state.borrow().workspace.upgrade();
    let Some(workspace) = workspace else {
        return Vec::new();
    };
    workspace
        .ordered_tabs()
        .into_iter()
        .filter(|tab| tab.is_dirty())
        .filter_map(|tab| tab.document_uri())
        .collect()
}

fn workspace_file_for_target(
    state: &SourceStateRef,
    target: &ActionTarget,
) -> Option<(Rc<Workspace>, gio::File)> {
    let workspace = state.borrow().workspace.upgrade()?;
    let path = target.entry.path.as_utf8()?;
    Some((workspace, gio::File::for_path(target.repo.join(path))))
}

#[cfg(test)]
mod tests;
