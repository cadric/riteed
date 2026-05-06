use gettextrs::ngettext;
use gtk4::prelude::*;

use crate::editor_tab::EditorTab;

pub(super) fn count_matches(
    context: &sourceview5::SearchContext,
    buffer: &sourceview5::Buffer,
) -> u32 {
    let mut count = 0;
    let mut iter = buffer.start_iter();
    while let Some((_start, end, wrapped)) = context.forward(&iter) {
        if wrapped {
            break;
        }
        count += 1;
        iter = end;
    }
    count
}

pub(super) fn select_match(tab: &EditorTab, start: &gtk4::TextIter, end: &gtk4::TextIter) {
    let buffer = tab.text_buffer();
    buffer.select_range(start, end);
    let mut scroll_iter = *start;
    tab.text_view()
        .scroll_to_iter(&mut scroll_iter, 0.2, false, 0.0, 0.0);
}

pub(super) fn selection_matches_query(selection: &str, query: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        selection == query
    } else {
        selection.to_lowercase() == query.to_lowercase()
    }
}

#[must_use]
pub fn format_match_count(count: u32) -> String {
    ngettext("%d match", "%d matches", count).replace("%d", &count.to_string())
}

#[must_use]
pub fn format_replaced_count(count: u32) -> String {
    ngettext("Replaced %d match", "Replaced %d matches", count).replace("%d", &count.to_string())
}

#[cfg(test)]
mod tests {
    use super::{format_match_count, format_replaced_count, selection_matches_query};

    #[test]
    fn match_count_is_plural_sensitive() {
        assert_eq!(format_match_count(1), "1 match");
        assert_eq!(format_match_count(2), "2 matches");
    }

    #[test]
    fn replaced_count_is_plural_sensitive() {
        assert_eq!(format_replaced_count(1), "Replaced 1 match");
        assert_eq!(format_replaced_count(2), "Replaced 2 matches");
    }

    #[test]
    fn case_insensitive_selection_match_works() {
        assert!(selection_matches_query("Hello", "hello", false));
        assert!(!selection_matches_query("Hello", "hello", true));
    }
}
