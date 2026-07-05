use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::gio;
use gtk4::prelude::*;

use crate::git_process::GitProcessError;
use crate::git_status::{GitAttrState, GitPath, GitStatusSnapshot};
use crate::source_control::{
    SourceControlState, SourceStateRef, actions, git_attrs_unavailable_text,
    git_error_is_cancelled, git_error_text, history, live, set_commit_controls_enabled,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshOrigin {
    Initial,
    Manual,
    Automatic,
    LockWaitExpired,
}

const fn lock_gate_applies(origin: RefreshOrigin) -> bool {
    !matches!(origin, RefreshOrigin::LockWaitExpired)
}

pub(super) fn refresh_status(state: &SourceStateRef) {
    refresh_status_with_origin(state, RefreshOrigin::Automatic);
}

pub(super) fn refresh_status_with_origin(state: &SourceStateRef, origin: RefreshOrigin) {
    super::cancel_refresh(state);
    if lock_gate_applies(origin) && live::index_lock_exists(state) {
        if matches!(origin, RefreshOrigin::Manual | RefreshOrigin::Initial) {
            state
                .borrow()
                .status_label
                .set_label(&ellipsis_label(gettext(
                    "Waiting for another Git operation to finish",
                )));
        }
        live::schedule(state);
        return;
    }
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let cancellable = gio::Cancellable::new();
    {
        let mut state = state.borrow_mut();
        state.cancellable = Some(cancellable.clone());
        if matches!(origin, RefreshOrigin::Manual | RefreshOrigin::Initial) {
            state.status_stale = true;
            state
                .status_label
                .set_label(&ellipsis_label(gettext("Refreshing Git status")));
        }
    }
    let weak = Rc::downgrade(state);
    let cancellable_for_callback = cancellable.clone();
    process.check_repo_capabilities(
        &cancellable,
        Rc::new(move |capabilities| {
            if should_ignore_cancelled(&cancellable_for_callback, &capabilities) {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            let capabilities = match capabilities {
                Ok(capabilities) => capabilities,
                Err(error) if git_error_is_cancelled(&error) => return,
                Err(error) => {
                    if matches!(error, GitProcessError::TimedOut) {
                        finish_error(&state, &git_error_text(&error));
                    } else {
                        finish_error(
                            &state,
                            &gettext("Unable to read Git repository capabilities."),
                        );
                    }
                    return;
                }
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

pub(super) fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

pub(super) fn finish_error(state: &SourceStateRef, message: &str) {
    {
        let mut state = state.borrow_mut();
        state.status_stale = true;
        state.status_label.set_label(message);
        set_commit_controls_enabled(&state, false);
        emit_project_statuses(&state);
        rebuild_views(&state);
    }
    actions::fire_state_change_handler(state);
}

pub(super) fn rebuild_views(state: &SourceControlState) {
    state.views.rebuild(&state.snapshot.entries);
    super::active_row::apply_active_row(state);
}

pub(super) fn emit_project_statuses(state: &SourceControlState) {
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

fn refresh_status_entries(state: &SourceStateRef) {
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let Some(cancellable) = state.borrow().cancellable.clone() else {
        return;
    };
    let weak = Rc::downgrade(state);
    let cancellable_for_callback = cancellable.clone();
    process.status(
        &cancellable,
        Rc::new(move |result| {
            if should_ignore_cancelled(&cancellable_for_callback, &result) {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            let snapshot = match result {
                Ok(snapshot) => snapshot,
                Err(error) if git_error_is_cancelled(&error) => return,
                Err(error) => {
                    if matches!(error, GitProcessError::TimedOut) {
                        finish_error(&state, &git_error_text(&error));
                    } else {
                        finish_error(&state, &gettext("Unable to refresh Git status."));
                    }
                    return;
                }
            };
            let paths = snapshot.changed_paths();
            if snapshot.too_large || paths.is_empty() {
                apply_status(&state, snapshot, GitAttrState::default());
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
    let cancellable_for_callback = cancellable.clone();
    process.check_attrs(
        paths,
        &cancellable,
        Rc::new(move |result| {
            if should_ignore_cancelled(&cancellable_for_callback, &result) {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            let attrs = match result {
                Ok(attrs) => GitAttrState::Known(attrs),
                Err(error) if git_error_is_cancelled(&error) => return,
                Err(_error) => GitAttrState::Unavailable,
            };
            apply_status(&state, snapshot.clone(), attrs);
        }),
    );
}

fn should_ignore_cancelled<T>(
    cancellable: &gio::Cancellable,
    result: &Result<T, GitProcessError>,
) -> bool {
    cancellable.is_cancelled() && !result_is_timeout(result)
}

fn result_is_timeout<T>(result: &Result<T, GitProcessError>) -> bool {
    match result {
        Err(error) => error == &GitProcessError::TimedOut,
        Ok(_) => false,
    }
}

fn apply_status(state: &SourceStateRef, snapshot: GitStatusSnapshot, attrs: GitAttrState) {
    let (head_oid, snapshot) = {
        let mut state = state.borrow_mut();
        let was_stale = state.status_stale;
        let previous_snapshot = state.snapshot.clone();
        let previous_attrs = state.attrs.clone();
        state.snapshot = snapshot;
        state.attrs = attrs;
        state.status_stale = false;
        state.review_generation = state.review_generation.wrapping_add(1);
        update_title(&state);
        actions::apply_entry_actions(&mut state);
        let changed = state.snapshot != previous_snapshot || state.attrs != previous_attrs;
        if changed || was_stale {
            update_status_label(&state);
            emit_project_statuses(&state);
            rebuild_views(&state);
        }
        (state.snapshot.head_oid.clone(), state.snapshot.clone())
    };
    actions::fire_state_change_handler(state);
    super::review::mark_open_reviews(&state.borrow());
    live::sync_branch_monitor(state, &snapshot);
    history::refresh(state, head_oid.as_deref());
}

fn finish_unsupported_repo(state: &SourceStateRef) {
    {
        let mut state = state.borrow_mut();
        state.status_stale = false;
        set_commit_controls_enabled(&state, false);
        state.status_label.set_label(&gettext(
            "This Git repository uses unsupported object or EOL settings.",
        ));
        emit_project_statuses(&state);
        rebuild_views(&state);
    }
    actions::fire_state_change_handler(state);
}

fn update_title(state: &SourceControlState) {
    let branch = state
        .snapshot
        .branch
        .clone()
        .unwrap_or_else(|| pgettext("git branch", "Detached"));
    state.title.set_subtitle(&branch);
}

fn update_status_label(state: &SourceControlState) {
    state
        .status_label
        .set_label(&status_label_text(&state.snapshot, &state.attrs));
}

fn status_label_text(snapshot: &GitStatusSnapshot, attrs: &GitAttrState) -> String {
    if snapshot.too_large {
        gettext("Too many Git changes to display.")
    } else if attrs.is_unavailable() {
        git_attrs_unavailable_text()
    } else if snapshot.entries.is_empty() {
        gettext("No changes.")
    } else {
        gettext("Changed files")
    }
}

#[cfg(test)]
mod tests {
    use super::status_label_text;
    use crate::git_status::{GitAttrState, GitStatusSnapshot};

    #[test]
    fn status_label_prefers_too_large_degraded_state() {
        let snapshot = GitStatusSnapshot {
            too_large: true,
            ..GitStatusSnapshot::default()
        };

        assert_eq!(
            status_label_text(&snapshot, &GitAttrState::Unavailable),
            "Too many Git changes to display."
        );
    }

    #[test]
    fn status_label_covers_clean_and_attr_unavailable_states() {
        assert_eq!(
            status_label_text(&GitStatusSnapshot::default(), &GitAttrState::default()),
            "No changes."
        );
        assert_eq!(
            status_label_text(&GitStatusSnapshot::default(), &GitAttrState::Unavailable),
            "Unable to read Git attributes. Git actions are disabled."
        );
    }

    #[test]
    fn lock_gate_skips_expired_lock_waits() {
        use super::{RefreshOrigin, lock_gate_applies};
        assert!(lock_gate_applies(RefreshOrigin::Manual));
        assert!(lock_gate_applies(RefreshOrigin::Automatic));
        assert!(lock_gate_applies(RefreshOrigin::Initial));
        assert!(!lock_gate_applies(RefreshOrigin::LockWaitExpired));
    }
}
