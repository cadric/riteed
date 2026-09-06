use std::cell::Cell;
use std::rc::Rc;

use crate::git_process::GitProcessError;
use crate::git_status::{GitAttrState, GitPath, GitStatusSnapshot};
use crate::source_control::slots::RefreshTicket;
use crate::source_control::{
    SourceStateRef, actions, git_attrs_unavailable_text, git_error_is_cancelled, git_error_text,
    history, live, operation_bridge, set_commit_controls_enabled, set_status_label,
    set_title_subtitle,
};
use gettextrs::{gettext, pgettext};
use gtk4::prelude::*;

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
    let (process, repo) = {
        let state = state.borrow();
        (state.process.clone(), state.repo.clone())
    };
    let (Some(process), Some(repo)) = (process, repo) else {
        return;
    };
    let Some(ticket) = operation_bridge::begin_refresh(state, &repo) else {
        return;
    };
    if !operation_bridge::is_refresh_current(state, &ticket) {
        return;
    }
    if lock_gate_applies(origin) && live::index_lock_exists(state) {
        if matches!(origin, RefreshOrigin::Manual | RefreshOrigin::Initial) {
            set_status_label(
                state,
                &ellipsis_label(gettext("Waiting for another Git operation to finish")),
            );
            if !operation_bridge::is_refresh_current(state, &ticket) {
                return;
            }
        }
        let _finished = operation_bridge::finish_refresh(state, &ticket);
        live::schedule(state);
        return;
    }
    if matches!(origin, RefreshOrigin::Manual | RefreshOrigin::Initial) {
        state.borrow_mut().status_stale = true;
        set_commit_controls_enabled(state, false);
        if !operation_bridge::is_refresh_current(state, &ticket) {
            return;
        }
        set_status_label(state, &ellipsis_label(gettext("Refreshing Git status")));
        if !operation_bridge::is_refresh_current(state, &ticket) {
            return;
        }
        actions::fire_state_change_handler(state);
        if !operation_bridge::is_refresh_current(state, &ticket) {
            return;
        }
    }
    let weak = Rc::downgrade(state);
    let cancellable = ticket.cancellable().clone();
    process.check_repo_capabilities(
        &cancellable,
        Rc::new(move |capabilities| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if !operation_bridge::is_refresh_current(&state, &ticket) {
                return;
            }
            let capabilities = match capabilities {
                Ok(capabilities) => capabilities,
                Err(error) if git_error_is_cancelled(&error) => {
                    let _finished = operation_bridge::finish_refresh(&state, &ticket);
                    return;
                }
                Err(error) => {
                    if matches!(error, GitProcessError::TimedOut) {
                        finish_refresh_error(&state, &ticket, &git_error_text(&error));
                    } else {
                        finish_refresh_error(
                            &state,
                            &ticket,
                            &gettext("Unable to read Git repository capabilities."),
                        );
                    }
                    return;
                }
            };
            state.borrow_mut().capabilities = capabilities;
            if !capabilities.object_format_supported || !capabilities.eol_supported {
                finish_unsupported_repo(&state, &ticket);
                return;
            }
            refresh_status_entries(&state, ticket.clone());
        }),
    );
}

pub(super) fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

pub(super) fn finish_error(state: &SourceStateRef, message: &str) {
    let generation = {
        let mut state = state.borrow_mut();
        state.status_stale = true;
        state.review_generation
    };
    set_status_label(state, message);
    if state.borrow().review_generation != generation {
        return;
    }
    set_commit_controls_enabled(state, false);
    if state.borrow().review_generation != generation {
        return;
    }
    emit_project_statuses(state);
    if state.borrow().review_generation != generation {
        return;
    }
    rebuild_views(state);
    if state.borrow().review_generation != generation {
        return;
    }
    actions::fire_state_change_handler(state);
}

pub(super) fn rebuild_views(state: &SourceStateRef) {
    let (views, entries) = {
        let state = state.borrow();
        (Rc::clone(&state.views), state.snapshot.entries.clone())
    };
    views.rebuild(&entries);
    super::active_row::apply_active_row(state);
}

pub(super) fn emit_project_statuses(state: &SourceStateRef) {
    let (handler, statuses) = {
        let state = state.borrow();
        let Some(handler) = state.status_handler.as_ref().map(Rc::clone) else {
            return;
        };
        let statuses = state.repo.as_ref().map_or_else(Vec::new, |repo| {
            state
                .snapshot
                .entries
                .iter()
                .filter_map(|entry| {
                    let path = entry.path.as_utf8()?;
                    let uri = gtk4::gio::File::for_path(repo.join(path)).uri().to_string();
                    Some((uri, String::from(entry.status.badge())))
                })
                .collect()
        });
        (handler, statuses)
    };
    handler(statuses);
}

fn refresh_status_entries(state: &SourceStateRef, ticket: RefreshTicket) {
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let cancellable = ticket.cancellable().clone();
    let weak = Rc::downgrade(state);
    process.status(
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if !operation_bridge::is_refresh_current(&state, &ticket) {
                return;
            }
            let snapshot = match result {
                Ok(snapshot) => snapshot,
                Err(error) if git_error_is_cancelled(&error) => {
                    let _finished = operation_bridge::finish_refresh(&state, &ticket);
                    return;
                }
                Err(error) => {
                    if matches!(error, GitProcessError::TimedOut) {
                        finish_refresh_error(&state, &ticket, &git_error_text(&error));
                    } else {
                        finish_refresh_error(
                            &state,
                            &ticket,
                            &gettext("Unable to refresh Git status."),
                        );
                    }
                    return;
                }
            };
            let paths = snapshot.changed_paths();
            if snapshot.too_large || paths.is_empty() {
                apply_status(&state, snapshot, GitAttrState::default(), &ticket);
                return;
            }
            refresh_attrs(&state, snapshot, &paths, ticket.clone());
        }),
    );
}

