use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

const ENUMERATE_ATTRIBUTES: &str =
    "standard::name,standard::display-name,standard::type,standard::size,standard::content-type";
const ENUMERATE_BATCH_SIZE: i32 = 200;
const MAX_VISITED_PATHS: u32 = 25_000;
const MAX_MATCHES: u32 = 500;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct FindMatch {
    pub(crate) file: gio::File,
    pub(crate) path: String,
    pub(crate) line_number: u32,
    pub(crate) line_text: String,
    pub(crate) start_offset: i32,
    pub(crate) end_offset: i32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ScanSummary {
    pub(crate) visited: u32,
    pub(crate) files_searched: u32,
    pub(crate) matches: u32,
    pub(crate) skipped: u32,
    pub(crate) limited: bool,
}

pub(crate) struct ScanRequest {
    pub(crate) generation: u64,
    pub(crate) root: gio::File,
    pub(crate) query: String,
    pub(crate) match_case: bool,
    pub(crate) show_hidden: bool,
    pub(crate) cancellable: gio::Cancellable,
}

pub(crate) struct ScanSink {
    pub(crate) result: Rc<dyn Fn(u64, FindMatch)>,
    pub(crate) finish: Rc<dyn Fn(u64, ScanSummary)>,
}

#[derive(Clone)]
struct FileCandidate {
    file: gio::File,
    path: String,
}

struct ScanState {
    request: ScanRequest,
    sink: ScanSink,
    pending_dirs: VecDeque<gio::File>,
    pending_files: VecDeque<FileCandidate>,
    summary: ScanSummary,
    query_folded: Vec<char>,
}

pub(crate) fn start_scan(request: ScanRequest, sink: ScanSink) {
    let query_folded = folded_chars(&request.query);
    let state = Rc::new(RefCell::new(ScanState {
        pending_dirs: VecDeque::from([request.root.clone()]),
        pending_files: VecDeque::new(),
        summary: ScanSummary::default(),
        request,
        sink,
        query_folded,
    }));
    scan_next(state);
}

fn scan_next(state: Rc<RefCell<ScanState>>) {
    if should_finish(&state) {
        finish_scan(&state);
        return;
    }

    let next_file = { state.borrow_mut().pending_files.pop_front() };
    if let Some(file) = next_file {
        read_file(state, file);
        return;
    }

    let next_directory = { state.borrow_mut().pending_dirs.pop_front() };
    if let Some(directory) = next_directory {
        enumerate_directory(state, directory);
        return;
    }

    finish_scan(&state);
}

fn enumerate_directory(state: Rc<RefCell<ScanState>>, directory: gio::File) {
    let cancellable = state.borrow().request.cancellable.clone();
    let call_target = directory.clone();
    call_target.enumerate_children_async(
        ENUMERATE_ATTRIBUTES,
        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
        glib::Priority::default(),
        Some(&cancellable),
        move |result| {
            if stale_or_cancelled(&state) {
                return;
            }
            match result {
                Ok(enumerator) => collect_directory(state, directory, enumerator, Vec::new()),
                Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {}
                Err(_error) => {
                    state.borrow_mut().summary.skipped += 1;
                    scan_next(state);
                }
            }
        },
    );
}

fn collect_directory(
    state: Rc<RefCell<ScanState>>,
    directory: gio::File,
    enumerator: gio::FileEnumerator,
    mut collected: Vec<gio::FileInfo>,
) {
    let cancellable = state.borrow().request.cancellable.clone();
    let call_target = enumerator.clone();
    call_target.next_files_async(
        ENUMERATE_BATCH_SIZE,
        glib::Priority::default(),
        Some(&cancellable),
        move |result| {
            if stale_or_cancelled(&state) {
                return;
            }
            match result {
                Ok(batch) if batch.is_empty() => {
                    queue_directory_entries(&state, &directory, &collected);
                    scan_next(state);
                }
                Ok(batch) => {
                    collected.extend(batch);
                    collect_directory(state, directory, enumerator, collected);
                }
                Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {}
                Err(_error) => {
                    state.borrow_mut().summary.skipped += 1;
                    scan_next(state);
                }
            }
        },
    );
}

fn queue_directory_entries(
    state: &Rc<RefCell<ScanState>>,
    directory: &gio::File,
    infos: &[gio::FileInfo],
) {
    let mut state = state.borrow_mut();
    for info in infos {
        if state.summary.visited_limit_reached() {
            state.summary.limited = true;
            return;
        }
        state.summary.visited += 1;
        let name = info.name().to_string_lossy().to_string();
        let is_directory = info.file_type() == gio::FileType::Directory;
        if should_skip_entry(&name, is_directory, state.show_hidden()) {
            state.summary.skipped += 1;
            continue;
        }
        let child = directory.child(info.name());
        match info.file_type() {
            gio::FileType::Directory => state.pending_dirs.push_back(child),
            gio::FileType::Regular => queue_file_candidate(&mut state, child, info),
            _ => state.summary.skipped += 1,
        }
    }
}

fn queue_file_candidate(state: &mut ScanState, file: gio::File, info: &gio::FileInfo) {
    if u64::try_from(info.size()).map_or(true, |size| size > MAX_FILE_BYTES) {
        state.summary.skipped += 1;
        return;
    }
    if info
        .content_type()
        .is_some_and(|kind| clearly_non_text(&kind))
    {
        state.summary.skipped += 1;
        return;
    }
    let path = state
        .request
        .root
        .relative_path(&file)
        .map_or_else(|| file.basename(), Some)
        .map_or_else(
            || file.uri().to_string(),
            |path| path.to_string_lossy().to_string(),
        );
    state.pending_files.push_back(FileCandidate { file, path });
}

fn read_file(state: Rc<RefCell<ScanState>>, candidate: FileCandidate) {
    let cancellable = state.borrow().request.cancellable.clone();
    let call_target = candidate.file.clone();
    call_target.load_contents_async(Some(&cancellable), move |result| {
        if stale_or_cancelled(&state) {
            return;
        }
        match result {
            Ok((contents, _etag)) => match std::str::from_utf8(contents.as_ref()) {
                Ok(text) => collect_matches(&state, &candidate, text),
                Err(_error) => state.borrow_mut().summary.skipped += 1,
            },
            Err(error) if error.matches(gio::IOErrorEnum::Cancelled) => {}
            Err(_error) => state.borrow_mut().summary.skipped += 1,
        }
        scan_next(state);
    });
}

fn collect_matches(state: &Rc<RefCell<ScanState>>, candidate: &FileCandidate, text: &str) {
    state.borrow_mut().summary.files_searched += 1;
    let mut line_start = 0_i32;
    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        let visible_line = line.trim_end_matches(['\r', '\n']);
        let ranges = match_ranges(&state.borrow(), visible_line);
        for (start, end) in ranges {
            if state.borrow().summary.matches >= MAX_MATCHES {
                state.borrow_mut().summary.limited = true;
                return;
            }
            let line_number = u32::try_from(line_index + 1).map_or(u32::MAX, |value| value);
            let result = FindMatch {
                file: candidate.file.clone(),
                path: candidate.path.clone(),
                line_number,
                line_text: visible_line.to_string(),
                // Results store GtkTextBuffer character offsets, not byte offsets, so activation
                // can reopen the file and select the same range with iter_at_offset().
                start_offset: line_start.saturating_add(start),
                end_offset: line_start.saturating_add(end),
            };
            let generation = state.borrow().request.generation;
            (state.borrow().sink.result)(generation, result);
            state.borrow_mut().summary.matches += 1;
        }
        line_start = line_start.saturating_add(buffer_line_advance(line));
    }
}

