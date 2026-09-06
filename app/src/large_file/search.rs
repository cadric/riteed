use std::cell::RefCell;
use std::rc::Rc;

use gtk4::{gio, prelude::*};

use crate::document_limits::{VIEWER_PAGE_BYTES, VIEWER_SEARCH_MATCH_LIMIT};
use crate::error::AppError;
use crate::large_file::{reader, usize_to_u64};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchOutcome {
    pub(crate) matches: Vec<u64>,
    pub(crate) reached_cap: bool,
    pub(crate) scanned_bytes: u64,
}

pub(crate) type SearchCallback = Rc<dyn Fn(Result<SearchOutcome, AppError>)>;

#[derive(Default)]
struct SearchState {
    carry: Vec<u8>,
    matches: Vec<u64>,
}

enum SearchStep {
    Complete(SearchOutcome),
    Continue(u64),
}

// PARSER-BOUNDARY: id=large_file_streaming_search
pub(crate) fn search_file(
    file: &gio::File,
    needle: &str,
    cancellable: Option<&gio::Cancellable>,
    callback: SearchCallback,
) {
    let query = Rc::new(needle.as_bytes().to_vec());
    if query.is_empty() {
        callback(Ok(SearchOutcome {
            matches: Vec::new(),
            reached_cap: false,
            scanned_bytes: 0,
        }));
        return;
    }
    let cancellable = cancellable.cloned();
    let cancellable_for_open = cancellable.clone();
    reader::open_stream(
        file,
        cancellable_for_open.as_ref(),
        Rc::new(move |result| match result {
            Ok(opened) => search_next(
                &opened,
                query.clone(),
                cancellable.clone(),
                0,
                Rc::new(RefCell::new(SearchState::default())),
                callback.clone(),
            ),
            Err(error) => callback(Err(error)),
        }),
    );
}

fn search_next(
    opened: &reader::OpenedStream,
    needle: Rc<Vec<u8>>,
    cancellable: Option<gio::Cancellable>,
    offset: u64,
    state: Rc<RefCell<SearchState>>,
    callback: SearchCallback,
) {
    if cancellable
        .as_ref()
        .is_some_and(gio::Cancellable::is_cancelled)
    {
        callback(Err(AppError::Cancelled));
        return;
    }

    let cancellable_for_read = cancellable.clone();
    let opened_for_callback = opened.clone();
    reader::read_open_stream_window(
        opened,
        offset,
        VIEWER_PAGE_BYTES,
        cancellable_for_read.as_ref(),
        Rc::new(move |result| match result {
            Ok(window) => {
                let scanned_bytes = window
                    .offset
                    .saturating_add(usize_to_u64(window.bytes.len()));
                let step = {
                    let mut state = state.borrow_mut();
                    let carry_len = state.carry.len();
                    let base_offset = window.offset.saturating_sub(usize_to_u64(carry_len));
                    let mut combined = std::mem::take(&mut state.carry);
                    combined.extend_from_slice(&window.bytes);

                    append_visible_matches(
                        &combined,
                        &needle,
                        carry_len,
                        base_offset,
                        &mut state.matches,
                    );
                    let reached_cap = state.matches.len() >= VIEWER_SEARCH_MATCH_LIMIT;
                    if window.eof || reached_cap {
                        SearchStep::Complete(SearchOutcome {
                            matches: std::mem::take(&mut state.matches),
                            reached_cap,
                            scanned_bytes,
                        })
                    } else {
                        state.carry = suffix_for_cross_chunk_matches(&combined, needle.len());
                        SearchStep::Continue(scanned_bytes)
                    }
                };

                match step {
                    SearchStep::Complete(outcome) => callback(Ok(outcome)),
                    SearchStep::Continue(next_offset) => search_next(
                        &opened_for_callback,
                        needle.clone(),
                        cancellable.clone(),
                        next_offset,
                        state.clone(),
                        callback.clone(),
                    ),
                }
            }
            Err(error) => callback(Err(error)),
        }),
    );
}

fn append_visible_matches(
    haystack: &[u8],
    needle: &[u8],
    current_start: usize,
    base_offset: u64,
    matches: &mut Vec<u64>,
) {
    if needle.is_empty() {
        return;
    }
    let mut index: usize = 0;
    while index.saturating_add(needle.len()) <= haystack.len()
        && matches.len() < VIEWER_SEARCH_MATCH_LIMIT
    {
        if &haystack[index..index + needle.len()] == needle
            && index.saturating_add(needle.len()) > current_start
        {
            matches.push(base_offset.saturating_add(usize_to_u64(index)));
        }
        index = index.saturating_add(1);
    }
}

