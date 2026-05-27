use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

pub(crate) const OPEN_FILE_LIMIT_BYTES: u64 = 25 * 1024 * 1024;
pub(crate) const SEARCH_CHAR_LIMIT: i32 = 2_000_000;
const SIZE_QUERY_ATTRIBUTES: &str = "standard::type,standard::size";

#[must_use]
pub(crate) fn buffer_supports_search(buffer: &sourceview5::Buffer) -> bool {
    char_count_supports_search(buffer.char_count())
}

#[must_use]
pub(crate) fn buffer_char_count_supports_save_snapshot(char_count: i32) -> bool {
    u64::try_from(char_count).is_ok_and(file_size_supports_open)
}

#[must_use]
pub(crate) fn text_len_supports_save_snapshot(len: usize) -> bool {
    u64::try_from(len).is_ok_and(file_size_supports_open)
}

pub(crate) fn query_file_supports_open(
    file: &gio::File,
    cancellable: Option<&gio::Cancellable>,
    callback: Rc<dyn Fn(Result<bool, glib::Error>)>,
) {
    file.query_info_async(
        SIZE_QUERY_ATTRIBUTES,
        gio::FileQueryInfoFlags::NONE,
        glib::Priority::default(),
        cancellable,
        move |result| match result {
            Ok(info) => callback(Ok(file_info_supports_open(&info))),
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => callback(Err(error)),
            Err(_error) => callback(Ok(true)),
        },
    );
}

#[must_use]
pub(crate) fn file_supports_session_restore(file: &gio::File) -> bool {
    file.path().is_some()
}

#[must_use]
pub(crate) fn uri_supports_session_restore(uri: &str) -> bool {
    file_supports_session_restore(&gio::File::for_uri(uri))
}

#[must_use]
fn char_count_supports_search(char_count: i32) -> bool {
    char_count <= SEARCH_CHAR_LIMIT
}

fn file_info_supports_open(info: &gio::FileInfo) -> bool {
    if info.file_type() != gio::FileType::Regular {
        return false;
    }
    u64::try_from(info.size()).is_ok_and(file_size_supports_open)
}

fn file_size_supports_open(size: u64) -> bool {
    size <= OPEN_FILE_LIMIT_BYTES
}

#[cfg(test)]
mod tests {
    use super::{
        OPEN_FILE_LIMIT_BYTES, SEARCH_CHAR_LIMIT, buffer_char_count_supports_save_snapshot,
        char_count_supports_search, file_size_supports_open, text_len_supports_save_snapshot,
    };

    #[test]
    fn search_at_minus_one_returns_ok() {
        assert!(char_count_supports_search(SEARCH_CHAR_LIMIT - 1));
    }

    #[test]
    fn search_at_exact_returns_ok() {
        assert!(char_count_supports_search(SEARCH_CHAR_LIMIT));
    }

    #[test]
    fn search_at_plus_one_returns_too_large() {
        assert!(!char_count_supports_search(SEARCH_CHAR_LIMIT + 1));
    }

    #[test]
    fn open_at_minus_one_returns_ok() {
        assert!(file_size_supports_open(OPEN_FILE_LIMIT_BYTES - 1));
    }

    #[test]
    fn open_at_exact_returns_ok() {
        assert!(file_size_supports_open(OPEN_FILE_LIMIT_BYTES));
    }

    #[test]
    fn open_at_plus_one_returns_too_large() {
        assert!(!file_size_supports_open(OPEN_FILE_LIMIT_BYTES + 1));
    }

    #[test]
    fn save_snapshot_char_count_uses_open_limit() {
        assert!(
            i32::try_from(OPEN_FILE_LIMIT_BYTES)
                .is_ok_and(buffer_char_count_supports_save_snapshot)
        );
        assert!(match i32::try_from(OPEN_FILE_LIMIT_BYTES + 1) {
            Ok(value) => !buffer_char_count_supports_save_snapshot(value),
            Err(_) => true,
        });
    }

    #[test]
    fn save_snapshot_text_len_uses_open_limit() {
        assert!(usize::try_from(OPEN_FILE_LIMIT_BYTES).is_ok_and(text_len_supports_save_snapshot));
        assert!(match usize::try_from(OPEN_FILE_LIMIT_BYTES + 1) {
            Ok(value) => !text_len_supports_save_snapshot(value),
            Err(_) => true,
        });
    }
}
