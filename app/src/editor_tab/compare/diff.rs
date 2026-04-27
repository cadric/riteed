use similar::{ChangeTag, TextDiff};

const MAX_COMPARE_BYTES: usize = 1_000_000;
const MAX_COMPARE_LINES: usize = 20_000;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct DiffHunk {
    pub(super) editable_lines: Vec<usize>,
    pub(super) reference_lines: Vec<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompareLineAnchor {
    pub(super) editable_line: usize,
    pub(super) reference_line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DiffPlan {
    pub(super) too_large: bool,
    pub(super) editable_lines: Vec<usize>,
    pub(super) reference_lines: Vec<usize>,
    pub(super) hunks: Vec<DiffHunk>,
    pub(super) anchors: Vec<CompareLineAnchor>,
}

impl DiffPlan {
    pub(super) fn empty() -> Self {
        Self {
            too_large: false,
            editable_lines: Vec::new(),
            reference_lines: Vec::new(),
            hunks: Vec::new(),
            anchors: Vec::new(),
        }
    }

    pub(super) fn changed_line_count(&self) -> usize {
        self.hunks
            .iter()
            .map(|hunk| hunk.editable_lines.len().max(hunk.reference_lines.len()))
            .sum()
    }
}

pub(super) fn compute_diff_plan(editable_text: &str, reference_text: &str) -> DiffPlan {
    if compare_too_large(editable_text, reference_text) {
        return DiffPlan {
            too_large: true,
            ..DiffPlan::empty()
        };
    }
    let editable_line_count = valid_line_count(editable_text);
    let reference_line_count = valid_line_count(reference_text);
    let diff = TextDiff::from_lines(editable_text, reference_text);
    let mut plan = DiffPlan::empty();
    for group in diff.grouped_ops(3) {
        let mut hunk = DiffHunk::default();
        for op in group {
            for change in diff.iter_changes(&op) {
                match change.tag() {
                    ChangeTag::Delete => {
                        if let Some(line) = change.old_index() {
                            plan.editable_lines.push(line);
                            hunk.editable_lines.push(line);
                        }
                    }
                    ChangeTag::Insert => {
                        if let Some(line) = change.new_index() {
                            plan.reference_lines.push(line);
                            hunk.reference_lines.push(line);
                        }
                    }
                    ChangeTag::Equal => {}
                }
            }
        }
        if !hunk.editable_lines.is_empty() || !hunk.reference_lines.is_empty() {
            plan.anchors.push(anchor_for_hunk(
                &hunk,
                editable_line_count,
                reference_line_count,
            ));
            plan.hunks.push(hunk);
        }
    }
    plan
}

#[must_use]
pub(super) fn map_line_with_anchors(
    source_line: usize,
    anchors: &[CompareLineAnchor],
    from_editable: bool,
    target_line_count: usize,
) -> usize {
    let valid_target_lines = target_line_count.max(1);
    let Some(anchor) = closest_prior_anchor(source_line, anchors, from_editable) else {
        return source_line.min(valid_target_lines - 1);
    };
    let source_anchor = if from_editable {
        anchor.editable_line
    } else {
        anchor.reference_line
    };
    let target_anchor = if from_editable {
        anchor.reference_line
    } else {
        anchor.editable_line
    };
    let delta = source_line.saturating_sub(source_anchor);
    target_anchor
        .saturating_add(delta)
        .min(valid_target_lines - 1)
}

fn anchor_for_hunk(
    hunk: &DiffHunk,
    editable_line_count: usize,
    reference_line_count: usize,
) -> CompareLineAnchor {
    match (hunk.editable_lines.first(), hunk.reference_lines.first()) {
        (Some(editable), Some(reference)) => CompareLineAnchor {
            editable_line: *editable,
            reference_line: *reference,
        },
        (Some(editable), None) => CompareLineAnchor {
            editable_line: *editable,
            reference_line: (*editable).min(reference_line_count.saturating_sub(1)),
        },
        (None, Some(reference)) => CompareLineAnchor {
            editable_line: (*reference).min(editable_line_count.saturating_sub(1)),
            reference_line: *reference,
        },
        (None, None) => CompareLineAnchor {
            editable_line: 0,
            reference_line: 0,
        },
    }
}

fn closest_prior_anchor(
    source_line: usize,
    anchors: &[CompareLineAnchor],
    from_editable: bool,
) -> Option<&CompareLineAnchor> {
    anchors
        .iter()
        .take_while(|anchor| {
            if from_editable {
                anchor.editable_line <= source_line
            } else {
                anchor.reference_line <= source_line
            }
        })
        .last()
}

fn compare_too_large(editable_text: &str, reference_text: &str) -> bool {
    editable_text.len().saturating_add(reference_text.len()) > MAX_COMPARE_BYTES
        || line_count(editable_text).saturating_add(line_count(reference_text)) > MAX_COMPARE_LINES
}

fn line_count(text: &str) -> usize {
    if text.is_empty() {
        0
    } else {
        text.lines().count().max(1)
    }
}

fn valid_line_count(text: &str) -> usize {
    line_count(text).max(1)
}

#[cfg(test)]
mod tests {
    use super::{MAX_COMPARE_BYTES, compute_diff_plan, map_line_with_anchors};

    #[test]
    fn line_diff_groups_adjacent_changes_into_hunks() {
        let plan = compute_diff_plan("a\nb\nc\n", "a\nx\nc\n");
        assert!(!plan.too_large);
        assert_eq!(plan.hunks.len(), 1);
        assert_eq!(plan.editable_lines, vec![1]);
        assert_eq!(plan.reference_lines, vec![1]);
        assert_eq!(plan.anchors[0].editable_line, 1);
        assert_eq!(plan.anchors[0].reference_line, 1);
    }

    #[test]
    fn line_diff_reports_insert_and_delete_lines() {
        let plan = compute_diff_plan("a\nb\n", "a\nb\nc\n");
        assert_eq!(plan.hunks.len(), 1);
        assert!(plan.editable_lines.is_empty());
        assert_eq!(plan.reference_lines, vec![2]);
        assert_eq!(plan.anchors[0].editable_line, 1);
        assert_eq!(plan.anchors[0].reference_line, 2);
        assert_eq!(plan.changed_line_count(), 1);
    }

    #[test]
    fn changed_line_count_counts_replacements_once_per_side_pair() {
        let plan = compute_diff_plan("a\nb\nc\nd\n", "a\nx\nc\ny\n");
        assert_eq!(plan.hunks.len(), 1);
        assert_eq!(plan.editable_lines, vec![1, 3]);
        assert_eq!(plan.reference_lines, vec![1, 3]);
        assert_eq!(plan.changed_line_count(), 2);
    }

    #[test]
    fn performance_guard_skips_large_inputs() {
        let large = "x".repeat(MAX_COMPARE_BYTES + 1);
        let plan = compute_diff_plan(&large, "");
        assert!(plan.too_large);
        assert!(plan.hunks.is_empty());
    }

    #[test]
    fn anchor_mapping_preserves_line_delta_after_prior_change() {
        let plan = compute_diff_plan("a\nb\nc\nd\n", "a\nx\nc\nd\n");
        assert_eq!(map_line_with_anchors(3, &plan.anchors, true, 4), 3);
        assert_eq!(map_line_with_anchors(3, &plan.anchors, false, 4), 3);
    }

    #[test]
    fn anchor_mapping_keeps_lines_before_first_change() {
        let plan = compute_diff_plan("a\nb\nc\n", "a\nb\nx\n");
        assert_eq!(map_line_with_anchors(0, &plan.anchors, true, 3), 0);
        assert_eq!(map_line_with_anchors(0, &plan.anchors, false, 3), 0);
    }
}
