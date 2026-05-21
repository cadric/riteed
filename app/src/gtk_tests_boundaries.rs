use gtk4::{gio, prelude::*};
use libadwaita as adw;

// The real GTK boundary smoke is called from gtk_surfaces_and_editor_flow_work.
// Keeping it in the single GTK integration test avoids cross-thread GTK
// initialization failures from multiple independent Rust #[test] entry points.
use crate::document_limits::{OPEN_FILE_LIMIT_BYTES, SEARCH_CHAR_LIMIT};
use crate::gtk_tests::{build_window, drain_events, spin_until, write_temp_file};
use crate::workspace::OpenSource;

const OPEN_SEED: &[u8] = include_bytes!("../../stress/corpus/seeds/open-boundary.txt");
const SEARCH_SEED: &str = include_str!("../../stress/corpus/seeds/search-boundary.txt");

pub(crate) fn exercise_boundary_smokes(test_app: &adw::Application) {
    exercise_open_file_at_cap(test_app);
    exercise_search_at_cap(test_app);
}

#[test]
fn gtk_tests_boundaries_seed_corpus_is_small() {
    assert!(OPEN_SEED.len().saturating_add(SEARCH_SEED.len()) < 100_000);
}

fn exercise_open_file_at_cap(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    let cap = usize::try_from(OPEN_FILE_LIMIT_BYTES).unwrap_or(0);
    let path = write_temp_file("riteed-open-boundary-cap.txt", &repeat_seed(OPEN_SEED, cap));
    let file = gio::File::for_path(&path);
    let uri = file.uri().to_string();

    window.request_open_files(vec![file], OpenSource::AppOpen);
    spin_until("open file exactly at byte cap", || {
        window.selected_saved_uri_for_tests() == uri
    });

    let _removed = std::fs::remove_file(path);
}

fn exercise_search_at_cap(test_app: &adw::Application) {
    let Some(window) = build_window(test_app) else {
        return;
    };
    window.ensure_default_tab();
    let query = SEARCH_SEED.trim();
    let search_limit = usize::try_from(SEARCH_CHAR_LIMIT).unwrap_or(0);
    let filler_len = search_limit.saturating_sub(query.len());
    let text = format!("{query}{}", "x".repeat(filler_len));
    window.set_selected_text_for_tests(&text);
    window.select_offsets_for_tests(0, i32::try_from(query.len()).unwrap_or(0));
    window.open_search(false);

    drain_events(12);
    assert!(window.search_visible_for_tests());
    assert_ne!(
        window.search_result_for_tests(),
        "Search is disabled for very large files."
    );
}

fn repeat_seed(seed: &[u8], target_len: usize) -> Vec<u8> {
    if seed.is_empty() {
        return vec![b'x'; target_len];
    }
    let mut contents = Vec::with_capacity(target_len);
    while contents.len().saturating_add(seed.len()) <= target_len {
        contents.extend_from_slice(seed);
    }
    let remaining = target_len.saturating_sub(contents.len());
    contents.extend_from_slice(&seed[..remaining]);
    contents
}
