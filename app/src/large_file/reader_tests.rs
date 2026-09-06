use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

use super::{ReadProgress, ReadWindow, append_read_chunk, read_window};
use crate::error::AppError;

#[test]
fn read_window_records_offset_and_eof() {
    let window = ReadWindow {
        offset: 12,
        bytes: b"abc".to_vec(),
        eof: true,
    };
    assert_eq!(window.offset, 12);
    assert!(window.eof);
}

#[test]
fn short_nonzero_read_requests_more_bytes() {
    let mut bytes = Vec::new();

    assert_eq!(
        append_read_chunk(&mut bytes, 6, b"abc"),
        ReadProgress::Continue
    );
    assert_eq!(
        append_read_chunk(&mut bytes, 6, b"def"),
        ReadProgress::Complete
    );
    assert_eq!(bytes, b"abcdef");
}

#[test]
fn zero_byte_read_reports_eof() {
    let mut bytes = b"abc".to_vec();

    assert_eq!(append_read_chunk(&mut bytes, 6, b""), ReadProgress::Eof);
    assert_eq!(bytes, b"abc");
}

#[test]
fn read_window_reads_from_requested_offset() {
    let (path, file) = temp_file(ReaderTempFile::Offset, b"alpha\nbeta");
    let result = wait_for_read(&file, 6, 8);

    assert!(result.is_ok());
    let Ok(window) = result else {
        return;
    };
    assert_eq!(window.offset, 6);
    assert_eq!(window.bytes, b"beta");
    assert!(window.eof);
    let _removed = fs::remove_file(path);
}

#[test]
fn read_window_reports_eof_after_forward_skip() {
    let (path, file) = temp_file(ReaderTempFile::Eof, b"short");
    let result = wait_for_read(&file, 512, 8);

    assert!(result.is_ok());
    let Ok(window) = result else {
        return;
    };
    assert_eq!(window.offset, 512);
    assert!(window.bytes.is_empty());
    assert!(window.eof);
    let _removed = fs::remove_file(path);
}

#[test]
fn read_window_maps_cancelled_reads() {
    let (path, file) = temp_file(ReaderTempFile::Cancel, b"cancelled");
    let cancellable = gio::Cancellable::new();
    cancellable.cancel();
    let result = wait_for_read_with_cancellable(&file, 0, 8, Some(&cancellable));

    assert!(matches!(result, Err(AppError::Cancelled)));
    let _removed = fs::remove_file(path);
}

#[derive(Clone, Copy)]
enum ReaderTempFile {
    Offset,
    Eof,
    Cancel,
}

impl ReaderTempFile {
    fn name(self) -> &'static str {
        match self {
            Self::Offset => "offset",
            Self::Eof => "eof",
            Self::Cancel => "cancel",
        }
    }
}

fn temp_file(fixture: ReaderTempFile, contents: &[u8]) -> (PathBuf, gio::File) {
    let path = PathBuf::from("/tmp").join(format!(
        "riteed-large-file-reader-{}-{}.txt",
        std::process::id(),
        fixture.name()
    ));
    assert!(fs::write(&path, contents).is_ok());
    let file = gio::File::for_path(&path);
    (path, file)
}

fn wait_for_read(file: &gio::File, offset: u64, max_bytes: usize) -> Result<ReadWindow, AppError> {
    wait_for_read_with_cancellable(file, offset, max_bytes, None)
}

fn wait_for_read_with_cancellable(
    file: &gio::File,
    offset: u64,
    max_bytes: usize,
    cancellable: Option<&gio::Cancellable>,
) -> Result<ReadWindow, AppError> {
    let context = glib::MainContext::new();
    let entered = context.with_thread_default(|| {
        let result = Rc::new(RefCell::new(None));
        let result_for_callback = Rc::clone(&result);
        read_window(
            file,
            offset,
            max_bytes,
            cancellable,
            Rc::new(move |value| {
                *result_for_callback.borrow_mut() = Some(value);
            }),
        );
        spin_until_result(&context, &result)
    });
    assert!(
        entered.is_ok(),
        "private reader context could not become thread-default"
    );
    match entered {
        Ok(value) => value,
        Err(_) => Err(AppError::Cancelled),
    }
}

fn spin_until_result(
    context: &glib::MainContext,
    result: &Rc<RefCell<Option<Result<ReadWindow, AppError>>>>,
) -> Result<ReadWindow, AppError> {
    let expired = Arc::new(AtomicBool::new(false));
    let expired_for_timeout = Arc::clone(&expired);
    let timeout = glib::timeout_source_new(
        Duration::from_secs(5),
        Some("riteed-reader-test-deadline"),
        glib::Priority::DEFAULT,
        move || {
            expired_for_timeout.store(true, Ordering::Release);
            glib::ControlFlow::Break
        },
    );
    timeout.attach(Some(context));
    while result.borrow().is_none() && !expired.load(Ordering::Acquire) {
        let _dispatched = context.iteration(true);
    }
    timeout.destroy();

    assert!(
        result.borrow().is_some(),
        "reader callback did not settle before the private-context deadline"
    );
    match result.borrow_mut().take() {
        Some(value) => value,
        None => Err(AppError::Cancelled),
    }
}
