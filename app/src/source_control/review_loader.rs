use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gettextrs::{gettext, ngettext};
use gtk4::{gio, prelude::*};

use crate::editor_tab::{EditorTab, ReviewFileId, ReviewFileInput, ReviewKind, ReviewTabSpec};
use crate::error::AppError;
use crate::git_process::{GIT_BLOB_BYTE_LIMIT, GitProcess, GitProcessError};
use crate::git_status::{GitFileStatus, GitStatusEntry, GitWorktreeMode};
use crate::large_file::reader::{ReadWindow, read_window};
use crate::source_control::slots::SnapshotId;
use crate::source_control::{
    SourceControlState, SourceStateRef, git_error_text, operation_bridge,
    remove_review_cancellable, track_review_cancellable,
};

const REVIEW_FILE_CAP: usize = 200;
const AGGREGATE_DECODED_BYTE_CAP: usize = 4 * 1024 * 1024;

pub(super) fn start(
    state: &SourceStateRef,
    tab: &Rc<EditorTab>,
    spec: ReviewTabSpec,
    entries: Vec<GitStatusEntry>,
    snapshot_id: SnapshotId,
) {
    if !operation_bridge::is_snapshot_current(state, &snapshot_id) {
        tab.set_git_review_text(&gettext("This review is out of date."));
        return;
    }
    let (process, repo) = {
        let state = state.borrow();
        (state.process.clone(), state.repo.clone())
    };
    let Some(process) = process else {
        tab.set_git_review_text(&gettext("Unable to start the Git review."));
        return;
    };
    let Some(repo) = repo else {
        tab.set_git_review_text(&gettext("No Git repository is active."));
        return;
    };
    tab.set_git_review_text(&ellipsis_label(gettext("Loading Git review")));
    let generation = spec.snapshot_generation_at_creation;
    let cancellable = gio::Cancellable::new();
    tab.set_review_load_cancellable(&cancellable);
    track_review_cancellable(state, &cancellable);
    let mut queue = VecDeque::from(entries);
    let skipped_for_cap = queue.len().saturating_sub(REVIEW_FILE_CAP);
    queue.truncate(REVIEW_FILE_CAP);
    let load = Rc::new(RefCell::new(ReviewLoad {
        source: Rc::downgrade(state),
        tab: Rc::clone(tab),
        process,
        repo,
        spec,
        cancellable,
        queue,
        inputs: Vec::new(),
        loaded_bytes: 0,
        skipped: 0,
        generation,
        snapshot_id,
    }));
    if skipped_for_cap > 0 {
        let mut state = load.borrow_mut();
        append_aggregate_skip_locked(&mut state, skipped_for_cap);
    }
    pump(load);
}

struct ReviewLoad {
    source: Weak<RefCell<SourceControlState>>,
    tab: Rc<EditorTab>,
    process: GitProcess,
    repo: PathBuf,
    spec: ReviewTabSpec,
    cancellable: gio::Cancellable,
    queue: VecDeque<GitStatusEntry>,
    inputs: Vec<ReviewFileInput>,
    loaded_bytes: usize,
    skipped: usize,
    generation: u64,
    snapshot_id: SnapshotId,
}

fn pump(load: Rc<RefCell<ReviewLoad>>) {
    if !load_is_current(&load) {
        abort(&load);
        return;
    }
    let next = {
        let mut state = load.borrow_mut();
        if state.loaded_bytes >= AGGREGATE_DECODED_BYTE_CAP {
            let remaining = state.queue.len();
            if remaining > 0 {
                append_aggregate_skip_locked(&mut state, remaining);
            }
        }
        let loaded_bytes = state.loaded_bytes;
        pop_next_with_aggregate_budget(&mut state.queue, loaded_bytes)
    };
    let Some(entry) = next else {
        finish(&load);
        return;
    };
    load_reference(load, entry);
}

fn pop_next_with_aggregate_budget<T>(queue: &mut VecDeque<T>, loaded_bytes: usize) -> Option<T> {
    if loaded_bytes >= AGGREGATE_DECODED_BYTE_CAP {
        queue.clear();
        None
    } else {
        queue.pop_front()
    }
}

