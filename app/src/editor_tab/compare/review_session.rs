use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use gettextrs::{gettext, ngettext, pgettext};
use gtk4::{gio, prelude::*};

use super::diff::{DiffOptions, DiffSkipReason, compute_diff_with_options, line_slices};
use super::display::{
    CompareDisplayModel, CompareDisplayOptions, CompareDisplayRow, DisplayCollapsedRow,
    DisplayContentRow, DisplayFileBoundaryRow, DisplayRowId, DisplayRowIdKind,
    DisplaySkippedMarkerRow, build_display_model,
};
use super::model::{DiffRowKind, DiffRowModel};
use crate::editor_tab::{ReviewFileId, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec};
use crate::git_status::{GitFileStatus, escape_git_path_display};
use crate::settings::CompareReviewSettingsSnapshot;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ReviewFileInput {
    pub(crate) file_id: ReviewFileId,
    pub(crate) status: GitFileStatus,
    pub(crate) reference_text: Option<String>,
    pub(crate) current_text: Option<String>,
    pub(crate) skip_reason: Option<String>,
}

impl ReviewFileInput {
    #[must_use]
    pub(crate) fn file(
        file_id: ReviewFileId,
        status: GitFileStatus,
        reference_text: Option<String>,
        current_text: Option<String>,
    ) -> Self {
        Self {
            file_id,
            status,
            reference_text,
            current_text,
            skip_reason: None,
        }
    }

    #[must_use]
    pub(crate) fn skipped(
        file_id: ReviewFileId,
        status: GitFileStatus,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            file_id,
            status,
            reference_text: None,
            current_text: None,
            skip_reason: Some(reason.into()),
        }
    }
}

pub(in crate::editor_tab) struct ReviewSession {
    pub(super) review_kind: ReviewKind,
    pub(super) repo_root: PathBuf,
    pub(super) review_generation_at_creation: u64,
    pub(super) fingerprint_at_creation: ReviewSnapshotFingerprint,
    pub(super) settings_snapshot: CompareReviewSettingsSnapshot,
    pub(super) files: Vec<ReviewFileEntry>,
    pub(super) rendered_lines: Vec<ReviewRenderedLine>,
    pub(super) stale: bool,
}

pub(super) struct ReviewFileEntry {
    pub(super) file_id: ReviewFileId,
    pub(super) status: GitFileStatus,
    pub(super) reference_text: Option<String>,
    pub(super) current_text: Option<String>,
    pub(super) diff_row_model: Option<DiffRowModel>,
    pub(super) skip_reason: Option<String>,
    pub(super) collapse_state: CollapseState,
    pub(super) display_model: Option<CompareDisplayModel>,
    pub(super) additions: usize,
    pub(super) removals: usize,
}