fn refresh_attrs(
    state: &SourceStateRef,
    snapshot: GitStatusSnapshot,
    paths: &[GitPath],
    ticket: RefreshTicket,
) {
    let Some(process) = state.borrow().process.clone() else {
        return;
    };
    let cancellable = ticket.cancellable().clone();
    let weak = Rc::downgrade(state);
    process.check_attrs(
        paths,
        &cancellable,
        Rc::new(move |result| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            if !operation_bridge::is_refresh_current(&state, &ticket) {
                return;
            }
            let attrs = match result {
                Ok(attrs) => GitAttrState::Known(attrs),
                Err(error) if git_error_is_cancelled(&error) => {
                    let _finished = operation_bridge::finish_refresh(&state, &ticket);
                    return;
                }
                Err(_error) => GitAttrState::Unavailable,
            };
            apply_status(&state, snapshot.clone(), attrs, &ticket);
        }),
    );
}

fn apply_status(
    state: &SourceStateRef,
    snapshot: GitStatusSnapshot,
    attrs: GitAttrState,
    ticket: &RefreshTicket,
) {
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    let dirty_uris = actions::dirty_open_uris(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    let (was_stale, previous_snapshot, previous_attrs) = {
        let mut state = state.borrow_mut();
        let was_stale = state.status_stale;
        let previous_snapshot = state.snapshot.clone();
        let previous_attrs = state.attrs.clone();
        state.snapshot = snapshot;
        state.attrs = attrs;
        state.status_stale = false;
        state.review_generation = state.review_generation.wrapping_add(1);
        (was_stale, previous_snapshot, previous_attrs)
    };
    let Some(_snapshot_id) = operation_bridge::publish_snapshot(state, ticket) else {
        return;
    };
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    let (head_oid, snapshot, changed, can_commit, title, status_text) = {
        let mut state = state.borrow_mut();
        let can_commit = actions::apply_entry_actions(&mut state, &dirty_uris);
        let changed = state.snapshot != previous_snapshot || state.attrs != previous_attrs;
        let title = branch_title(&state.snapshot);
        let status_text = status_label_text(&state.snapshot, &state.attrs);
        (
            state.snapshot.head_oid.clone(),
            state.snapshot.clone(),
            changed || was_stale,
            can_commit,
            title,
            status_text,
        )
    };
    set_title_subtitle(state, &title);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    set_commit_controls_enabled(state, can_commit);
    if changed {
        if !operation_bridge::is_refresh_current(state, ticket) {
            return;
        }
        set_status_label(state, &status_text);
        if !operation_bridge::is_refresh_current(state, ticket) {
            return;
        }
        emit_project_statuses(state);
        if !operation_bridge::is_refresh_current(state, ticket) {
            return;
        }
        rebuild_views(state);
    }
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    actions::fire_state_change_handler(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    super::review::mark_open_reviews(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    let completion = refresh_completion(state, ticket);
    live::sync_branch_monitor(state, &snapshot, ticket, Rc::clone(&completion));
    history::refresh(state, head_oid.as_deref(), ticket, completion);
}

fn refresh_completion(state: &SourceStateRef, ticket: &RefreshTicket) -> Rc<dyn Fn()> {
    let remaining = Rc::new(Cell::new(2_u8));
    let weak = Rc::downgrade(state);
    let ticket = ticket.clone();
    Rc::new(move || {
        let current = remaining.get();
        if current == 0 {
            return;
        }
        remaining.set(current - 1);
        if current != 1 {
            return;
        }
        if let Some(state) = weak.upgrade() {
            let _finished = operation_bridge::finish_refresh(&state, &ticket);
        }
    })
}

fn finish_refresh_error(state: &SourceStateRef, ticket: &RefreshTicket, message: &str) {
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    state.borrow_mut().status_stale = true;
    set_status_label(state, message);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    set_commit_controls_enabled(state, false);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    emit_project_statuses(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    rebuild_views(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    actions::fire_state_change_handler(state);
    let _finished = operation_bridge::finish_refresh(state, ticket);
}

fn finish_unsupported_repo(state: &SourceStateRef, ticket: &RefreshTicket) {
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    state.borrow_mut().status_stale = false;
    set_commit_controls_enabled(state, false);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    set_status_label(
        state,
        &gettext("This Git repository uses unsupported object or EOL settings."),
    );
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    emit_project_statuses(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    rebuild_views(state);
    if !operation_bridge::is_refresh_current(state, ticket) {
        return;
    }
    actions::fire_state_change_handler(state);
    let _finished = operation_bridge::finish_refresh(state, ticket);
}

fn branch_title(snapshot: &GitStatusSnapshot) -> String {
    snapshot
        .branch
        .clone()
        .unwrap_or_else(|| pgettext("git branch", "Detached"))
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
