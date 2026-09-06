use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::gio;
use gtk4::gio::prelude::*;

#[derive(Clone)]
struct Identity(Rc<()>);

impl Identity {
    fn new() -> Self {
        Self(Rc::new(()))
    }

    fn is(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Clone)]
struct RootEpoch(Identity);

impl RootEpoch {
    fn new() -> Self {
        Self(Identity::new())
    }

    fn is(&self, other: &Self) -> bool {
        self.0.is(&other.0)
    }
}

#[derive(Clone)]
pub(crate) struct SnapshotId(Identity);

impl SnapshotId {
    fn new() -> Self {
        Self(Identity::new())
    }

    fn is(&self, other: &Self) -> bool {
        self.0.is(&other.0)
    }
}

#[derive(Clone)]
pub(super) struct RefreshTicket {
    identity: Identity,
    root_epoch: RootEpoch,
    cancellable: gio::Cancellable,
}

impl RefreshTicket {
    fn new(root_epoch: &RootEpoch) -> Self {
        Self {
            identity: Identity::new(),
            root_epoch: root_epoch.clone(),
            cancellable: gio::Cancellable::new(),
        }
    }

    pub(super) fn cancellable(&self) -> &gio::Cancellable {
        &self.cancellable
    }

    fn is(&self, other: &Self) -> bool {
        self.identity.is(&other.identity)
    }
}

#[derive(Clone)]
pub(super) struct DiffTicket {
    identity: Identity,
    root_epoch: RootEpoch,
    snapshot: SnapshotId,
    cancellable: gio::Cancellable,
}

impl DiffTicket {
    fn new(root_epoch: &RootEpoch, snapshot: &SnapshotId) -> Self {
        Self {
            identity: Identity::new(),
            root_epoch: root_epoch.clone(),
            snapshot: snapshot.clone(),
            cancellable: gio::Cancellable::new(),
        }
    }

    pub(super) fn cancellable(&self) -> &gio::Cancellable {
        &self.cancellable
    }

    fn is(&self, other: &Self) -> bool {
        self.identity.is(&other.identity)
    }
}

#[derive(Clone)]
pub(super) struct MutationTicket {
    identity: Identity,
    root_epoch: RootEpoch,
    repo: PathBuf,
    cancellable: gio::Cancellable,
}

impl MutationTicket {
    fn new(root_epoch: &RootEpoch, repo: &Path) -> Self {
        Self {
            identity: Identity::new(),
            root_epoch: root_epoch.clone(),
            repo: repo.to_path_buf(),
            cancellable: gio::Cancellable::new(),
        }
    }

    pub(super) fn cancellable(&self) -> &gio::Cancellable {
        &self.cancellable
    }

    pub(super) fn repo(&self) -> &Path {
        &self.repo
    }

    fn is(&self, other: &Self) -> bool {
        self.identity.is(&other.identity)
    }
}

pub(crate) struct OperationSlots {
    root_epoch: RootEpoch,
    snapshot: Option<SnapshotId>,
    refresh: Option<RefreshTicket>,
    diff: Option<DiffTicket>,
    mutation: Option<MutationTicket>,
    cancellations: Vec<gio::Cancellable>,
}

impl OperationSlots {
    pub(super) fn new() -> Self {
        Self {
            root_epoch: RootEpoch::new(),
            snapshot: None,
            refresh: None,
            diff: None,
            mutation: None,
            cancellations: Vec::new(),
        }
    }

    /// Starts repository detection in the current root epoch. Detection deliberately bypasses
    /// same-repository mutation coalescing because it must resolve the root before status can run.
    pub(super) fn begin_detection(&mut self) -> RefreshTicket {
        self.retire_refresh();
        self.start_refresh()
    }

    /// Starts a refresh unless a mutation for the same repository still owns its slot.
    /// Repeated blocked requests coalesce because they create no ticket or child operation.
    pub(super) fn begin_refresh(&mut self, repo: &Path) -> Option<RefreshTicket> {
        if self
            .mutation
            .as_ref()
            .is_some_and(|mutation| mutation.repo == repo)
        {
            return None;
        }
        self.retire_refresh();
        Some(self.start_refresh())
    }

    fn start_refresh(&mut self) -> RefreshTicket {
        let ticket = RefreshTicket::new(&self.root_epoch);
        self.refresh = Some(ticket.clone());
        ticket
    }

    pub(super) fn is_refresh_current(&self, ticket: &RefreshTicket) -> bool {
        self.root_epoch.is(&ticket.root_epoch)
            && self
                .refresh
                .as_ref()
                .is_some_and(|current| current.is(ticket))
    }

    pub(super) fn finish_refresh(&mut self, ticket: &RefreshTicket) -> bool {
        if !self
            .refresh
            .as_ref()
            .is_some_and(|current| current.is(ticket))
        {
            return false;
        }
        self.refresh = None;
        true
    }

    /// Retires only the refresh owner and queues its cancellation for an external flush.
    pub(super) fn cancel_refresh(&mut self) {
        self.retire_refresh();
    }

