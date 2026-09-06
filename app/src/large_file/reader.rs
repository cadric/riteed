#[cfg(test)]
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, glib, prelude::*};

use crate::error::AppError;
use crate::large_file::file_path_for_error;

const READ_CHUNK_BYTE_LIMIT: usize = 64 * 1024;

#[cfg(test)]
type ReadObserver = Rc<dyn Fn(usize)>;
#[cfg(test)]
type CloseObserver = Rc<dyn Fn(bool)>;

#[cfg(test)]
thread_local! {
    static TEST_READ_CHUNK_LIMIT: Cell<Option<usize>> = const { Cell::new(None) };
    static TEST_READ_OBSERVER: RefCell<Option<ReadObserver>> = const { RefCell::new(None) };
    static TEST_CLOSE_OBSERVER: RefCell<Option<CloseObserver>> = const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct ReadTestHooks;

#[cfg(test)]
impl Drop for ReadTestHooks {
    fn drop(&mut self) {
        TEST_READ_CHUNK_LIMIT.with(|limit| limit.set(None));
        TEST_READ_OBSERVER.with(|observer| *observer.borrow_mut() = None);
        TEST_CLOSE_OBSERVER.with(|observer| *observer.borrow_mut() = None);
    }
}

#[cfg(test)]
pub(crate) fn install_read_test_hooks(
    chunk_limit: Option<usize>,
    read_observer: Option<ReadObserver>,
    close_observer: Option<CloseObserver>,
) -> ReadTestHooks {
    TEST_READ_CHUNK_LIMIT.with(|limit| limit.set(chunk_limit));
    TEST_READ_OBSERVER.with(|slot| *slot.borrow_mut() = read_observer);
    TEST_CLOSE_OBSERVER.with(|slot| *slot.borrow_mut() = close_observer);
    ReadTestHooks
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReadWindow {
    pub(crate) offset: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) eof: bool,
}

pub(crate) type ReadCallback = Rc<dyn Fn(Result<ReadWindow, AppError>)>;

#[derive(Clone)]
pub(crate) struct OpenedStream {
    stream: gio::FileInputStream,
    display_path: PathBuf,
}

pub(crate) type OpenStreamCallback = Rc<dyn Fn(Result<OpenedStream, AppError>)>;

pub(crate) fn open_stream(
    file: &gio::File,
    cancellable: Option<&gio::Cancellable>,
    callback: OpenStreamCallback,
) {
    let display_path = file_path_for_error(file);
    let file = file.clone();
    let cancellable = cancellable.cloned();
    file.read_async(
        glib::Priority::DEFAULT,
        cancellable.as_ref(),
        move |result| match result {
            Ok(stream) => callback(Ok(OpenedStream {
                stream,
                display_path,
            })),
            Err(error) => callback(Err(map_read_error(&display_path, &error))),
        },
    );
}

// PARSER-BOUNDARY: id=large_file_paged_reader
pub(crate) fn read_window(
    file: &gio::File,
    offset: u64,
    max_bytes: usize,
    cancellable: Option<&gio::Cancellable>,
    callback: ReadCallback,
) {
    let display_path = file_path_for_error(file);
    let cancellable = cancellable.cloned();
    let file = file.clone();
    let cancellable_for_open = cancellable.clone();
    file.read_async(
        glib::Priority::DEFAULT,
        cancellable_for_open.as_ref(),
        move |result| match result {
            Ok(stream) => {
                let stream_for_close = stream.clone();
                let path_for_close = display_path.clone();
                let callback_for_close = callback.clone();
                skip_to_offset(
                    &stream,
                    display_path,
                    offset,
                    max_bytes,
                    cancellable.as_ref(),
                    Rc::new(move |read_result| {
                        close_owned_stream(
                            &stream_for_close,
                            &path_for_close,
                            read_result,
                            callback_for_close.clone(),
                        );
                    }),
                );
            }
            Err(error) => callback(Err(map_read_error(&display_path, &error))),
        },
    );
}

fn close_owned_stream(
    stream: &gio::FileInputStream,
    display_path: &Path,
    read_result: Result<ReadWindow, AppError>,
    callback: ReadCallback,
) {
    #[cfg(test)]
    let stream_for_callback = stream.clone();
    let display_path = display_path.to_path_buf();
    stream.close_async(
        glib::Priority::DEFAULT,
        None::<&gio::Cancellable>,
        move |close_result| {
            #[cfg(test)]
            TEST_CLOSE_OBSERVER.with(|observer| {
                if let Some(observer) = observer.borrow().as_ref() {
                    observer(stream_for_callback.is_closed());
                }
            });
            match (read_result, close_result) {
                (Err(error), _) => callback(Err(error)),
                (Ok(window), Ok(())) => callback(Ok(window)),
                (Ok(_window), Err(error)) => callback(Err(map_read_error(&display_path, &error))),
            }
        },
    );
}

pub(crate) fn read_open_stream_window(
    opened: &OpenedStream,
    offset: u64,
    max_bytes: usize,
    cancellable: Option<&gio::Cancellable>,
    callback: ReadCallback,
) {
    read_from_stream(
        &opened.stream,
        opened.display_path.clone(),
        offset,
        max_bytes,
        cancellable,
        callback,
    );
}

fn skip_to_offset(
    stream: &gio::FileInputStream,
    display_path: PathBuf,
    offset: u64,
    max_bytes: usize,
    cancellable: Option<&gio::Cancellable>,
    callback: ReadCallback,
) {
    if offset == 0 {
        read_from_stream(
            stream,
            display_path,
            offset,
            max_bytes,
            cancellable,
            callback,
        );
        return;
    }

    let Ok(skip_count) = usize::try_from(offset) else {
        callback(Err(AppError::ReadFailed(
            display_path,
            gettext("The file position is too large for this system."),
        )));
        return;
    };
    skip_remaining(
        stream,
        display_path,
        offset,
        skip_count,
        max_bytes,
        cancellable.cloned(),
        callback,
    );
}

fn skip_remaining(
    stream: &gio::FileInputStream,
    display_path: PathBuf,
    target_offset: u64,
    remaining: usize,
    max_bytes: usize,
    cancellable: Option<gio::Cancellable>,
    callback: ReadCallback,
) {
    if remaining == 0 {
        read_from_stream(
            stream,
            display_path,
            target_offset,
            max_bytes,
            cancellable.as_ref(),
            callback,
        );
        return;
    }

    let stream_for_callback = stream.clone();
    let cancellable_for_skip = cancellable.clone();
    stream.skip_async(
        remaining,
        glib::Priority::DEFAULT,
        cancellable_for_skip.as_ref(),
        move |result| match result {
            Ok(skipped) if skipped > 0 => {
                let skipped = usize::try_from(skipped).map_or(remaining, |value| value);
                skip_remaining(
                    &stream_for_callback,
                    display_path,
                    target_offset,
                    remaining.saturating_sub(skipped),
                    max_bytes,
                    cancellable,
                    callback,
                );
            }
            Ok(_) => callback(Ok(ReadWindow {
                offset: target_offset,
                bytes: Vec::new(),
                eof: true,
            })),
            Err(error) => callback(Err(map_read_error(&display_path, &error))),
        },
    );
}

fn read_from_stream(
    stream: &gio::FileInputStream,
    display_path: PathBuf,
    offset: u64,
    max_bytes: usize,
    cancellable: Option<&gio::Cancellable>,
    callback: ReadCallback,
) {
    read_more_from_stream(
        stream,
        display_path,
        offset,
        max_bytes,
        Vec::with_capacity(max_bytes),
        cancellable.cloned(),
        callback,
    );
}

fn read_more_from_stream(
    stream: &gio::FileInputStream,
    display_path: PathBuf,
    offset: u64,
    max_bytes: usize,
    mut bytes: Vec<u8>,
    cancellable: Option<gio::Cancellable>,
    callback: ReadCallback,
) {
    if cancellable
        .as_ref()
        .is_some_and(gio::Cancellable::is_cancelled)
    {
        callback(Err(AppError::Cancelled));
        return;
    }
    if bytes.len() >= max_bytes {
        callback(Ok(ReadWindow {
            offset,
            bytes,
            eof: false,
        }));
        return;
    }
    let remaining = max_bytes.saturating_sub(bytes.len());
    let request_bytes = remaining.min(read_chunk_byte_limit());
    let stream_for_callback = stream.clone();
    let cancellable_for_read = cancellable.clone();
    stream.read_bytes_async(
        request_bytes,
        glib::Priority::DEFAULT,
        cancellable_for_read.as_ref(),
        move |result| match result {
            Ok(chunk) => {
                #[cfg(test)]
                TEST_READ_OBSERVER.with(|observer| {
                    if let Some(observer) = observer.borrow().as_ref() {
                        observer(chunk.len());
                    }
                });
                let chunk = chunk.to_vec();
                match append_read_chunk(&mut bytes, max_bytes, &chunk) {
                    ReadProgress::Continue => read_more_from_stream(
                        &stream_for_callback,
                        display_path,
                        offset,
                        max_bytes,
                        bytes,
                        cancellable,
                        callback,
                    ),
                    ReadProgress::Complete => callback(Ok(ReadWindow {
                        offset,
                        bytes,
                        eof: false,
                    })),
                    ReadProgress::Eof => callback(Ok(ReadWindow {
                        offset,
                        bytes,
                        eof: true,
                    })),
                }
            }
            Err(error) => callback(Err(map_read_error(&display_path, &error))),
        },
    );
}

fn read_chunk_byte_limit() -> usize {
    #[cfg(test)]
    if let Some(limit) = TEST_READ_CHUNK_LIMIT.with(Cell::get) {
        return limit.max(1);
    }
    READ_CHUNK_BYTE_LIMIT
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadProgress {
    Continue,
    Complete,
    Eof,
}

fn append_read_chunk(bytes: &mut Vec<u8>, max_bytes: usize, chunk: &[u8]) -> ReadProgress {
    if bytes.len() >= max_bytes {
        return ReadProgress::Complete;
    }
    if chunk.is_empty() {
        return ReadProgress::Eof;
    }
    let remaining = max_bytes.saturating_sub(bytes.len());
    let take = remaining.min(chunk.len());
    bytes.extend_from_slice(&chunk[..take]);
    if bytes.len() >= max_bytes {
        ReadProgress::Complete
    } else {
        ReadProgress::Continue
    }
}

pub(crate) fn map_read_error(path: &Path, error: &glib::Error) -> AppError {
    if error.matches(gio::IOErrorEnum::Cancelled) {
        AppError::Cancelled
    } else {
        AppError::ReadFailed(path.to_path_buf(), error.message().to_string())
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod tests;