fn match_ranges(state: &ScanState, line: &str) -> Vec<(i32, i32)> {
    if state.request.match_case {
        return case_sensitive_ranges(line, &state.request.query);
    }
    folded_ranges(line, &state.query_folded)
}

fn case_sensitive_ranges(line: &str, query: &str) -> Vec<(i32, i32)> {
    line.match_indices(query)
        .map(|(byte_start, matched)| {
            let start = char_count_i32(&line[..byte_start]);
            let end = start.saturating_add(char_count_i32(matched));
            (start, end)
        })
        .collect()
}

fn folded_ranges(line: &str, query: &[char]) -> Vec<(i32, i32)> {
    let mut ranges = Vec::new();
    let mut byte_start = 0_usize;
    let mut char_start = 0_usize;
    while byte_start < line.len() {
        if let Some(byte_len) = folded_match_end(&line[byte_start..], query) {
            let matched = &line[byte_start..byte_start + byte_len];
            let start = i32::try_from(char_start).map_or(i32::MAX, |value| value);
            let end = start.saturating_add(char_count_i32(matched));
            ranges.push((start, end));
            char_start = char_start.saturating_add(matched.chars().count());
            byte_start += byte_len;
            continue;
        }
        let Some(character) = line[byte_start..].chars().next() else {
            break;
        };
        byte_start += character.len_utf8();
        char_start = char_start.saturating_add(1);
    }
    ranges
}