fn load_reference(load: Rc<RefCell<ReviewLoad>>, entry: GitStatusEntry) {
    if !load_is_current(&load) {
        abort(&load);
        return;
    }
    let oid = reference_oid(load.borrow().spec.review_kind, &entry);
    let Some(oid) = oid else {
        load_current(load, entry, None);
        return;
    };
    let entry_for_callback = entry.clone();
    let load_for_callback = Rc::clone(&load);
    load_blob_text(
        &load,
        &oid,
        entry.path.display().to_string(),
        Rc::new(move |reference| {
            if !load_is_current(&load_for_callback) {
                abort(&load_for_callback);
                return;
            }
            match reference {
                Ok(reference) => load_current(
                    Rc::clone(&load_for_callback),
                    entry_for_callback.clone(),
                    Some(reference),
                ),
                Err(reason) => {
                    append_skip(&load_for_callback, &entry_for_callback, &reason);
                    pump(Rc::clone(&load_for_callback));
                }
            }
        }),
    );
}

fn load_current(load: Rc<RefCell<ReviewLoad>>, entry: GitStatusEntry, reference: Option<String>) {
    if !load_is_current(&load) {
        abort(&load);
        return;
    }
    let current = current_source(load.borrow().spec.review_kind, &entry);
    match current {
        CurrentSource::Empty => record_file(load, &entry, reference, None),
        CurrentSource::Blob(oid) => {
            let load_for_callback = Rc::clone(&load);
            load_blob_text(
                &load,
                &oid,
                entry.path.display().to_string(),
                Rc::new(move |current| {
                    if !load_is_current(&load_for_callback) {
                        abort(&load_for_callback);
                        return;
                    }
                    match current {
                        Ok(current) => record_file(
                            Rc::clone(&load_for_callback),
                            &entry,
                            reference.clone(),
                            Some(current),
                        ),
                        Err(reason) => {
                            append_skip(&load_for_callback, &entry, &reason);
                            pump(Rc::clone(&load_for_callback));
                        }
                    }
                }),
            );
        }
        CurrentSource::Worktree(path) => {
            let full_path = load.borrow().repo.join(path);
            load_worktree_text(load, entry, reference, full_path);
        }
    }
}

fn record_file(
    load: Rc<RefCell<ReviewLoad>>,
    entry: &GitStatusEntry,
    reference: Option<String>,
    current: Option<String>,
) {
    if !load_is_current(&load) {
        abort(&load);
        return;
    }
    let added_bytes = reference
        .as_ref()
        .map_or(0, String::len)
        .saturating_add(current.as_ref().map_or(0, String::len));
    {
        let mut state = load.borrow_mut();
        if state.loaded_bytes.saturating_add(added_bytes) > AGGREGATE_DECODED_BYTE_CAP {
            append_skip_locked(&mut state, entry, &aggregate_byte_limit_text());
            let remaining = state.queue.len();
            if remaining > 0 {
                append_aggregate_skip_locked(&mut state, remaining);
            }
            state.queue.clear();
        } else {
            state.loaded_bytes = state.loaded_bytes.saturating_add(added_bytes);
            let file_id = ReviewFileId::new(state.spec.review_kind, entry.path.raw().to_vec());
            state.inputs.push(ReviewFileInput::file(
                file_id,
                entry.status,
                reference,
                current,
            ));
        }
    }
    pump(load);
}

fn finish(load: &Rc<RefCell<ReviewLoad>>) {
    if !load_is_current(load) {
        abort(load);
        return;
    }
    let (tab, spec, inputs, cancellable) = {
        let state = load.borrow();
        (
            state.tab.clone(),
            state.spec.clone(),
            state.inputs.clone(),
            state.cancellable.clone(),
        )
    };
    tab.populate_review_session_with_spec(&spec, inputs);
    tab.clear_review_load_cancellable(&cancellable);
    if let Some(source) = load.borrow().source.upgrade() {
        remove_review_cancellable(&source, &cancellable);
    }
}

fn abort(load: &Rc<RefCell<ReviewLoad>>) {
    let (tab, cancellable, source) = {
        let state = load.borrow();
        (
            state.tab.clone(),
            state.cancellable.clone(),
            state.source.upgrade(),
        )
    };
    tab.clear_review_load_cancellable(&cancellable);
    if let Some(source) = source {
        remove_review_cancellable(&source, &cancellable);
    }
    let show_stale = {
        let state = load.borrow();
        let cancelled = state.cancellable.is_cancelled();
        let source = state.source.clone();
        let tab = Rc::clone(&state.tab);
        drop(state);
        !cancelled && review_tab_is_attached(&source, &tab)
    };
    if show_stale {
        tab.set_git_review_text(&gettext("This review is out of date."));
    }
}

