use gettextrs::{gettext, ngettext};

use super::CompareController;

impl CompareController {
    pub(super) fn update_status(&self, hidden_trim_whitespace_differences: bool) {
        let model = self.row_model.borrow();
        if model.too_large {
            self.status_label
                .set_label(&gettext("Too large to compare differences."));
            return;
        }
        let hunk_count = model.hunks.len();
        if hunk_count == 0 {
            if hidden_trim_whitespace_differences {
                self.status_label.set_label(&gettext(
                    "Only leading or trailing whitespace differences are hidden.",
                ));
                return;
            }
            self.status_label
                .set_label(&gettext("No differences were found."));
            return;
        }
        let changed_lines = model.changed_row_count();
        self.status_label.set_label(&compare_status_text(
            changed_lines,
            self.current_hunk,
            hunk_count,
        ));
    }
}

pub(super) fn current_hunk_after_recompute(
    previous: Option<usize>,
    hunk_count: usize,
    too_large: bool,
) -> Option<usize> {
    if hunk_count == 0 || too_large {
        return None;
    }
    let previous = previous.map_or(0, |previous| previous);
    Some(previous.min(hunk_count - 1))
}

fn compare_status_text(
    changed_lines: usize,
    current_hunk: Option<usize>,
    hunk_count: usize,
) -> String {
    let plural_count = u32::try_from(changed_lines).map_or(u32::MAX, |value| value);
    let changed_lines = changed_lines.to_string();
    if let Some(current) = current_hunk {
        return ngettext(
            "%1$d changed line - %2$d/%3$d",
            "%1$d changed lines - %2$d/%3$d",
            plural_count,
        )
        .replace("%1$d", &changed_lines)
        .replace("%2$d", &(current + 1).to_string())
        .replace("%3$d", &hunk_count.to_string());
    }
    ngettext("%d changed line", "%d changed lines", plural_count).replace("%d", &changed_lines)
}

#[cfg(test)]
mod tests {
    use super::compare_status_text;

    #[test]
    fn compare_status_text_keeps_hunk_position_translatable() {
        assert_eq!(compare_status_text(1, None, 2), "1 changed line");
        assert_eq!(compare_status_text(2, None, 2), "2 changed lines");
        assert_eq!(compare_status_text(1, Some(0), 2), "1 changed line - 1/2");
        assert_eq!(compare_status_text(2, Some(1), 2), "2 changed lines - 2/2");
    }
}
