use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::gio;
use gtk4::prelude::*;

use crate::git_status::{GitAttrs, GitPath, GitStatusSnapshot};
use crate::source_control::{SourceControlState, SourceStateRef, actions, history, live};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RefreshOrigin {
    Initial,
    Manual,
    Automatic,
}

pub(super) fn refresh_status(state: &SourceStateRef) {
    refresh_status_with_origin(state, RefreshOrigin::Automatic);
}

pub(super) fn refresh_status_with_origin(state: &SourceStateRef, origin: RefreshOrigin) {
    super::cancel_refresh(state);
    if live::index_lock_exists(state) {
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
        if origin != RefreshOrigin::Automatic {
            state.status_stale = true;
            state
                .status_label
                .set_label(&gettext("Refreshing Git status..."));
        }
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

pub(super) fn finish_error(state: &SourceStateRef, message: &str) {
    let mut state = state.borrow_mut();
    state.status_stale = true;
    state.status_label.set_label(message);
    state.commit_button.set_sensitive(false);
    emit_project_statuses(&state);
    rebuild_views(&state);
}

pub(super) fn rebuild_views(state: &SourceControlState) {
    state.views.rebuild(&state.snapshot.entries);
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
                apply_status(&state, snapshot, GitAttrs::default());
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
            apply_status(&state, snapshot.clone(), result.unwrap_or_default());
        }),
    );
}

fn apply_status(state: &SourceStateRef, snapshot: GitStatusSnapshot, attrs: GitAttrs) {
    let head_oid = {
        let mut state = state.borrow_mut();
        let was_stale = state.status_stale;
        let previous_snapshot = state.snapshot.clone();
        let previous_attrs = state.attrs.clone();
        state.snapshot = snapshot;
        state.attrs = attrs;
        state.status_stale = false;
        update_title(&state);
        actions::apply_entry_actions(&mut state);
        let changed = state.snapshot != previous_snapshot || state.attrs != previous_attrs;
        if changed || was_stale {
            update_status_label(&state);
            emit_project_statuses(&state);
            rebuild_views(&state);
        }
        state.snapshot.head_oid.clone()
    };
    history::refresh(state, head_oid.as_deref());
}

fn finish_unsupported_repo(state: &SourceStateRef) {
    let mut state = state.borrow_mut();
    state.status_stale = false;
    state.commit_button.set_sensitive(false);
    state.status_label.set_label(&gettext(
        "This Git repository uses unsupported object or EOL settings.",
    ));
    emit_project_statuses(&state);
    rebuild_views(&state);
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
    if state.snapshot.entries.is_empty() {
        state.status_label.set_label(&gettext("No changes."));
    } else {
        state.status_label.set_label(&gettext("Changed files"));
    }
}
