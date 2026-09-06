use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, prelude::*};

use crate::git_process::{GitCallback, GitProcess, GitRepoContext};
use crate::git_status::{GitAttrState, GitStatusSnapshot};

use super::refresh::{
    RefreshOrigin, ellipsis_label, emit_project_statuses, rebuild_views, refresh_status_with_origin,
};
use super::slots::{RefreshTicket, cancel_queued};
use super::{
    SourceControlState, SourceStateRef, actions, cancel_minimap_requests, cancel_review_requests,
    live, operation_bridge, set_commit_controls_enabled, set_status_label,
};

// PARSER-BOUNDARY: id=git_status_ui
pub(super) fn set_project_root(state: &SourceStateRef, folder: Option<gio::File>) {
    let ticket = begin_root_change(state);
    if !operation_bridge::is_refresh_current(state, &ticket) {
        return;
    }
    live::cancel(state);
    if !operation_bridge::is_refresh_current(state, &ticket) {
        return;
    }
    cancel_review_requests(state);
    if !operation_bridge::is_refresh_current(state, &ticket) {
        return;
    }
    cancel_minimap_requests(state);
    if !operation_bridge::is_refresh_current(state, &ticket) {
        return;
    }
    let Some(folder) = folder else {
        if reset_project_state(
            state,
            &ticket,
            &gettext("Open a folder to see Git status."),
            false,
        ) {
            let _finished = operation_bridge::finish_refresh(state, &ticket);
        }
        return;
    };
    let Some(path) = folder.path() else {
        if reset_project_state(
            state,
            &ticket,
            &gettext("Only local Git folders are supported."),
            false,
        ) {
            let _finished = operation_bridge::finish_refresh(state, &ticket);
        }
        return;
    };
    if !reset_project_state(
        state,
        &ticket,
        &ellipsis_label(gettext("Refreshing Git status")),
        true,
    ) {
        return;
    }
    let weak = Rc::downgrade(state);
    let cancellable = ticket.cancellable().clone();
    let callback: GitCallback<GitRepoContext> = Rc::new(move |result| {
        let Some(state) = weak.upgrade() else {
            return;
        };
        if !operation_bridge::is_refresh_current(&state, &ticket) {
            return;
        }
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
                if !operation_bridge::is_refresh_current(&state, &ticket) {
                    return;
                }
                refresh_status_with_origin(&state, RefreshOrigin::Initial);
            }
            Err(error) if super::git_error_is_cancelled(&error) => {
                let _finished = operation_bridge::finish_refresh(&state, &ticket);
            }
            Err(error) if matches!(error, crate::git_process::GitProcessError::TimedOut) => {
                if reset_project_state(&state, &ticket, &super::git_error_text(&error), true) {
                    let _finished = operation_bridge::finish_refresh(&state, &ticket);
                }
            }
            Err(_error) => {
                if reset_project_state(
                    &state,
                    &ticket,
                    &gettext("This folder is not a Git repository."),
                    false,
                ) {
                    let _finished = operation_bridge::finish_refresh(&state, &ticket);
                }
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

fn begin_root_change(state: &SourceStateRef) -> RefreshTicket {
    let (ticket, cancellations) = {
        let mut state = state.borrow_mut();
        state.operations.invalidate_root();
        state.snapshot_id = None;
        let ticket = state.operations.begin_detection();
        let cancellations = state.operations.take_cancellations();
        (ticket, cancellations)
    };
    cancel_queued(cancellations);
    ticket
}

fn reset_project_state(
    state: &SourceStateRef,
    ticket: &RefreshTicket,
    label: &str,
    mark_stale: bool,
) -> bool {
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    let history = {
        let mut state = state.borrow_mut();
        state.repo = None;
        state.process = None;
        state.attrs = GitAttrState::default();
        state.snapshot = GitStatusSnapshot::default();
        state.status_stale = mark_stale;
        state.review_generation = state.review_generation.wrapping_add(1);
        Rc::clone(&state.history)
    };
    set_status_label(state, label);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    set_commit_controls_enabled(state, false);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    emit_project_statuses(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    history.clear();
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    rebuild_views(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    actions::fire_state_change_handler(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return false;
    }
    close_review_tabs_for_current_repo(state);
    operation_bridge::is_refresh_current(state, ticket)
}

fn close_review_tabs_for_current_repo(state: &SourceStateRef) {
    let (workspace, repo) = {
        let state = state.borrow();
        (state.workspace.upgrade(), state.repo.clone())
    };
    if let Some(workspace) = workspace {
        workspace.close_review_tabs_for_other_repo(repo.as_deref());
    }
}

pub(super) fn saved_file_in_repo(state: &SourceControlState, file: &gio::File) -> bool {
    live::saved_file_in_repo(state, file)
}