fn load_is_current(load: &Rc<RefCell<ReviewLoad>>) -> bool {
    let (cancelled, source, generation, snapshot_id, tab) = {
        let load = load.borrow();
        (
            load.cancellable.is_cancelled(),
            load.source.clone(),
            load.generation,
            load.snapshot_id.clone(),
            Rc::clone(&load.tab),
        )
    };
    if cancelled {
        return false;
    }
    let Some(source_state) = source.upgrade() else {
        return false;
    };
    let source_current = {
        let source = source_state.borrow();
        source.review_generation == generation
            && source.operations.is_snapshot_current(&snapshot_id)
    };
    if !source_current {
        return false;
    }
    review_tab_is_attached(&source, &tab)
}

fn review_tab_is_attached(source: &Weak<RefCell<SourceControlState>>, tab: &Rc<EditorTab>) -> bool {
    let Some(source) = source.upgrade() else {
        return false;
    };
    let workspace = source.borrow().workspace.upgrade();
    let Some(workspace) = workspace else {
        return false;
    };
    let Some(page) = tab.page() else {
        return false;
    };
    workspace
        .find_tab_by_page(&page)
        .is_some_and(|attached| Rc::ptr_eq(&attached, tab))
}

fn load_blob_text(
    load: &Rc<RefCell<ReviewLoad>>,
    oid: &str,
    label: String,
    callback: Rc<dyn Fn(Result<String, String>)>,
) {
    let process = load.borrow().process.clone();
    let cancellable = load.borrow().cancellable.clone();
    let load = Rc::clone(load);
    process.cat_blob(
        oid,
        &cancellable,
        Rc::new(move |result| {
            if !load_is_current(&load) {
                abort(&load);
                return;
            }
            callback(result.and_then(decode_text).map_err(|error| {
                gettext("Unable to load Git blob for review.")
                    + "\n"
                    + &label
                    + "\n"
                    + &git_error_text(&error)
            }));
        }),
    );
}

fn load_worktree_text(
    load: Rc<RefCell<ReviewLoad>>,
    entry: GitStatusEntry,
    reference: Option<String>,
    path: PathBuf,
) {
    let file = gio::File::for_path(path);
    let cancellable = load.borrow().cancellable.clone();
    load_bounded_worktree_text(
        &file,
        GIT_BLOB_BYTE_LIMIT,
        Some(&cancellable),
        Rc::new(move |result| {
            if !load_is_current(&load) {
                abort(&load);
                return;
            }
            match result {
                Ok(current) => {
                    record_file(Rc::clone(&load), &entry, reference.clone(), Some(current));
                }
                Err(WorktreeReadError::Cancelled) => abort(&load),
                Err(WorktreeReadError::Git(error)) => {
                    append_skip(&load, &entry, &git_error_text(&error));
                    pump(Rc::clone(&load));
                }
                Err(WorktreeReadError::Read(message)) => {
                    append_skip(&load, &entry, &message);
                    pump(Rc::clone(&load));
                }
            }
        }),
    );
}

#[derive(Debug)]
enum WorktreeReadError {
    Cancelled,
    Git(GitProcessError),
    Read(String),
}

type WorktreeReadCallback = Rc<dyn Fn(Result<String, WorktreeReadError>)>;

fn load_bounded_worktree_text(
    file: &gio::File,
    limit: usize,
    cancellable: Option<&gio::Cancellable>,
    callback: WorktreeReadCallback,
) {
    let Some(read_budget) = limit.checked_add(1) else {
        callback(Err(WorktreeReadError::Git(GitProcessError::OutputTooLarge)));
        return;
    };
    read_window(
        file,
        0,
        read_budget,
        cancellable,
        Rc::new(move |result| match result {
            Ok(window) => callback(decode_complete_window(window, limit)),
            Err(AppError::Cancelled) => callback(Err(WorktreeReadError::Cancelled)),
            Err(AppError::ReadFailed(_path, message)) => {
                callback(Err(WorktreeReadError::Read(message)));
            }
            Err(error) => callback(Err(WorktreeReadError::Read(error.body()))),
        }),
    );
}

fn decode_complete_window(window: ReadWindow, limit: usize) -> Result<String, WorktreeReadError> {
    if !window.eof || window.bytes.len() > limit {
        return Err(WorktreeReadError::Git(GitProcessError::OutputTooLarge));
    }
    decode_text(window.bytes).map_err(WorktreeReadError::Git)
}

