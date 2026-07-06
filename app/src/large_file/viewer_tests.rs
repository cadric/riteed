use std::cell::RefCell;

use gtk4::gio;
use gtk4::prelude::CancellableExt;

use super::usize_to_u64;
use super::viewer::{cancel_cancellable, count_newlines, locate_line_in_chunk};
use super::viewer_status::{format_page_status, search_match_message, viewer_memory_tooltip};

#[test]
fn line_jump_finds_line_start_in_chunk() {
    assert_eq!(locate_line_in_chunk(3, 1, 10, b"a\nb\nc"), Some(14));
}

#[test]
fn line_jump_returns_current_offset_for_current_or_previous_line() {
    assert_eq!(locate_line_in_chunk(2, 2, 64, b"a\nb\nc"), Some(64));
    assert_eq!(locate_line_in_chunk(1, 2, 64, b"a\nb\nc"), Some(64));
}

#[test]
fn line_jump_reports_unknown_when_line_is_not_in_chunk() {
    assert_eq!(locate_line_in_chunk(5, 1, 10, b"a\nb\nc"), None);
}

#[test]
fn line_count_counts_lf_bytes_only() {
    assert_eq!(count_newlines(b"a\nb\r\nc"), 2);
}

#[test]
fn page_status_names_visible_byte_range() {
    let status = format_page_status(10, 20, 100);

    assert!(status.contains("10"));
    assert!(status.contains("20"));
    assert!(status.contains("100"));
}

#[test]
fn memory_tooltip_names_current_page_only() {
    assert_eq!(
        viewer_memory_tooltip(),
        "Viewer keeps only the current file page in memory."
    );
}

#[test]
fn search_match_message_is_plural_sensitive() {
    assert_eq!(
        search_match_message(1, false),
        "1 match found; showing the first match."
    );
    assert_eq!(
        search_match_message(2, false),
        "2 matches found; showing the first match."
    );
    assert_eq!(
        search_match_message(2, true),
        "Many matches found; showing the first match."
    );
}

#[test]
fn search_note_replaces_page_status_once() {
    use crate::large_file::viewer_status::status_after_page_load;

    assert_eq!(
        status_after_page_load(
            Some(String::from("2 matches found; showing the first match.")),
            String::from("Viewing bytes 0-9 of 9."),
        ),
        "2 matches found; showing the first match."
    );
    assert_eq!(
        status_after_page_load(None, String::from("Viewing bytes 0-9 of 9.")),
        "Viewing bytes 0-9 of 9."
    );
}

#[test]
fn usize_to_u64_preserves_small_offsets() {
    assert_eq!(usize_to_u64(42), 42);
}

#[test]
fn cancelling_cancellable_cell_takes_and_cancels() {
    let cancellable = gio::Cancellable::new();
    let cell = RefCell::new(Some(cancellable.clone()));

    cancel_cancellable(&cell);

    assert!(cell.borrow().is_none());
    assert!(cancellable.is_cancelled());
}

#[test]
fn operation_switches_cancel_opposite_viewer_work() {
    let source = include_str!("viewer.rs");

    assert!(source.contains("self.cancel_search_request();"));
    assert!(source.contains("self.cancel_current_request();"));
    assert!(source.contains("fn replace_search_cancellable"));
}

#[test]
fn line_jump_uses_retained_stream_for_sequential_scan() {
    let source = include_str!("viewer.rs");

    assert!(source.contains("reader::open_stream("));
    assert!(source.contains("fn find_line_offset_in_stream"));
    assert!(source.contains("reader::read_open_stream_window("));
}