fn suffix_for_cross_chunk_matches(bytes: &[u8], needle_len: usize) -> Vec<u8> {
    let keep = needle_len.saturating_sub(1).min(bytes.len());
    bytes[bytes.len().saturating_sub(keep)..].to_vec()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    use gtk4::{gio, glib, prelude::*};

    use super::{
        SearchOutcome, append_visible_matches, search_file, suffix_for_cross_chunk_matches,
    };
    use crate::document_limits::VIEWER_SEARCH_MATCH_LIMIT;
    use crate::error::AppError;

    #[test]
    fn search_records_cross_chunk_match_once() {
        let mut matches = Vec::new();
        append_visible_matches(b"abcde", b"cd", 3, 10, &mut matches);
        assert_eq!(matches, vec![12]);
    }

    #[test]
    fn suffix_is_bounded_by_query_length() {
        assert_eq!(suffix_for_cross_chunk_matches(b"abcdef", 3), b"ef");
        assert_eq!(suffix_for_cross_chunk_matches(b"abcdef", 1), b"");
    }

    #[test]
    fn search_uses_retained_stream_for_sequential_scan() {
        let source = include_str!("search.rs");
        let random_window_reader = ["reader::read", "_window("].concat();

        assert!(source.contains("reader::open_stream("));
        assert!(source.contains("reader::read_open_stream_window("));
        assert!(!source.contains(&random_window_reader));
    }

    #[test]
    fn empty_query_finishes_without_io() {
        let (path, file) = temp_file(SearchTempFile::EmptyQuery, b"ignored");
        let result = wait_for_search(&file, "");

        assert!(result.is_ok());
        let Ok(outcome) = result else {
            return;
        };
        assert_eq!(
            outcome,
            SearchOutcome {
                matches: Vec::new(),
                reached_cap: false,
                scanned_bytes: 0,
            }
        );
        let _removed = fs::remove_file(path);
    }

    #[test]
    fn search_file_finds_offsets_in_window() {
        let (path, file) = temp_file(SearchTempFile::Matches, b"one two one");
        let result = wait_for_search(&file, "one");

        assert!(result.is_ok());
        let Ok(outcome) = result else {
            return;
        };
        assert_eq!(outcome.matches, vec![0, 8]);
        assert!(!outcome.reached_cap);
        assert_eq!(outcome.scanned_bytes, 11);
        let _removed = fs::remove_file(path);
    }

    #[test]
    fn search_file_honors_cancelled_request_before_io() {
        let (path, file) = temp_file(SearchTempFile::Cancelled, b"needle");
        let cancellable = gio::Cancellable::new();
        cancellable.cancel();
        let result = wait_for_search_with_cancellable(&file, "needle", Some(&cancellable));

        assert!(matches!(result, Err(AppError::Cancelled)));
        let _removed = fs::remove_file(path);
    }

    #[test]
    fn search_file_stops_at_match_cap() {
        let contents = vec![b'a'; VIEWER_SEARCH_MATCH_LIMIT.saturating_add(32)];
        let (path, file) = temp_file(SearchTempFile::Cap, &contents);
        let result = wait_for_search(&file, "a");

        assert!(result.is_ok());
        let Ok(outcome) = result else {
            return;
        };
        assert_eq!(outcome.matches.len(), VIEWER_SEARCH_MATCH_LIMIT);
        assert!(outcome.reached_cap);
        let _removed = fs::remove_file(path);
    }

    #[derive(Clone, Copy)]
    enum SearchTempFile {
        EmptyQuery,
        Matches,
        Cancelled,
        Cap,
    }

    impl SearchTempFile {
        fn name(self) -> &'static str {
            match self {
                Self::EmptyQuery => "empty-query",
                Self::Matches => "matches",
                Self::Cancelled => "cancelled",
                Self::Cap => "cap",
            }
        }
    }

    fn temp_file(fixture: SearchTempFile, contents: &[u8]) -> (PathBuf, gio::File) {
        let path = PathBuf::from("/tmp").join(format!(
            "riteed-large-file-search-{}-{}.txt",
            std::process::id(),
            fixture.name()
        ));
        assert!(fs::write(&path, contents).is_ok());
        let file = gio::File::for_path(&path);
        (path, file)
    }

    fn wait_for_search(file: &gio::File, needle: &str) -> Result<SearchOutcome, AppError> {
        wait_for_search_with_cancellable(file, needle, None)
    }

    fn wait_for_search_with_cancellable(
        file: &gio::File,
        needle: &str,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<SearchOutcome, AppError> {
        let context = glib::MainContext::new();
        context
            .with_thread_default(|| {
                let result = Rc::new(RefCell::new(None));
                let result_for_callback = Rc::clone(&result);
                search_file(
                    file,
                    needle,
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
        result: &Rc<RefCell<Option<Result<SearchOutcome, AppError>>>>,
    ) -> Result<SearchOutcome, AppError> {
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