#[cfg(test)]
pub(super) type TestWorktreeReadCallback = Rc<dyn Fn(Result<String, String>)>;

#[cfg(test)]
pub(super) fn load_worktree_text_for_tests(
    file: &gio::File,
    limit: usize,
    cancellable: Option<&gio::Cancellable>,
    callback: TestWorktreeReadCallback,
) {
    load_bounded_worktree_text(
        file,
        limit,
        cancellable,
        Rc::new(move |result| {
            callback(result.map_err(|error| match error {
                WorktreeReadError::Cancelled => String::from("cancelled"),
                WorktreeReadError::Git(error) => git_error_text(&error),
                WorktreeReadError::Read(message) => message,
            }));
        }),
    );
}

#[cfg(test)]
pub(super) fn decode_worktree_window_for_tests(
    bytes: Vec<u8>,
    eof: bool,
    limit: usize,
) -> Result<String, String> {
    decode_complete_window(
        ReadWindow {
            offset: 0,
            bytes,
            eof,
        },
        limit,
    )
    .map_err(|error| match error {
        WorktreeReadError::Cancelled => String::from("cancelled"),
        WorktreeReadError::Git(error) => git_error_text(&error),
        WorktreeReadError::Read(message) => message,
    })
}

fn decode_text(bytes: Vec<u8>) -> Result<String, GitProcessError> {
    if bytes.contains(&0) {
        return Err(GitProcessError::BinaryContent);
    }
    String::from_utf8(bytes).map_err(|_| GitProcessError::ParseFailed)
}

fn append_skip(load: &Rc<RefCell<ReviewLoad>>, entry: &GitStatusEntry, reason: &str) {
    let mut state = load.borrow_mut();
    append_skip_locked(&mut state, entry, reason);
}

fn append_skip_locked(state: &mut ReviewLoad, entry: &GitStatusEntry, reason: &str) {
    state.skipped = state.skipped.saturating_add(1);
    state.inputs.push(ReviewFileInput::skipped(
        ReviewFileId::new(state.spec.review_kind, entry.path.raw().to_vec()),
        entry.status,
        reason,
    ));
}

fn append_aggregate_skip_locked(state: &mut ReviewLoad, remaining: usize) {
    state.skipped = state.skipped.saturating_add(remaining);
    let count = u32::try_from(remaining).map_or(u32::MAX, |value| value);
    let reason = ngettext(
        "%d more file skipped (review limit reached).",
        "%d more files skipped (review limit reached).",
        count,
    )
    .replace("%d", &remaining.to_string());
    state.inputs.push(ReviewFileInput::skipped(
        ReviewFileId::new(state.spec.review_kind, Vec::new()),
        GitFileStatus::Unsupported,
        reason,
    ));
}

fn reference_oid(kind: ReviewKind, entry: &GitStatusEntry) -> Option<String> {
    match kind {
        ReviewKind::Staged if entry.status != GitFileStatus::Added => entry.head_oid.clone(),
        ReviewKind::Unstaged if entry.status != GitFileStatus::Untracked => entry.index_oid.clone(),
        ReviewKind::Staged | ReviewKind::Unstaged => None,
    }
}

fn current_source(kind: ReviewKind, entry: &GitStatusEntry) -> CurrentSource {
    match kind {
        ReviewKind::Staged if entry.status == GitFileStatus::Deleted => CurrentSource::Empty,
        ReviewKind::Staged => entry
            .index_oid
            .clone()
            .map_or(CurrentSource::Empty, CurrentSource::Blob),
        ReviewKind::Unstaged
            if entry.status == GitFileStatus::Deleted
                || entry.worktree_mode == GitWorktreeMode::Absent =>
        {
            CurrentSource::Empty
        }
        ReviewKind::Unstaged => entry.path.as_utf8().map_or_else(
            || {
                CurrentSource::Worktree(PathBuf::from(OsString::from_vec(
                    entry.path.raw().to_vec(),
                )))
            },
            |path| CurrentSource::Worktree(PathBuf::from(path)),
        ),
    }
}

enum CurrentSource {
    Empty,
    Blob(String),
    Worktree(PathBuf),
}

fn ellipsis_label(mut label: String) -> String {
    label.push('…');
    label
}

fn aggregate_byte_limit_text() -> String {
    gettext("Diff was skipped because the files are over the compare byte limit.")
}

#[cfg(test)]
#[path = "review_loader_tests.rs"]
mod tests;