#[derive(Default)]
pub(super) struct CollapseState {
    pub(super) revealed_rows: BTreeSet<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReviewRenderedLine {
    pub(super) file_index: usize,
    pub(super) file_id: ReviewFileId,
    pub(super) kind: RenderedLineKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RenderedLineKind {
    FileBoundary,
    Content { display_row_id: DisplayRowId },
    Collapsed { id: DisplayRowId },
    SkippedMarker { reason: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReviewScrollTarget {
    pub(crate) line_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::editor_tab) struct ReviewChangeListItem {
    pub(in crate::editor_tab) label: String,
    pub(in crate::editor_tab) description: String,
    pub(in crate::editor_tab) target: ReviewScrollTarget,
}

impl ReviewSession {
    #[must_use]
    pub(in crate::editor_tab) fn from_spec(spec: &ReviewTabSpec) -> Self {
        Self {
            review_kind: spec.review_kind,
            repo_root: spec.repo_root.clone(),
            review_generation_at_creation: spec.snapshot_generation_at_creation,
            fingerprint_at_creation: spec.fingerprint_at_creation.clone(),
            settings_snapshot: spec.settings_snapshot,
            files: Vec::new(),
            rendered_lines: Vec::new(),
            stale: false,
        }
    }

    #[must_use]
    pub(in crate::editor_tab) fn from_inputs(
        spec: &ReviewTabSpec,
        inputs: Vec<ReviewFileInput>,
    ) -> Self {
        let mut session = Self::from_spec(spec);
        session.files = inputs
            .into_iter()
            .map(|input| ReviewFileEntry::from_input(input, spec.settings_snapshot))
            .collect();
        session.rebuild_displays(spec.settings_snapshot);
        session
    }

    pub(in crate::editor_tab) fn rebuild_displays(
        &mut self,
        settings: CompareReviewSettingsSnapshot,
    ) {
        self.settings_snapshot = settings;
        for file in &mut self.files {
            file.rebuild_display(settings);
        }
        self.rebuild_rendered_lines();
    }

    #[must_use]
    pub(in crate::editor_tab) fn render_text(&self) -> String {
        if self.files.is_empty() {
            return gettext("No reviewable text changes were found.");
        }
        let mut lines = Vec::new();
        for file in &self.files {
            push_display_row(&mut lines, &file_boundary_row(file));
            if let Some(reason) = file.skip_reason.as_deref() {
                push_display_row(&mut lines, &skipped_marker_row(file, reason.to_string()));
                continue;
            }
            let Some(display) = file.display_model.as_ref() else {
                continue;
            };
            for row in &display.rows {
                push_display_row(&mut lines, row);
            }
        }
        lines.join("\n")
    }

    pub(in crate::editor_tab) fn render_into_buffer(&mut self, buffer: &sourceview5::Buffer) {
        self.rebuild_rendered_lines();
        buffer.set_text(&self.render_text());
        buffer.set_modified(false);
    }

    #[must_use]
    pub(in crate::editor_tab) fn file_count(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub(in crate::editor_tab) fn repo_root(&self) -> PathBuf {
        self.repo_root.clone()
    }

    #[must_use]
    pub(in crate::editor_tab) fn is_stale(&self) -> bool {
        self.stale
    }

    pub(in crate::editor_tab) fn clear_stale(&mut self) {
        self.stale = false;
    }

    #[must_use]
    pub(in crate::editor_tab) fn target_for_direction(
        &self,
        current_line: Option<usize>,
        direction: i32,
    ) -> Option<ReviewScrollTarget> {
        let line = current_line.unwrap_or(0);
        if direction >= 0 {
            let first = self.first_navigation_line()?;
            let next = self
                .rendered_lines
                .iter()
                .enumerate()
                .skip(line.saturating_add(1))
                .find_map(|(index, rendered)| self.is_navigation_line(rendered).then_some(index))
                .unwrap_or(first);
            return Some(ReviewScrollTarget { line_index: next });
        }
        self.rendered_lines
            .iter()
            .enumerate()
            .take(line)
            .rev()
            .find_map(|(index, rendered)| self.is_navigation_line(rendered).then_some(index))
            .or_else(|| self.last_navigation_line())
            .map(|target| ReviewScrollTarget { line_index: target })
    }

    fn first_navigation_line(&self) -> Option<usize> {
        self.rendered_lines
            .iter()
            .enumerate()
            .find_map(|(index, line)| self.is_navigation_line(line).then_some(index))
    }

    fn last_navigation_line(&self) -> Option<usize> {
        self.rendered_lines
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, line)| self.is_navigation_line(line).then_some(index))
    }

    #[must_use]
    pub(in crate::editor_tab) fn current_file_for_line(&self, line: usize) -> Option<ReviewFileId> {
        self.rendered_lines
            .get(line)
            .map(|rendered| rendered.file_id.clone())
    }

    #[must_use]
    pub(in crate::editor_tab) fn open_target_for_line(&self, line: usize) -> Option<gio::File> {
        let rendered = self.rendered_lines.get(line)?;
        let file = self
            .files
            .iter()
            .find(|file| file.file_id == rendered.file_id)?;
        file.current_text.as_ref()?;
        if file.skip_reason.is_some() {
            return None;
        }
        let path = OsString::from_vec(file.file_id.raw_path.clone());
        Some(gio::File::for_path(self.repo_root.join(path)))
    }

    pub(in crate::editor_tab) fn mark_stale_if_mismatch(
        &mut self,
        current_fingerprint: &ReviewSnapshotFingerprint,
        current_generation: u64,
    ) -> bool {
        let stale = match self.review_kind {
            ReviewKind::Staged => &self.fingerprint_at_creation != current_fingerprint,
            ReviewKind::Unstaged => {
                self.review_generation_at_creation != current_generation
                    || &self.fingerprint_at_creation != current_fingerprint
            }
        };
        self.stale = stale;
        stale
    }

    #[must_use]
    pub(in crate::editor_tab) fn change_list_items(&self) -> Vec<ReviewChangeListItem> {
        let mut items = Vec::new();
        for (index, line) in self.rendered_lines.iter().enumerate() {
            match &line.kind {
                RenderedLineKind::FileBoundary => {
                    items.push(ReviewChangeListItem {
                        label: path_display(&line.file_id.raw_path),
                        description: pgettext("change list row", "Changed file"),
                        target: ReviewScrollTarget { line_index: index },
                    });
                }
                RenderedLineKind::Content { .. } => {
                    if self.is_navigation_line(line) {
                        items.push(ReviewChangeListItem {
                            label: path_display(&line.file_id.raw_path),
                            description: pgettext("change list row", "Changed line"),
                            target: ReviewScrollTarget { line_index: index },
                        });
                    }
                }
                RenderedLineKind::Collapsed { .. } => {
                    items.push(ReviewChangeListItem {
                        label: path_display(&line.file_id.raw_path),
                        description: pgettext("change list row", "Hidden unchanged lines"),
                        target: ReviewScrollTarget { line_index: index },
                    });
                }
                RenderedLineKind::SkippedMarker { reason } => {
                    items.push(ReviewChangeListItem {
                        label: path_display(&line.file_id.raw_path),
                        description: reason.clone(),
                        target: ReviewScrollTarget { line_index: index },
                    });
                }
            }
        }
        items
    }

    fn is_navigation_line(&self, line: &ReviewRenderedLine) -> bool {
        match &line.kind {
            RenderedLineKind::FileBoundary | RenderedLineKind::Collapsed { .. } => false,
            RenderedLineKind::SkippedMarker { .. } => true,
            RenderedLineKind::Content { display_row_id } => self
                .files
                .get(line.file_index)
                .and_then(|file| file.display_model.as_ref())
                .and_then(|display| {
                    display.rows.iter().find_map(|row| match row {
                        CompareDisplayRow::Content(row) if &row.id == display_row_id => {
                            Some(row.kind != DiffRowKind::Equal)
                        }
                        _ => None,
                    })
                })
                .unwrap_or(false),
        }
    }

    fn rebuild_rendered_lines(&mut self) {
        let mut rendered = Vec::new();
        for (file_index, file) in self.files.iter().enumerate() {
            rendered.push(rendered_line_for_row(
                file_index,
                &file.file_id,
                &file_boundary_row(file),
            ));
            if let Some(reason) = file.skip_reason.as_ref() {
                rendered.push(rendered_line_for_row(
                    file_index,
                    &file.file_id,
                    &skipped_marker_row(file, reason.clone()),
                ));
                continue;
            }
            let Some(display) = file.display_model.as_ref() else {
                continue;
            };
            for row in &display.rows {
                rendered.push(rendered_line_for_row(file_index, &file.file_id, row));
            }
        }
        self.rendered_lines = rendered;
    }
}

impl ReviewFileEntry {
    fn from_input(input: ReviewFileInput, settings: CompareReviewSettingsSnapshot) -> Self {
        let mut entry = Self {
            file_id: input.file_id,
            status: input.status,
            reference_text: input.reference_text,
            current_text: input.current_text,
            diff_row_model: None,
            skip_reason: input.skip_reason,
            collapse_state: CollapseState::default(),
            display_model: None,
            additions: 0,
            removals: 0,
        };
        entry.rebuild_row_model(settings);
        entry
    }

    fn rebuild_row_model(&mut self, settings: CompareReviewSettingsSnapshot) {
        if self.skip_reason.is_some() {
            return;
        }
        let reference = self.reference_text.as_deref().unwrap_or("");
        let current = self.current_text.as_deref().unwrap_or("");
        let computation = compute_diff_with_options(
            reference,
            current,
            DiffOptions {
                ignore_leading_trailing_whitespace: settings.ignore_leading_trailing_whitespace,
            },
        );
        if let Some(reason) = computation.skip_reason {
            self.skip_reason = Some(skip_reason_text(reason));
            self.diff_row_model = None;
            return;
        }
        let counts = changed_counts(&computation.model);
        self.additions = counts.additions;
        self.removals = counts.removals;
        self.diff_row_model = Some(computation.model);
    }

    fn rebuild_display(&mut self, settings: CompareReviewSettingsSnapshot) {
        let Some(model) = self.diff_row_model.as_ref() else {
            self.display_model = None;
            return;
        };
        let reference_text = self.reference_text.as_deref().unwrap_or("");
        let current_text = self.current_text.as_deref().unwrap_or("");
        let reference_lines = line_slices(reference_text);
        let current_lines = line_slices(current_text);
        let options = CompareDisplayOptions {
            collapse_unchanged: settings.collapse_unchanged,
            context_lines: usize::try_from(settings.context_lines)
                .map_or(3, |value| value)
                .clamp(1, 10),
            revealed_rows: self.collapse_state.revealed_rows.clone(),
        };
        self.display_model = Some(build_display_model(
            Some(&self.file_id),
            model,
            &reference_lines,
            &current_lines,
            &options,
        ));
    }
}

struct ChangeCounts {
    additions: usize,
    removals: usize,
}

fn changed_counts(model: &DiffRowModel) -> ChangeCounts {
    let mut counts = ChangeCounts {
        additions: 0,
        removals: 0,
    };
    for row in &model.rows {
        match row.kind {
            DiffRowKind::Equal => {}
            DiffRowKind::ReferenceOnly => counts.removals = counts.removals.saturating_add(1),
            DiffRowKind::CurrentOnly => counts.additions = counts.additions.saturating_add(1),
            DiffRowKind::Modify => {
                counts.additions = counts.additions.saturating_add(1);
                counts.removals = counts.removals.saturating_add(1);
            }
        }
    }
    counts
}

fn rendered_line_for_row(
    file_index: usize,
    file_id: &ReviewFileId,
    row: &CompareDisplayRow,
) -> ReviewRenderedLine {
    let kind = match row {
        CompareDisplayRow::FileBoundary(_) => RenderedLineKind::FileBoundary,
        CompareDisplayRow::Content(row) => RenderedLineKind::Content {
            display_row_id: row.id.clone(),
        },
        CompareDisplayRow::Collapsed(row) => RenderedLineKind::Collapsed { id: row.id.clone() },
        CompareDisplayRow::SkippedMarker(row) => RenderedLineKind::SkippedMarker {
            reason: row.reason.clone(),
        },
    };
    ReviewRenderedLine {
        file_index,
        file_id: file_id.clone(),
        kind,
    }
}

fn push_display_row(lines: &mut Vec<String>, row: &CompareDisplayRow) {
    match row {
        CompareDisplayRow::FileBoundary(row) => {
            lines.push(file_boundary_label_parts(
                &row.path,
                row.status_badge,
                row.additions,
                row.removals,
            ));
        }
        CompareDisplayRow::Content(row) => lines.push(content_row_text(row)),
        CompareDisplayRow::Collapsed(row) => lines.push(collapsed_row_text(row)),
        CompareDisplayRow::SkippedMarker(row) => lines.push(row.reason.clone()),
    }
}

fn content_row_text(row: &DisplayContentRow) -> String {
    row.current_text
        .as_ref()
        .or(row.reference_text.as_ref())
        .cloned()
        .unwrap_or_default()
}

fn collapsed_row_text(row: &DisplayCollapsedRow) -> String {
    row.label.clone()
}

fn file_boundary_row(file: &ReviewFileEntry) -> CompareDisplayRow {
    CompareDisplayRow::FileBoundary(DisplayFileBoundaryRow {
        id: DisplayRowId {
            file_id: Some(file.file_id.clone()),
            kind: DisplayRowIdKind::FileBoundary,
        },
        path: path_display(&file.file_id.raw_path),
        status_badge: file.status.badge(),
        additions: file.additions,
        removals: file.removals,
    })
}

fn skipped_marker_row(file: &ReviewFileEntry, reason: String) -> CompareDisplayRow {
    CompareDisplayRow::SkippedMarker(DisplaySkippedMarkerRow {
        id: DisplayRowId {
            file_id: Some(file.file_id.clone()),
            kind: DisplayRowIdKind::SkippedMarker,
        },
        reason,
    })
}

fn file_boundary_label_parts(
    path: &str,
    status_badge: &str,
    additions: usize,
    removals: usize,
) -> String {
    let additions_text = count_text(additions, "%d addition", "%d additions");
    let removals_text = count_text(removals, "%d removal", "%d removals");
    format!("{path} ({status_badge}, {additions_text}, {removals_text})")
}

fn count_text(count: usize, singular: &str, plural: &str) -> String {
    let plural_count = u32::try_from(count).map_or(u32::MAX, |value| value);
    ngettext(singular, plural, plural_count).replace("%d", &count.to_string())
}

fn path_display(raw_path: &[u8]) -> String {
    if raw_path.is_empty() {
        return pgettext("git review path", "Review limit");
    }
    std::str::from_utf8(raw_path).map_or_else(
        |_error| pgettext("git path fallback", "Invalid path encoding"),
        escape_git_path_display,
    )
}

fn skip_reason_text(reason: DiffSkipReason) -> String {
    match reason {
        DiffSkipReason::Bytes => {
            gettext("Diff was skipped because the files are over the compare byte limit.")
        }
        DiffSkipReason::Lines => {
            gettext("Diff was skipped because the files are over the compare line limit.")
        }
        DiffSkipReason::Computation => gettext("Diff was too large to compute fully."),
    }
}
