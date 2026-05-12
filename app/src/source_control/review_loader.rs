use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use gettextrs::{gettext, ngettext};
use gtk4::{gio, prelude::*};

use crate::editor_tab::{
    EditorTab, ReviewFileId, ReviewFileInput, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec,
};
use crate::git_process::{GitProcess, GitProcessError};
use crate::git_status::{GitFileStatus, GitStatusEntry, GitWorktreeMode};
use crate::source_control::{SourceControlState, SourceStateRef, git_error_text};

const REVIEW_FILE_CAP: usize = 200;
const AGGREGATE_DECODED_BYTE_CAP: usize = 4 * 1024 * 1024;

pub(super) fn start(
    state: &SourceStateRef,
    tab: &Rc<EditorTab>,
    spec: ReviewTabSpec,
    entries: Vec<GitStatusEntry>,
) {
    let Some(process) = state.borrow().process.clone() else {
        tab.set_git_review_text(&gettext("Unable to start the Git review."));
        return;
    };
    let Some(repo) = state.borrow().repo.clone() else {
        tab.set_git_review_text(&gettext("No Git repository is active."));
        return;
    };
    tab.set_git_review_text(&ellipsis_label(gettext("Loading Git review")));
    let generation = spec.snapshot_generation_at_creation;
    let mut queue = VecDeque::from(entries);
    let skipped_for_cap = queue.len().saturating_sub(REVIEW_FILE_CAP);
    queue.truncate(REVIEW_FILE_CAP);
    let load = Rc::new(RefCell::new(ReviewLoad {
        source: Rc::downgrade(state),
        tab: Rc::clone(tab),
        process,
        repo,
        spec,
        cancellable: gio::Cancellable::new(),
        queue,
        inputs: Vec::new(),
        loaded_bytes: 0,
        skipped: 0,
        generation,
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
}

fn pump(load: Rc<RefCell<ReviewLoad>>) {
    if load.borrow().cancellable.is_cancelled() {
        return;
    }
    let Some(entry) = load.borrow_mut().queue.pop_front() else {
        finish(&load);
        return;
    };
    load_reference(load, entry);
}

fn load_reference(load: Rc<RefCell<ReviewLoad>>, entry: GitStatusEntry) {
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
        Rc::new(move |reference| match reference {
            Ok(reference) => load_current(
                Rc::clone(&load_for_callback),
                entry_for_callback.clone(),
                Some(reference),
            ),
            Err(reason) => {
                append_skip(&load_for_callback, &entry_for_callback, &reason);
                pump(Rc::clone(&load_for_callback));
            }
        }),
    );
}

fn load_current(load: Rc<RefCell<ReviewLoad>>, entry: GitStatusEntry, reference: Option<String>) {
    let current = current_source(load.borrow().spec.review_kind, &entry);
    match current {
        CurrentSource::Empty => record_file(load, &entry, reference, None),
        CurrentSource::Blob(oid) => {
            let load_for_callback = Rc::clone(&load);
            load_blob_text(
                &load,
                &oid,
                entry.path.display().to_string(),
                Rc::new(move |current| match current {
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
    let state = load.borrow();
    state
        .tab
        .populate_review_session_with_spec(&state.spec, state.inputs.clone());
    if review_is_stale(&state) {
        let stale_generation = state.generation.saturating_add(1);
        let _shown = state.tab.mark_review_stale_if_mismatch(
            &ReviewSnapshotFingerprint::new("stale-review-load"),
            stale_generation,
        );
    }
}

fn review_is_stale(load: &ReviewLoad) -> bool {
    load.source
        .upgrade()
        .is_some_and(|state| state.borrow().review_generation != load.generation)
}

fn load_blob_text(
    load: &Rc<RefCell<ReviewLoad>>,
    oid: &str,
    label: String,
    callback: Rc<dyn Fn(Result<String, String>)>,
) {
    let process = load.borrow().process.clone();
    let cancellable = load.borrow().cancellable.clone();
    process.cat_blob(
        oid,
        &cancellable,
        Rc::new(move |result| {
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
    file.load_contents_async(Some(&cancellable), move |result| match result {
        Ok((bytes, _etag)) => match decode_text(bytes.to_vec()) {
            Ok(current) => record_file(Rc::clone(&load), &entry, reference.clone(), Some(current)),
            Err(error) => {
                append_skip(&load, &entry, &git_error_text(&error));
                pump(Rc::clone(&load));
            }
        },
        Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {}
        Err(error) => {
            append_skip(&load, &entry, error.message());
            pump(Rc::clone(&load));
        }
    });
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
mod tests {
    use crate::editor_tab::ReviewKind;
    use crate::git_process::GitProcessError;
    use crate::git_status::{GitFileStatus, GitPath, GitStatusEntry, GitWorktreeMode};
    use crate::source_control::review_loader::{
        CurrentSource, aggregate_byte_limit_text, current_source, decode_text, reference_oid,
    };

    #[test]
    fn staged_delete_uses_head_vs_empty() {
        let mut entry = entry(GitFileStatus::Deleted);
        entry.head_oid = Some(String::from("head"));
        entry.index_oid = None;

        assert_eq!(
            reference_oid(ReviewKind::Staged, &entry).as_deref(),
            Some("head")
        );
        assert!(matches!(
            current_source(ReviewKind::Staged, &entry),
            CurrentSource::Empty
        ));
    }

    #[test]
    fn staged_add_uses_empty_vs_index() {
        let entry = entry(GitFileStatus::Added);

        assert_eq!(reference_oid(ReviewKind::Staged, &entry), None);
        assert!(matches!(
            current_source(ReviewKind::Staged, &entry),
            CurrentSource::Blob(_)
        ));
    }

    #[test]
    fn unstaged_delete_uses_index_vs_empty() {
        let entry = entry(GitFileStatus::Deleted);

        assert_eq!(
            reference_oid(ReviewKind::Unstaged, &entry).as_deref(),
            Some("index")
        );
        assert!(matches!(
            current_source(ReviewKind::Unstaged, &entry),
            CurrentSource::Empty
        ));
    }

    #[test]
    fn text_decoder_rejects_binary_and_invalid_utf8() {
        assert_eq!(decode_text(b"hello".to_vec()).as_deref(), Ok("hello"));
        assert_eq!(
            decode_text(b"hello\0world".to_vec()),
            Err(GitProcessError::BinaryContent)
        );
        assert_eq!(decode_text(vec![0xff]), Err(GitProcessError::ParseFailed));
    }

    #[test]
    fn aggregate_byte_limit_reason_matches_diff_skip_copy() {
        assert_eq!(
            aggregate_byte_limit_text(),
            "Diff was skipped because the files are over the compare byte limit."
        );
    }

    fn entry(status: GitFileStatus) -> GitStatusEntry {
        GitStatusEntry::with_worktree_mode(
            GitPath::from_bytes(b"file.txt"),
            status,
            Some(String::from("head")),
            Some(String::from("index")),
            true,
            true,
            GitWorktreeMode::Regular("100644"),
        )
    }
}
