use gettextrs::{gettext, ngettext};

pub(super) fn format_page_status(start: u64, end: u64, size: u64) -> String {
    let template = gettext("Viewing bytes %1$s-%2$s of %3$s.");
    template
        .replace("%1$s", &start.to_string())
        .replace("%2$s", &end.to_string())
        .replace("%3$s", &size.to_string())
}

pub(super) fn viewer_memory_tooltip() -> String {
    gettext("Viewer keeps only the current file page in memory.")
}

pub(super) fn search_match_message(match_count: usize, reached_cap: bool) -> String {
    if reached_cap {
        return gettext("Many matches found; showing the first match.");
    }
    let count = u32::try_from(match_count).map_or(u32::MAX, |value| value);
    ngettext(
        "%d match found; showing the first match.",
        "%d matches found; showing the first match.",
        count,
    )
    .replace("%d", &match_count.to_string())
}

pub(super) fn status_after_page_load(note: Option<String>, page_status: String) -> String {
    note.unwrap_or(page_status)
}