    /// Gives a completed, current refresh a new snapshot identity.
    /// Any diff for the previous snapshot becomes stale and is queued for cancellation.
    pub(super) fn publish_snapshot(&mut self, ticket: &RefreshTicket) -> Option<SnapshotId> {
        if !self.is_refresh_current(ticket) {
            return None;
        }
        self.retire_diff();
        let snapshot = SnapshotId::new();
        self.snapshot = Some(snapshot.clone());
        Some(snapshot)
    }

    pub(super) fn is_snapshot_current(&self, snapshot: &SnapshotId) -> bool {
        self.snapshot
            .as_ref()
            .is_some_and(|current| current.is(snapshot))
    }

    pub(super) fn begin_diff(&mut self) -> Option<DiffTicket> {
        let snapshot = self.snapshot.clone()?;
        self.retire_diff();
        let ticket = DiffTicket::new(&self.root_epoch, &snapshot);
        self.diff = Some(ticket.clone());
        Some(ticket)
    }

    pub(super) fn is_diff_current(&self, ticket: &DiffTicket) -> bool {
        self.root_epoch.is(&ticket.root_epoch)
            && self.is_snapshot_current(&ticket.snapshot)
            && self.diff.as_ref().is_some_and(|current| current.is(ticket))
    }

    pub(super) fn finish_diff(&mut self, ticket: &DiffTicket) -> bool {
        if !self.diff.as_ref().is_some_and(|current| current.is(ticket)) {
            return false;
        }
        self.diff = None;
        true
    }

    /// Acquires the sole writer slot and invalidates all status derived before it.
    /// A cancelled mutation remains the owner until its matching terminal completion.
    pub(super) fn try_begin_mutation(&mut self, repo: &Path) -> Option<MutationTicket> {
        if self.mutation.is_some() {
            return None;
        }
        self.retire_refresh();
        self.invalidate_snapshot();
        let ticket = MutationTicket::new(&self.root_epoch, repo);
        self.mutation = Some(ticket.clone());
        Some(ticket)
    }

    pub(super) fn is_mutation_current(&self, ticket: &MutationTicket) -> bool {
        self.mutation_root_is_current(ticket)
            && self
                .mutation
                .as_ref()
                .is_some_and(|current| current.is(ticket))
    }

    pub(super) fn mutation_active(&self) -> bool {
        self.mutation.is_some()
    }

    /// Checks whether a mutation result still belongs to the displayed root. This remains
    /// usable after [`Self::finish_mutation`] clears the matching owner first.
    pub(super) fn mutation_root_is_current(&self, ticket: &MutationTicket) -> bool {
        self.root_epoch.is(&ticket.root_epoch)
    }

    /// Releases the writer only when `ticket` is the exact current owner.
    /// Root invalidation deliberately does not affect this identity check.
    pub(super) fn finish_mutation(&mut self, ticket: &MutationTicket) -> bool {
        if !self
            .mutation
            .as_ref()
            .is_some_and(|current| current.is(ticket))
        {
            return false;
        }
        self.mutation = None;
        true
    }

    /// Rotates the display epoch, invalidates snapshots, retires readers, and requests
    /// cancellation of a mutation without releasing its owner slot.
    pub(super) fn invalidate_root(&mut self) {
        self.root_epoch = RootEpoch::new();
        self.retire_refresh();
        self.invalidate_snapshot();
        if let Some(cancellable) = self
            .mutation
            .as_ref()
            .map(|mutation| mutation.cancellable.clone())
        {
            self.queue_cancellation(cancellable);
        }
    }

    /// Takes cancellation requests without signalling them. The controller must release
    /// its `SourceControlState` borrow before passing this batch to [`cancel_queued`].
    pub(super) fn take_cancellations(&mut self) -> Vec<gio::Cancellable> {
        std::mem::take(&mut self.cancellations)
    }

    /// Invalidates every owner and returns all cancellation requests for teardown.
    /// The returned batch must be signalled after any outer state borrow is released.
    pub(super) fn drain_for_teardown(&mut self) -> Vec<gio::Cancellable> {
        self.root_epoch = RootEpoch::new();
        self.snapshot = None;
        if let Some(ticket) = self.refresh.take() {
            self.queue_cancellation(ticket.cancellable);
        }
        if let Some(ticket) = self.diff.take() {
            self.queue_cancellation(ticket.cancellable);
        }
        if let Some(ticket) = self.mutation.take() {
            self.queue_cancellation(ticket.cancellable);
        }
        self.take_cancellations()
    }

    fn invalidate_snapshot(&mut self) {
        self.snapshot = None;
        self.retire_diff();
    }

    fn retire_refresh(&mut self) {
        if let Some(ticket) = self.refresh.take() {
            self.queue_cancellation(ticket.cancellable);
        }
    }

    fn retire_diff(&mut self) {
        if let Some(ticket) = self.diff.take() {
            self.queue_cancellation(ticket.cancellable);
        }
    }

    fn queue_cancellation(&mut self, cancellable: gio::Cancellable) {
        if !self
            .cancellations
            .iter()
            .any(|queued| queued == &cancellable)
        {
            self.cancellations.push(cancellable);
        }
    }
}

/// Signals a batch returned by [`OperationSlots::take_cancellations`] or teardown.
/// Call this only after releasing the `SourceControlState` borrow that produced it.
pub(super) fn cancel_queued(cancellations: Vec<gio::Cancellable>) {
    for cancellable in cancellations {
        cancellable.cancel();
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
