use std::path::Path;

use gtk4::{gio, prelude::*};

pub(crate) const OPEN_FILE_LIMIT_BYTES: u64 = 25 * 1024 * 1024;
pub(crate) const SEARCH_CHAR_LIMIT: i32 = 2_000_000;
pub(crate) const SESSION_RESTORE_FILE_LIMIT_BYTES: u64 = 25 * 1024 * 1024;

#[must_use]
pub(crate) fn buffer_supports_search(buffer: &sourceview5::Buffer) -> bool {
    char_count_supports_search(buffer.char_count())
}

#[must_use]
pub(crate) fn path_supports_open(path: &Path) -> bool {
    std::fs::metadata(path).map_or(true, |metadata| {
        metadata.is_file() && file_size_supports_open(metadata.len())
    })
}

#[must_use]
pub(crate) fn file_supports_session_restore(file: &gio::File) -> bool {
    file.path()
        .as_deref()
        .is_some_and(path_supports_session_restore)
}

#[must_use]
pub(crate) fn uri_supports_session_restore(uri: &str) -> bool {
    file_supports_session_restore(&gio::File::for_uri(uri))
}

#[must_use]
fn char_count_supports_search(char_count: i32) -> bool {
    char_count <= SEARCH_CHAR_LIMIT
}

fn path_supports_session_restore(path: &Path) -> bool {
    std::fs::metadata(path).map_or(true, |metadata| {
        metadata.is_file() && file_size_supports_session_restore(metadata.len())
    })
}

fn file_size_supports_open(size: u64) -> bool {
    size <= OPEN_FILE_LIMIT_BYTES
}

#[must_use]
fn file_size_supports_session_restore(size: u64) -> bool {
    size <= SESSION_RESTORE_FILE_LIMIT_BYTES
}

#[cfg(test)]
mod tests {
    use super::{
        OPEN_FILE_LIMIT_BYTES, SEARCH_CHAR_LIMIT, SESSION_RESTORE_FILE_LIMIT_BYTES,
        char_count_supports_search, file_size_supports_open, file_size_supports_session_restore,
    };

    #[test]
    fn search_limit_is_inclusive() {
        assert!(char_count_supports_search(SEARCH_CHAR_LIMIT));
        assert!(!char_count_supports_search(SEARCH_CHAR_LIMIT + 1));
    }

    #[test]
    fn session_restore_limit_is_inclusive() {
        assert!(file_size_supports_session_restore(
            SESSION_RESTORE_FILE_LIMIT_BYTES
        ));
        assert!(!file_size_supports_session_restore(
            SESSION_RESTORE_FILE_LIMIT_BYTES + 1
        ));
    }

    #[test]
    fn open_limit_is_inclusive() {
        assert!(file_size_supports_open(OPEN_FILE_LIMIT_BYTES));
        assert!(!file_size_supports_open(OPEN_FILE_LIMIT_BYTES + 1));
    }
}
