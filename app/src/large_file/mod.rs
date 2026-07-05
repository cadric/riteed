use std::path::PathBuf;

use gtk4::gio;
use gtk4::prelude::FileExt;

pub(crate) mod page_text;
pub(crate) mod reader;
pub(crate) mod search;
pub(crate) mod viewer;
pub(crate) mod viewer_status;

#[cfg(test)]
mod viewer_tests;

#[must_use]
pub(crate) fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).map_or(u64::MAX, |converted| converted)
}

#[must_use]
pub(crate) fn file_path_for_error(file: &gio::File) -> PathBuf {
    file.path()
        .unwrap_or_else(|| PathBuf::from(file.uri().to_string()))
}
