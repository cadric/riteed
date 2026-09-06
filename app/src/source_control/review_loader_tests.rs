use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use gtk4::prelude::CancellableExt;
use gtk4::{gio, glib};

use super::super::git_error_text;
use super::{
    AGGREGATE_DECODED_BYTE_CAP, CurrentSource, aggregate_byte_limit_text, current_source,
    decode_text, decode_worktree_window_for_tests, load_worktree_text_for_tests,
    pop_next_with_aggregate_budget, reference_oid,
};
use crate::editor_tab::ReviewKind;
use crate::git_process::{GIT_BLOB_BYTE_LIMIT, GitProcessError};
use crate::git_status::{GitFileStatus, GitPath, GitStatusEntry, GitWorktreeMode};
use crate::large_file::reader::install_read_test_hooks;

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

#[test]
fn real_worktree_loader_rejects_limit_plus_one() {
    let path = temp_path("over-cap");
    assert!(fs::write(&path, vec![b'x'; 128]).is_ok());
    let reads = Rc::new(Cell::new(0));
    let bytes_read = Rc::new(Cell::new(0_usize));
    let bytes_for_observer = Rc::clone(&bytes_read);
    let closes = Rc::new(RefCell::new(Vec::new()));
    let closes_for_observer = Rc::clone(&closes);
    let _hooks = install_read_test_hooks(
        None,
        Some(Rc::new(move |count| {
            bytes_for_observer.set(bytes_for_observer.get().saturating_add(count));
        })),
        Some(Rc::new(move |closed| {
            closes_for_observer.borrow_mut().push(closed);
        })),
    );
    let result = wait_for_load(&gio::File::for_path(&path), 5, None, Rc::clone(&reads));

    assert_eq!(reads.get(), 1);
    assert_eq!(bytes_read.get(), 6);
    assert_eq!(closes.borrow().as_slice(), &[true]);
    assert_eq!(
        result,
        Err(git_error_text(&GitProcessError::OutputTooLarge))
    );
    let _removed = fs::remove_file(path);
}

#[test]
fn short_chunks_are_stitched_until_eof_at_the_exact_limit() {
    let path = temp_path("short-chunks");
    assert!(fs::write(&path, b"abcde").is_ok());
    let callbacks = Rc::new(Cell::new(0));
    let bytes_read = Rc::new(Cell::new(0_usize));
    let read_calls = Rc::new(Cell::new(0_usize));
    let bytes_for_observer = Rc::clone(&bytes_read);
    let calls_for_observer = Rc::clone(&read_calls);
    let closes = Rc::new(Cell::new(0_usize));
    let closes_for_observer = Rc::clone(&closes);
    let _hooks = install_read_test_hooks(
        Some(2),
        Some(Rc::new(move |count| {
            calls_for_observer.set(calls_for_observer.get().saturating_add(1));
            bytes_for_observer.set(bytes_for_observer.get().saturating_add(count));
        })),
        Some(Rc::new(move |closed| {
            assert!(closed);
            closes_for_observer.set(closes_for_observer.get().saturating_add(1));
        })),
    );

    let result = wait_for_load(&gio::File::for_path(&path), 5, None, callbacks);

    assert_eq!(result.as_deref(), Ok("abcde"));
    assert_eq!(bytes_read.get(), 5);
    assert!(read_calls.get() >= 3);
    assert_eq!(closes.get(), 1);
    let _removed = fs::remove_file(path);
}

#[test]
fn cancellation_between_chunks_closes_once_and_completes_once() {
    let path = temp_path("cancel-between-chunks");
    assert!(fs::write(&path, b"abcdef").is_ok());
    let callbacks = Rc::new(Cell::new(0));
    let cancellable = gio::Cancellable::new();
    let cancellable_for_observer = cancellable.clone();
    let bytes_read = Rc::new(Cell::new(0_usize));
    let bytes_for_observer = Rc::clone(&bytes_read);
    let closes = Rc::new(Cell::new(0_usize));
    let closes_for_observer = Rc::clone(&closes);
    let _hooks = install_read_test_hooks(
        Some(2),
        Some(Rc::new(move |count| {
            bytes_for_observer.set(bytes_for_observer.get().saturating_add(count));
            if count > 0 {
                cancellable_for_observer.cancel();
            }
        })),
        Some(Rc::new(move |closed| {
            assert!(closed);
            closes_for_observer.set(closes_for_observer.get().saturating_add(1));
        })),
    );

    let result = wait_for_load(
        &gio::File::for_path(&path),
        5,
        Some(&cancellable),
        Rc::clone(&callbacks),
    );

    assert_eq!(result, Err(String::from("cancelled")));
    assert_eq!(bytes_read.get(), 2);
    assert_eq!(callbacks.get(), 1);
    assert_eq!(closes.get(), 1);
    let _removed = fs::remove_file(path);
}

#[test]
fn incomplete_or_non_text_windows_are_not_published() {
    let too_large = git_error_text(&GitProcessError::OutputTooLarge);
    assert_eq!(
        decode_worktree_window_for_tests(b"abc".to_vec(), false, 5),
        Err(too_large)
    );
    assert_eq!(
        decode_worktree_window_for_tests(b"a\0b".to_vec(), true, 5),
        Err(git_error_text(&GitProcessError::BinaryContent))
    );
    assert_eq!(
        decode_worktree_window_for_tests(vec![0xff], true, 5),
        Err(git_error_text(&GitProcessError::ParseFailed))
    );
}

