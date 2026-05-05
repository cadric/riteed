use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

pub(crate) const OPEN_FILE_LIMIT_BYTES: u64 = 25 * 1024 * 1024;
pub(crate) const SEARCH_CHAR_LIMIT: i32 = 2_000_000;
const SIZE_QUERY_ATTRIBUTES: &str = "standard::type,standard::size";

#[must_use]
pub(crate) fn buffer_supports_search(buffer: &sourceview5::Buffer) -> bool {
    char_count_supports_search(buffer.char_count())
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
        OPEN_FILE_LIMIT_BYTES, SEARCH_CHAR_LIMIT, char_count_supports_search,
        file_size_supports_open,
    };

    #[test]
    fn search_limit_is_inclusive() {
        assert!(char_count_supports_search(SEARCH_CHAR_LIMIT));
        assert!(!char_count_supports_search(SEARCH_CHAR_LIMIT + 1));
    }

    #[test]
    fn open_limit_is_inclusive() {
        assert!(file_size_supports_open(OPEN_FILE_LIMIT_BYTES));
        assert!(!file_size_supports_open(OPEN_FILE_LIMIT_BYTES + 1));
    }
}
