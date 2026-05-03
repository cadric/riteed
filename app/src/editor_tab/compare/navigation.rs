use gtk4::prelude::*;

use super::model::DiffRowModel;

pub(super) fn top_visible_row(view: &sourceview5::View, row_count: usize) -> usize {
    if row_count == 0 {
        return 0;
    }
    let y = view.visible_rect().y();
    let (iter, _line_top) = view.line_at_y(y);
    usize::try_from(iter.line())
        .map_or(0, |row| row)
        .min(row_count.saturating_sub(1))
}

pub(super) fn target_hunk_for_navigation(
    model: &DiffRowModel,
    top_visible_row: usize,
    direction: i32,
) -> Option<usize> {
    if model.hunks.is_empty() || model.too_large {
        return None;
    }
    if direction < 0 {
        return model
            .hunks
            .iter()
            .rposition(|hunk| hunk.first_row < top_visible_row)
            .or_else(|| model.hunks.len().checked_sub(1));
    }
    model
        .hunks
        .iter()
        .position(|hunk| hunk.first_row > top_visible_row)
        .or(Some(0))
}

#[cfg(test)]
mod tests {
    use super::target_hunk_for_navigation;
    use crate::editor_tab::compare::diff::compute_diff_row_model;

    #[test]
    fn next_hunk_uses_strict_visible_row() {
        let model = compute_diff_row_model("a\nb\nc\nd\ne\nf\n", "x\nb\nc\nd\ne\ny\n");

        assert_eq!(target_hunk_for_navigation(&model, 0, 1), Some(1));
        assert_eq!(target_hunk_for_navigation(&model, 5, 1), Some(0));
    }

    #[test]
    fn previous_hunk_uses_strict_visible_row() {
        let model = compute_diff_row_model("a\nb\nc\nd\ne\nf\n", "x\nb\nc\nd\ne\ny\n");

        assert_eq!(target_hunk_for_navigation(&model, 5, -1), Some(0));
        assert_eq!(target_hunk_for_navigation(&model, 0, -1), Some(1));
    }
}
