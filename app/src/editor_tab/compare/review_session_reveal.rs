use super::display::DisplayRowIdKind;
use super::reveal::{RevealScope, reveal_rows};
use super::review_session::{RenderedLineKind, ReviewScrollTarget, ReviewSession};

impl ReviewSession {
    #[must_use]
    pub(in crate::editor_tab) fn can_reveal_context(&self, current_line: Option<usize>) -> bool {
        self.current_collapsed_marker(current_line).is_some()
    }

    pub(in crate::editor_tab) fn reveal_above(
        &mut self,
        current_line: Option<usize>,
    ) -> Option<ReviewScrollTarget> {
        self.reveal_context(current_line, RevealScope::Above)
    }

    pub(in crate::editor_tab) fn reveal_below(
        &mut self,
        current_line: Option<usize>,
    ) -> Option<ReviewScrollTarget> {
        self.reveal_context(current_line, RevealScope::Below)
    }

    pub(in crate::editor_tab) fn reveal_current_all(
        &mut self,
        current_line: Option<usize>,
    ) -> Option<ReviewScrollTarget> {
        self.reveal_context(current_line, RevealScope::All)
    }

    fn reveal_context(
        &mut self,
        current_line: Option<usize>,
        scope: RevealScope,
    ) -> Option<ReviewScrollTarget> {
        let marker = self.current_collapsed_marker(current_line)?;
        let rows = reveal_rows(
            marker.hidden_start,
            marker.hidden_end,
            self.settings_snapshot.context_lines,
            scope,
        );
        if rows.is_empty() {
            return None;
        }
        let file = self.files.get_mut(marker.file_index)?;
        let before = file.collapse_state.revealed_rows.len();
        file.collapse_state.revealed_rows.extend(rows);
        if file.collapse_state.revealed_rows.len() == before {
            return None;
        }
        self.rebuild_displays(self.settings_snapshot);
        self.line_for_remaining_marker(marker.file_index, marker.hidden_start, marker.hidden_end)
            .map(|line_index| ReviewScrollTarget { line_index })
    }

    fn current_collapsed_marker(
        &self,
        current_line: Option<usize>,
    ) -> Option<CollapsedMarkerTarget> {
        let mut line = current_line?.min(self.rendered_lines.len().saturating_sub(1));
        loop {
            let rendered = self.rendered_lines.get(line)?;
            if let RenderedLineKind::Collapsed { id } = &rendered.kind
                && let DisplayRowIdKind::Collapsed {
                    hidden_start,
                    hidden_end,
                } = id.kind
            {
                return Some(CollapsedMarkerTarget {
                    file_index: rendered.file_index,
                    hidden_start,
                    hidden_end,
                });
            }
            if line == 0 {
                return None;
            }
            line = line.saturating_sub(1);
        }
    }

    fn line_for_remaining_marker(
        &self,
        file_index: usize,
        old_start: usize,
        old_end: usize,
    ) -> Option<usize> {
        self.rendered_lines
            .iter()
            .enumerate()
            .find_map(|(line_index, line)| {
                if line.file_index != file_index {
                    return None;
                }
                let RenderedLineKind::Collapsed { id } = &line.kind else {
                    return None;
                };
                let DisplayRowIdKind::Collapsed {
                    hidden_start,
                    hidden_end,
                } = id.kind
                else {
                    return None;
                };
                (hidden_start >= old_start && hidden_end <= old_end).then_some(line_index)
            })
    }
}

struct CollapsedMarkerTarget {
    file_index: usize,
    hidden_start: usize,
    hidden_end: usize,
}

#[cfg(test)]
mod tests {
    use super::ReviewSession;
    use crate::editor_tab::compare::review_session::ReviewFileInput;
    use crate::editor_tab::{
        ReviewFileId, ReviewFileSpec, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec,
    };
    use crate::git_status::GitFileStatus;
    use crate::settings::{CompareReviewSettingsSnapshot, CompareViewMode};

    #[test]
    fn reveal_above_extends_top_of_current_marker() {
        let mut session = collapsed_session();

        assert!(session.can_reveal_context(Some(3)));
        let target = session.reveal_above(Some(3));

        assert!(target.is_some());
        assert!(session.files[0].collapse_state.revealed_rows.contains(&2));
        assert!(!session.files[0].collapse_state.revealed_rows.contains(&4));
    }

    #[test]
    fn reveal_below_extends_bottom_of_current_marker() {
        let mut session = collapsed_session();
        let target = session.reveal_below(Some(3));

        assert!(target.is_some());
        assert!(session.files[0].collapse_state.revealed_rows.contains(&4));
        assert!(!session.files[0].collapse_state.revealed_rows.contains(&2));
    }

    #[test]
    fn reveal_all_expands_current_marker_range() {
        let mut session = collapsed_session();
        let target = session.reveal_current_all(Some(3));

        assert!(target.is_none());
        for row in 2..5 {
            assert!(session.files[0].collapse_state.revealed_rows.contains(&row));
        }
        assert!(!session.can_reveal_context(Some(3)));
    }

    fn collapsed_session() -> ReviewSession {
        ReviewSession::from_inputs(
            &spec(),
            vec![ReviewFileInput::file(
                ReviewFileId::new(ReviewKind::Staged, b"file.txt".to_vec()),
                GitFileStatus::Modified,
                Some(String::from("0\n1\n2\n3\n4\n5\n6\n")),
                Some(String::from("x\n1\n2\n3\n4\n5\ny\n")),
            )],
        )
    }

    fn spec() -> ReviewTabSpec {
        ReviewTabSpec::new(
            ReviewKind::Staged,
            std::path::PathBuf::from("/repo"),
            7,
            ReviewSnapshotFingerprint::new("fingerprint"),
            vec![ReviewFileSpec::new(b"file.txt".to_vec())],
            CompareReviewSettingsSnapshot {
                view_mode: CompareViewMode::Unified,
                collapse_unchanged: true,
                context_lines: 1,
                ignore_leading_trailing_whitespace: false,
                word_wrap: false,
            },
        )
    }
}
