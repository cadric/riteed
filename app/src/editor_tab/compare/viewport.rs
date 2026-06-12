use gtk4::prelude::*;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::editor_tab) struct CompareViewportState {
    cursor_line: Option<i32>,
    top_line: Option<i32>,
}

pub(in crate::editor_tab) fn capture(view: &sourceview5::View) -> CompareViewportState {
    let buffer = view.buffer();
    let cursor = buffer.iter_at_mark(&buffer.get_insert());
    let (top, _line_top) = view.line_at_y(view.visible_rect().y());
    CompareViewportState {
        cursor_line: Some(cursor.line()),
        top_line: Some(top.line()),
    }
}

pub(in crate::editor_tab) fn restore(state: &CompareViewportState, view: &sourceview5::View) {
    restore_with_cursor_line(
        state,
        view,
        state
            .cursor_line
            .and_then(|line| usize::try_from(line).ok()),
    );
}

pub(in crate::editor_tab) fn restore_with_cursor_line(
    state: &CompareViewportState,
    view: &sourceview5::View,
    cursor_line: Option<usize>,
) {
    let buffer = view.buffer();
    if let Some(cursor) = cursor_line
        .and_then(|line| i32::try_from(line).ok())
        .and_then(|line| iter_at_line_clamped(&buffer, line))
    {
        buffer.place_cursor(&cursor);
    }
    if view.allocated_width() <= 0 || view.allocated_height() <= 0 {
        return;
    }
    if let Some(mut top) = state
        .top_line
        .and_then(|line| iter_at_line_clamped(&buffer, line))
    {
        view.scroll_to_iter(&mut top, 0.0, true, 0.0, 0.0);
    }
}

fn iter_at_line_clamped(buffer: &gtk4::TextBuffer, line: i32) -> Option<gtk4::TextIter> {
    let max_line = buffer.line_count().saturating_sub(1);
    buffer.iter_at_line(line.clamp(0, max_line))
}
