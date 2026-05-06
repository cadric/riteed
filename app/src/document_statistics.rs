use gettextrs::{gettext, pgettext};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::document_limits::SEARCH_CHAR_LIMIT;
use crate::editor_tab::EditorTab;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextStatistics {
    lines: u32,
    words: u32,
    characters: u32,
    characters_without_spaces: u32,
}

pub(crate) fn present(parent: &adw::ApplicationWindow, tab: &EditorTab) {
    let buffer = tab.text_buffer();
    if buffer.char_count() > SEARCH_CHAR_LIMIT {
        present_too_large(parent);
        return;
    }

    let document = collect_document_statistics(&buffer);
    let selection = collect_selection_statistics(&buffer);
    let shell =
        crate::dialog_shell::build_dialog_shell(&gettext("Document Statistics"), 420, None, true);
    shell.content.append(&statistics_group(
        &pgettext("statistics scope", "Document"),
        &document,
    ));
    if let Some(selection) = selection {
        shell.content.append(&statistics_group(
            &pgettext("statistics scope", "Selection"),
            &selection,
        ));
    }
    shell.dialog.present(Some(parent));
}

fn present_too_large(parent: &adw::ApplicationWindow) {
    let shell =
        crate::dialog_shell::build_dialog_shell(&gettext("Document Statistics"), 420, None, true);
    let label = gtk4::Label::new(Some(&gettext(
        "Statistics are disabled for very large files.",
    )));
    label.set_wrap(true);
    label.set_xalign(0.0);
    shell.content.append(&label);
    shell.dialog.present(Some(parent));
}

fn collect_document_statistics(buffer: &sourceview5::Buffer) -> TextStatistics {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    let text = buffer.text(&start, &end, true);
    count_text(&text, buffer.line_count().cast_unsigned())
}

fn collect_selection_statistics(buffer: &sourceview5::Buffer) -> Option<TextStatistics> {
    let (start, end) = buffer.selection_bounds()?;
    if start.offset() == end.offset() {
        return None;
    }
    let text = buffer.text(&start, &end, true);
    Some(count_text(&text, selected_line_count(&start, &end)))
}

fn statistics_group(title: &str, statistics: &TextStatistics) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::new();
    group.set_title(title);
    group.add(&statistics_row(
        &pgettext("statistics row", "Lines"),
        statistics.lines,
        None,
    ));
    group.add(&statistics_row(
        &pgettext("statistics row", "Words (whitespace-delimited)"),
        statistics.words,
        Some(&gettext(
            "Words are separated by whitespace; em dashes and punctuation count with adjacent words.",
        )),
    ));
    group.add(&statistics_row(
        &pgettext("statistics row", "Characters"),
        statistics.characters,
        None,
    ));
    group.add(&statistics_row(
        &pgettext("statistics row", "Characters Without Spaces"),
        statistics.characters_without_spaces,
        None,
    ));
    group
}

fn statistics_row(title: &str, value: u32, tooltip: Option<&str>) -> adw::ActionRow {
    let row = adw::ActionRow::new();
    row.set_title(title);
    row.set_subtitle(&value.to_string());
    if let Some(tooltip) = tooltip {
        row.set_tooltip_text(Some(tooltip));
    }
    row
}

fn count_text(text: &str, lines: u32) -> TextStatistics {
    let characters = count_u32(text.chars());
    let characters_without_spaces =
        count_u32(text.chars().filter(|character| !character.is_whitespace()));
    let words = count_u32(text.split_whitespace());
    TextStatistics {
        lines,
        words,
        characters,
        characters_without_spaces,
    }
}

fn selected_line_count(start: &gtk4::TextIter, end: &gtk4::TextIter) -> u32 {
    line_span_count(start.line(), end.line())
}

fn line_span_count(start_line: i32, end_line: i32) -> u32 {
    let lines = end_line.saturating_sub(start_line).saturating_add(1);
    lines.cast_unsigned()
}

fn count_u32<I>(iter: I) -> u32
where
    I: Iterator,
{
    u32::try_from(iter.count()).map_or(u32::MAX, |value| value)
}

#[cfg(test)]
mod tests {
    use super::{TextStatistics, count_text, line_span_count};

    #[test]
    fn text_statistics_count_words_and_spaces() {
        assert_eq!(
            count_text("one two\nthree", 2),
            TextStatistics {
                lines: 2,
                words: 3,
                characters: 13,
                characters_without_spaces: 11,
            }
        );
    }

    #[test]
    fn selection_lines_are_inclusive() {
        assert_eq!(line_span_count(0, 1), 2);
    }
}