#[test]
fn empty_file_and_checked_sentinel_overflow_settle_once() {
    let path = temp_path("empty");
    assert!(fs::write(&path, []).is_ok());
    let callbacks = Rc::new(Cell::new(0));
    let result = wait_for_load(&gio::File::for_path(&path), 5, None, Rc::clone(&callbacks));
    assert_eq!(result.as_deref(), Ok(""));
    assert_eq!(callbacks.get(), 1);

    let overflow_callbacks = Rc::new(Cell::new(0));
    let overflow = wait_for_load(
        &gio::File::for_path(&path),
        usize::MAX,
        None,
        Rc::clone(&overflow_callbacks),
    );
    assert_eq!(
        overflow,
        Err(git_error_text(&GitProcessError::OutputTooLarge))
    );
    assert_eq!(overflow_callbacks.get(), 1);
    let _removed = fs::remove_file(path);
}

#[test]
fn read_open_error_settles_once() {
    let callbacks = Rc::new(Cell::new(0));
    let result = wait_for_load(
        &gio::File::for_path(temp_path("missing")),
        5,
        None,
        Rc::clone(&callbacks),
    );

    assert!(result.is_err());
    assert_eq!(callbacks.get(), 1);
}

#[test]
fn shared_blob_limit_accepts_the_complete_exact_boundary() {
    let path = temp_path("shared-exact-cap");
    assert_eq!(GIT_BLOB_BYTE_LIMIT, 1_000_001);
    assert!(fs::write(&path, vec![b'x'; GIT_BLOB_BYTE_LIMIT]).is_ok());
    let callbacks = Rc::new(Cell::new(0));

    let result = wait_for_load(
        &gio::File::for_path(&path),
        GIT_BLOB_BYTE_LIMIT,
        None,
        Rc::clone(&callbacks),
    );

    assert_eq!(result.as_ref().map(String::len), Ok(GIT_BLOB_BYTE_LIMIT));
    assert_eq!(callbacks.get(), 1);
    let _removed = fs::remove_file(path);
}

#[test]
fn nul_and_invalid_utf8_are_rejected_through_the_real_loader() {
    for (label, bytes, expected) in [
        ("nul", b"a\0b".as_slice(), GitProcessError::BinaryContent),
        (
            "invalid-utf8",
            [0xff].as_slice(),
            GitProcessError::ParseFailed,
        ),
    ] {
        let path = temp_path(label);
        assert!(fs::write(&path, bytes).is_ok());
        let result = wait_for_load(&gio::File::for_path(&path), 5, None, Rc::new(Cell::new(0)));
        assert_eq!(result, Err(git_error_text(&expected)));
        let _removed = fs::remove_file(path);
    }
}

#[test]
fn exhausted_aggregate_budget_clears_queue_without_next_item() {
    let mut queue = std::collections::VecDeque::from([1_u8, 2]);

    assert_eq!(
        pop_next_with_aggregate_budget(&mut queue, AGGREGATE_DECODED_BYTE_CAP,),
        None
    );
    assert!(queue.is_empty());
}

fn wait_for_load(
    file: &gio::File,
    limit: usize,
    cancellable: Option<&gio::Cancellable>,
    callbacks: Rc<Cell<usize>>,
) -> Result<String, String> {
    let context = glib::MainContext::new();
    let entered = context.with_thread_default(|| {
        let result = Rc::new(RefCell::new(None));
        let result_for_callback = Rc::clone(&result);
        load_worktree_text_for_tests(
            file,
            limit,
            cancellable,
            Rc::new(move |value| {
                callbacks.set(callbacks.get().saturating_add(1));
                *result_for_callback.borrow_mut() = Some(value);
            }),
        );

        let expired = Arc::new(AtomicBool::new(false));
        let expired_for_timeout = Arc::clone(&expired);
        let timeout = glib::timeout_source_new(
            Duration::from_secs(5),
            Some("riteed-review-loader-test-deadline"),
            glib::Priority::DEFAULT,
            move || {
                expired_for_timeout.store(true, Ordering::Release);
                glib::ControlFlow::Break
            },
        );
        timeout.attach(Some(&context));
        while result.borrow().is_none() && !expired.load(Ordering::Acquire) {
            let _dispatched = context.iteration(true);
        }
        timeout.destroy();

        assert!(
            result.borrow().is_some(),
            "loader callback did not settle before the private-context deadline"
        );
        match result.borrow_mut().take() {
            Some(value) => value,
            None => Err(String::new()),
        }
    });
    assert!(
        entered.is_ok(),
        "private loader context could not become thread-default"
    );
    match entered {
        Ok(value) => value,
        Err(_) => Err(String::new()),
    }
}

fn temp_path(label: &str) -> PathBuf {
    PathBuf::from("/tmp").join(format!(
        "riteed-review-loader-{}-{label}.txt",
        std::process::id()
    ))
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