fn folded_match_end(candidate: &str, query: &[char]) -> Option<usize> {
    if query.is_empty() {
        return None;
    }
    let mut query_index = 0;
    for (index, character) in candidate.char_indices() {
        let end = index + character.len_utf8();
        for folded in character.to_lowercase() {
            if query.get(query_index) != Some(&folded) {
                return None;
            }
            query_index += 1;
        }
        if query_index == query.len() {
            return Some(end);
        }
    }
    None
}

fn folded_chars(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

fn char_count_i32(text: &str) -> i32 {
    i32::try_from(text.chars().count()).map_or(i32::MAX, |value| value)
}

fn buffer_line_advance(line: &str) -> i32 {
    // GtkTextBuffer stores a CRLF line ending as one line break for offset selection.
    if let Some(without_lf) = line.strip_suffix('\n') {
        let without_cr = without_lf.strip_suffix('\r').unwrap_or(without_lf);
        return char_count_i32(without_cr).saturating_add(1);
    }
    char_count_i32(line)
}

fn should_finish(state: &Rc<RefCell<ScanState>>) -> bool {
    let state = state.borrow();
    state.request.cancellable.is_cancelled()
        || state.summary.matches >= MAX_MATCHES
        || state.summary.visited_limit_reached()
}

fn finish_scan(state: &Rc<RefCell<ScanState>>) {
    let mut state = state.borrow_mut();
    if state.summary.matches >= MAX_MATCHES || state.summary.visited_limit_reached() {
        state.summary.limited = true;
    }
    (state.sink.finish)(state.request.generation, state.summary.clone());
}

fn stale_or_cancelled(state: &Rc<RefCell<ScanState>>) -> bool {
    state.borrow().request.cancellable.is_cancelled()
}

fn should_skip_entry(name: &str, is_directory: bool, show_hidden: bool) -> bool {
    (is_directory && always_skipped_dir(name)) || (!show_hidden && name.starts_with('.'))
}

fn always_skipped_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "target"
            | "build"
            | "build-dir"
            | "node_modules"
            | "vendor"
            | "dist"
            | ".flatpak-builder"
            | "__pycache__"
            | ".venv"
    )
}

fn clearly_non_text(content_type: &str) -> bool {
    content_type.starts_with("image/")
        || content_type.starts_with("audio/")
        || content_type.starts_with("video/")
        || content_type.starts_with("font/")
        || matches!(
            content_type,
            "application/pdf"
                | "application/zip"
                | "application/gzip"
                | "application/x-tar"
                | "application/x-7z-compressed"
                | "application/x-executable"
                | "application/octet-stream"
        )
}

impl ScanSummary {
    fn visited_limit_reached(&self) -> bool {
        self.visited >= MAX_VISITED_PATHS
    }
}

impl ScanState {
    fn show_hidden(&self) -> bool {
        self.request.show_hidden
    }
}

#[cfg(test)]
mod tests {
    use super::{
        buffer_line_advance, case_sensitive_ranges, folded_chars, folded_ranges, should_skip_entry,
    };

    #[test]
    fn ranges_are_character_offsets_not_byte_offsets() {
        assert_eq!(case_sensitive_ranges("å alpha", "alpha"), vec![(2, 7)]);
    }

    #[test]
    fn line_advance_matches_normalized_text_buffer_newlines() {
        assert_eq!(buffer_line_advance("ab\n"), 3);
        assert_eq!(buffer_line_advance("ab\r\n"), 3);
        assert_eq!(buffer_line_advance("ab"), 2);
    }

    #[test]
    fn folded_ranges_find_case_insensitive_matches() {
        assert_eq!(
            folded_ranges("Alpha alpha", &folded_chars("alpha")),
            vec![(0, 5), (6, 11)]
        );
    }

    #[test]
    fn folded_ranges_do_not_overlap() {
        assert_eq!(
            folded_ranges("aaaa", &folded_chars("aa")),
            vec![(0, 2), (2, 4)]
        );
    }

    #[test]
    fn folded_ranges_preserve_lowercase_expansion_behavior() {
        assert!(folded_ranges("\u{0130}", &folded_chars("i")).is_empty());
        assert_eq!(
            folded_ranges("\u{0130}", &folded_chars("i\u{0307}")),
            vec![(0, 1)]
        );
    }

    #[test]
    fn skip_list_applies_to_directories_only() {
        assert!(should_skip_entry(".git", true, true));
        assert!(should_skip_entry("target", true, true));
        assert!(!should_skip_entry("target", false, true));
        assert!(!should_skip_entry("build", false, true));
        assert!(should_skip_entry(".secret", false, false));
        assert!(!should_skip_entry(".secret", false, true));
    }
}
