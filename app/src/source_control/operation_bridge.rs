use std::path::Path;
use std::rc::Rc;

use gtk4::{gio, prelude::*};

use super::slots::{DiffTicket, MutationTicket, RefreshTicket, SnapshotId, cancel_queued};
use super::{EditorTab, MinimapRequest, SourceControlState, SourceStateRef};

pub(super) fn begin_refresh(state: &SourceStateRef, repo: &Path) -> Option<RefreshTicket> {
    transition(state, |operations| operations.begin_refresh(repo))
}

pub(super) fn is_refresh_current(state: &SourceStateRef, ticket: &RefreshTicket) -> bool {
    state.borrow().operations.is_refresh_current(ticket)
}

pub(super) fn finish_refresh(state: &SourceStateRef, ticket: &RefreshTicket) -> bool {
    transition(state, |operations| operations.finish_refresh(ticket))
}

pub(super) fn publish_snapshot(
    state: &SourceStateRef,
    ticket: &RefreshTicket,
) -> Option<SnapshotId> {
    let (snapshot, cancellations) = {
        let mut state = state.borrow_mut();
        let snapshot = state.operations.publish_snapshot(ticket);
        if let Some(snapshot) = snapshot.as_ref() {
            state.snapshot_id = Some(snapshot.clone());
        }
        let cancellations = state.operations.take_cancellations();
        (snapshot, cancellations)
    };
    cancel_queued(cancellations);
    snapshot
}

pub(super) fn is_snapshot_current(state: &SourceStateRef, snapshot: &SnapshotId) -> bool {
    state.borrow().operations.is_snapshot_current(snapshot)
}

pub(super) fn begin_diff(state: &SourceStateRef) -> Option<DiffTicket> {
    transition(state, super::slots::OperationSlots::begin_diff)
}

pub(super) fn is_diff_current(state: &SourceStateRef, ticket: &DiffTicket) -> bool {
    state.borrow().operations.is_diff_current(ticket)
}

pub(super) fn finish_diff(state: &SourceStateRef, ticket: &DiffTicket) -> bool {
    transition(state, |operations| operations.finish_diff(ticket))
}

pub(super) fn try_begin_mutation(state: &SourceStateRef, repo: &Path) -> Option<MutationTicket> {
    let (ticket, cancellations) = {
        let mut state = state.borrow_mut();
        let ticket = state.operations.try_begin_mutation(repo);
        if ticket.is_some() {
            state.snapshot_id = None;
            state.status_stale = true;
        }
        let cancellations = state.operations.take_cancellations();
        (ticket, cancellations)
    };
    cancel_queued(cancellations);
    ticket
}

pub(super) fn is_mutation_current(state: &SourceStateRef, ticket: &MutationTicket) -> bool {
    state.borrow().operations.is_mutation_current(ticket)
}

pub(super) fn finish_mutation(state: &SourceStateRef, ticket: &MutationTicket) -> bool {
    transition(state, |operations| operations.finish_mutation(ticket))
}

pub(super) fn mutation_root_is_current(state: &SourceStateRef, ticket: &MutationTicket) -> bool {
    state.borrow().operations.mutation_root_is_current(ticket)
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

pub(super) fn cancel_review_requests(state: &SourceStateRef) {
    let cancellations = {
        let mut state = state.borrow_mut();
        take_review_cancellations(&mut state)
    };
    cancel_queued(cancellations);
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
    let cancellations = {
        let mut state = state.borrow_mut();
        let mut cancellations = Vec::new();
        let mut retained = Vec::new();
        for request in state.minimap_cancellables.drain(..) {
            let same_tab = request
                .tab
                .upgrade()
                .is_some_and(|active| Rc::ptr_eq(&active, tab));
            let same_source = source.is_none_or(|source| request.source == source);
            if same_tab && same_source {
                cancellations.push(request.cancellable);
            } else {
                retained.push(request);
            }
        }
        state.minimap_cancellables = retained;
        cancellations
    };
    cancel_queued(cancellations);
}

pub(super) fn cancel_minimap_requests(state: &SourceStateRef) {
    let cancellations = {
        let mut state = state.borrow_mut();
        take_minimap_cancellations(&mut state)
    };
    cancel_queued(cancellations);
}

pub(super) fn take_review_cancellations(state: &mut SourceControlState) -> Vec<gio::Cancellable> {
    state.review_cancellables.drain(..).collect()
}

pub(super) fn take_minimap_cancellations(state: &mut SourceControlState) -> Vec<gio::Cancellable> {
    state
        .minimap_cancellables
        .drain(..)
        .map(|request| request.cancellable)
        .collect()
}

/// Checks the real repository lock even when the live watcher is absent. Portal-backed paths
/// deliberately skip this synchronous advisory probe; Git remains the authoritative lock owner.
pub(super) fn native_index_lock_exists(state: &SourceStateRef) -> bool {
    let context = {
        let state = state.borrow();
        state
            .process
            .as_ref()
            .map(|process| process.context().clone())
    };
    let Some(context) = context else {
        return false;
    };
    if [
        context.work_tree.as_path(),
        context.git_dir.as_path(),
        context.git_common_dir.as_path(),
    ]
    .into_iter()
    .any(path_requires_polling)
    {
        return false;
    }
    context.index_lock_path.exists()
}

fn transition<T>(
    state: &SourceStateRef,
    transition: impl FnOnce(&mut super::slots::OperationSlots) -> T,
) -> T {
    let (result, cancellations) = {
        let mut state = state.borrow_mut();
        let result = transition(&mut state.operations);
        let cancellations = state.operations.take_cancellations();
        (result, cancellations)
    };
    // Cancellation handlers can re-enter the controller. A caller that receives a ticket must
    // check currentness after this function returns and again in every async continuation.
    cancel_queued(cancellations);
    result
}

fn path_requires_polling(path: &Path) -> bool {
    let file = gio::File::for_path(path);
    !file.is_native() || document_portal_path(path)
}

fn document_portal_path(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.starts_with("/run/flatpak/doc/")
        || (path.starts_with("/run/user/") && path.contains("/doc/"))
}
