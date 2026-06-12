use std::path::{Path, PathBuf};
use std::rc::Rc;

use gettextrs::gettext;
use gtk4::{gio, glib, prelude::*};

use crate::error::AppError;
use crate::large_file::file_path_for_error;

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
            Ok(stream) => skip_to_offset(
                &stream,
                display_path,
                offset,
                max_bytes,
                cancellable.as_ref(),
                callback.clone(),
            ),
            Err(error) => callback(Err(map_read_error(&display_path, &error))),
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
    let stream_for_callback = stream.clone();
    let cancellable_for_read = cancellable.clone();
    stream.read_bytes_async(
        remaining,
        glib::Priority::DEFAULT,
        cancellable_for_read.as_ref(),
        move |result| match result {
            Ok(chunk) => {
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
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

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

    fn wait_for_read(
        file: &gio::File,
        offset: u64,
        max_bytes: usize,
    ) -> Result<ReadWindow, AppError> {
        wait_for_read_with_cancellable(file, offset, max_bytes, None)
    }

    fn wait_for_read_with_cancellable(
        file: &gio::File,
        offset: u64,
        max_bytes: usize,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<ReadWindow, AppError> {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
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
            })
            .unwrap_or(Err(AppError::Cancelled))
    }

    fn spin_until_result(
        context: &glib::MainContext,
        result: &Rc<RefCell<Option<Result<ReadWindow, AppError>>>>,
    ) -> Result<ReadWindow, AppError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        while result.borrow().is_none() && Instant::now() < deadline {
            while context.iteration(false) {}
            if result.borrow().is_none() {
                let _dispatched = context.iteration(true);
            }
        }
        match result.borrow_mut().take() {
            Some(value) => value,
            None => Err(AppError::Cancelled),
        }
    }
}
