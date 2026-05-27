use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, prelude::*};

use crate::git_process::{GitCallback, GitProcess, GitProcessError, GitRepoContext};
use crate::git_status::{GitAttrState, GitStatusSnapshot};

use super::refresh::{
    RefreshOrigin, ellipsis_label, emit_project_statuses, rebuild_views, refresh_status_with_origin,
};
use super::{
    SourceControlState, SourceStateRef, actions, cancel_minimap_requests_locked, cancel_refresh,
    cancel_review_requests_locked, git_error_is_cancelled, git_error_text, live,
    set_commit_controls_enabled,
};

// PARSER-BOUNDARY: id=git_status_ui
pub(super) fn set_project_root(state: &SourceStateRef, folder: Option<gio::File>) {
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
        state.review_generation = state.review_generation.wrapping_add(1);
        cancel_review_requests_locked(&mut state);
        cancel_minimap_requests_locked(&mut state);
        state
            .status_label
            .set_label(&ellipsis_label(gettext("Refreshing Git status")));
        set_commit_controls_enabled(&state, false);
        emit_project_statuses(&state);
        state.history.clear();
        rebuild_views(&state);
    }
    actions::fire_state_change_handler(state);
    close_review_tabs_for_current_repo(state);
    let weak = Rc::downgrade(state);
    let cancellable_for_callback = cancellable.clone();
    let callback: GitCallback<GitRepoContext> = Rc::new(move |result| {
        let timed_out = matches!(&result, Err(error) if error == &GitProcessError::TimedOut);
        if cancellable_for_callback.is_cancelled() && !timed_out {
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
            Err(error) if matches!(error, GitProcessError::TimedOut) => {
                reset_project_state(&state, &git_error_text(&error), true);
            }
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
        state.review_generation = state.review_generation.wrapping_add(1);
        cancel_review_requests_locked(&mut state);
        cancel_minimap_requests_locked(&mut state);
        state.status_label.set_label(label);
        set_commit_controls_enabled(&state, false);
        emit_project_statuses(&state);
        state.history.clear();
        rebuild_views(&state);
    }
    actions::fire_state_change_handler(state);
    close_review_tabs_for_current_repo(state);
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
