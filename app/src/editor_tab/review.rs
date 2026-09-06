use std::path::PathBuf;
use std::rc::Rc;

use gtk4::{gio, glib, prelude::*};

use crate::settings::CompareReviewSettingsSnapshot;

use super::{EditorTab, VisibleBannerState};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabKind {
    Document,
    GitReview,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ReviewKind {
    Staged,
    Unstaged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewSnapshotFingerprint {
    token: String,
}

impl ReviewSnapshotFingerprint {
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ReviewFileId {
    pub review_kind: ReviewKind,
    pub raw_path: Vec<u8>,
}

impl ReviewFileId {
    #[must_use]
    pub fn new(review_kind: ReviewKind, raw_path: Vec<u8>) -> Self {
        Self {
            review_kind,
            raw_path,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewFileSpec {
    pub raw_path: Vec<u8>,
}

impl ReviewFileSpec {
    #[must_use]
    pub fn new(raw_path: Vec<u8>) -> Self {
        Self { raw_path }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewTabSpec {
    pub review_kind: ReviewKind,
    pub repo_root: PathBuf,
    pub snapshot_generation_at_creation: u64,
    pub fingerprint_at_creation: ReviewSnapshotFingerprint,
    pub files: Vec<ReviewFileSpec>,
    pub settings_snapshot: CompareReviewSettingsSnapshot,
}

impl ReviewTabSpec {
    #[must_use]
    pub fn new(
        review_kind: ReviewKind,
        repo_root: PathBuf,
        snapshot_generation_at_creation: u64,
        fingerprint_at_creation: ReviewSnapshotFingerprint,
        files: Vec<ReviewFileSpec>,
        settings_snapshot: CompareReviewSettingsSnapshot,
    ) -> Self {
        Self {
            review_kind,
            repo_root,
            snapshot_generation_at_creation,
            fingerprint_at_creation,
            files,
            settings_snapshot,
        }
    }
}

impl EditorTab {
    pub(crate) fn set_git_review_text(&self, text: &str) {
        if self.kind() != TabKind::GitReview {
            return;
        }
        self.text_buffer.set_text(text);
        self.text_buffer.set_modified(false);
        self.sync_presentation();
    }

    pub(crate) fn set_review_load_cancellable(&self, cancellable: &gio::Cancellable) {
        if self.kind() != TabKind::GitReview {
            return;
        }
        if let Some(previous) = self
            .state
            .borrow_mut()
            .review
            .load_cancellable
            .replace(cancellable.clone())
        {
            previous.cancel();
        }
    }

    pub(crate) fn clear_review_load_cancellable(&self, cancellable: &gio::Cancellable) {
        let mut state = self.state.borrow_mut();
        if state
            .review
            .load_cancellable
            .as_ref()
            .is_some_and(|active| active == cancellable)
        {
            state.review.load_cancellable = None;
        }
    }

    pub(crate) fn cancel_review_load(&self) {
        if let Some(cancellable) = self.state.borrow_mut().review.load_cancellable.take() {
            cancellable.cancel();
        }
    }

    pub(crate) fn populate_review_session_with_spec(
        &self,
        spec: &ReviewTabSpec,
        inputs: Vec<super::ReviewFileInput>,
    ) {
        if self.kind() != TabKind::GitReview {
            return;
        }
        let session = Rc::new(std::cell::RefCell::new(
            super::compare::ReviewSession::from_inputs(spec, inputs),
        ));
        session.borrow_mut().render_into_buffer(&self.text_buffer);
        self.state.borrow_mut().review.session = Some(session);
        self.text_buffer.set_modified(false);
        self.sync_presentation();
    }

    #[must_use]
    pub fn review_file_count(&self) -> usize {
        self.state
            .borrow()
            .review
            .session
            .as_ref()
            .map_or(0, |session| session.borrow().file_count())
    }

    #[must_use]
    pub fn current_review_open_target(&self) -> Option<gio::File> {
        let line = self.current_buffer_line()?;
        self.state
            .borrow()
            .review
            .session
            .as_ref()
            .and_then(|session| session.borrow().open_target_for_line(line))
    }

    #[must_use]
    pub fn review_repo_root(&self) -> Option<PathBuf> {
        self.state
            .borrow()
            .review
            .session
            .as_ref()
            .map(|session| session.borrow().repo_root())
    }

    pub fn present_change_list(self: &Rc<Self>) {
        let items = self
            .state
            .borrow()
            .review
            .session
            .as_ref()
            .map_or_else(Vec::new, |session| session.borrow().change_list_items());
        if !items.is_empty() {
            super::compare::present_change_list_dialog(self, &items);
        }
    }

    pub(crate) fn scroll_review_to_target(&self, target: super::ReviewScrollTarget) {
        scroll_review_line(&self.text_buffer, &self.text_view, target.line_index);
    }

    pub fn review_next_change(&self) {
        self.move_review_change(1);
    }

    pub fn review_previous_change(&self) {
        self.move_review_change(-1);
    }

    pub fn review_reveal_above(&self) {
        self.review_reveal_with(super::compare::ReviewSession::reveal_above);
    }

    pub fn review_reveal_below(&self) {
        self.review_reveal_with(super::compare::ReviewSession::reveal_below);
    }

    pub fn review_reveal_all(&self) {
        self.review_reveal_with(super::compare::ReviewSession::reveal_current_all);
    }

    #[must_use]
    pub fn review_can_reveal_context(&self) -> bool {
        let line = self.current_buffer_line();
        self.state
            .borrow()
            .review
            .session
            .as_ref()
            .is_some_and(|session| session.borrow().can_reveal_context(line))
    }

    fn review_reveal_with(
        &self,
        reveal: impl FnOnce(
            &mut super::compare::ReviewSession,
            Option<usize>,
        ) -> Option<super::ReviewScrollTarget>,
    ) {
        let current_line = self.current_buffer_line();
        let session = self.state.borrow().review.session.clone();
        if let Some(session) = session {
            let viewport = super::compare::viewport::capture(&self.text_view);
            let target = {
                let mut session = session.borrow_mut();
                let target = reveal(&mut session, current_line);
                session.render_into_buffer(&self.text_buffer);
                target
            };
            self.text_buffer.set_modified(false);
            self.sync_presentation();
            let cursor = target.map(|target| target.line_index).or(current_line);
            super::compare::viewport::restore_with_cursor_line(&viewport, &self.text_view, cursor);
        }
    }

    pub fn refresh_review_session(&self) {
        let session = self.state.borrow().review.session.clone();
        if let Some(session) = session {
            let settings = self.settings.compare_review_settings_snapshot();
            {
                let mut session = session.borrow_mut();
                session.clear_stale();
                session.rebuild_displays(settings);
                session.render_into_buffer(&self.text_buffer);
            }
            self.text_buffer.set_modified(false);
            self.state.borrow_mut().ui.visible_banner = VisibleBannerState::None;
            self.banner.set_button_label(None);
            self.set_banner_revealed(false);
            self.sync_presentation();
        }
    }

    pub fn mark_review_stale_if_mismatch(
        &self,
        fingerprint: &ReviewSnapshotFingerprint,
        generation: u64,
    ) -> bool {
        let stale = self
            .state
            .borrow()
            .review
            .session
            .as_ref()
            .is_some_and(|session| {
                session
                    .borrow_mut()
                    .mark_stale_if_mismatch(fingerprint, generation)
            });
        if stale {
            self.sync_external_banner(true, true);
        }
        stale
    }

    fn current_buffer_line(&self) -> Option<usize> {
        if self.kind() != TabKind::GitReview {
            return None;
        }
        let iter = self
            .text_buffer
            .iter_at_mark(&self.text_buffer.get_insert());
        usize::try_from(iter.line()).ok()
    }

    fn move_review_change(&self, direction: i32) {
        let current_line = self.current_buffer_line();
        let target = self
            .state
            .borrow()
            .review
            .session
            .as_ref()
            .and_then(|session| {
                session
                    .borrow()
                    .target_for_direction(current_line, direction)
            });
        if let Some(target) = target {
            queue_scroll_review_line(&self.text_buffer, &self.text_view, target.line_index);
        }
    }
}

fn queue_scroll_review_line(
    buffer: &sourceview5::Buffer,
    view: &sourceview5::View,
    line_index: usize,
) {
    let buffer = buffer.clone();
    let view = view.clone();
    glib::idle_add_local_once(move || {
        scroll_review_line(&buffer, &view, line_index);
    });
}

fn scroll_review_line(buffer: &sourceview5::Buffer, view: &sourceview5::View, line_index: usize) {
    let line = i32::try_from(line_index).map_or(i32::MAX, |value| value);
    let Some(mut iter) = buffer.iter_at_line(line) else {
        return;
    };
    buffer.place_cursor(&iter);
    view.scroll_to_iter(&mut iter, 0.2, true, 0.0, 0.5);
}

#[cfg(test)]
mod tests {
    use super::{
        ReviewFileId, ReviewFileSpec, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec,
    };
    use crate::settings::{CompareReviewSettingsSnapshot, CompareViewMode};

    #[test]
    fn review_identifiers_preserve_kind_and_raw_path() {
        let file_id = ReviewFileId::new(ReviewKind::Unstaged, b"dir/\xff.txt".to_vec());
        let spec = ReviewFileSpec::new(file_id.raw_path.clone());

        assert_eq!(file_id.review_kind, ReviewKind::Unstaged);
        assert_eq!(spec.raw_path, b"dir/\xff.txt");
    }

    #[test]
    fn review_tab_spec_keeps_snapshot_metadata() {
        let fingerprint = ReviewSnapshotFingerprint::new("token");
        let settings = CompareReviewSettingsSnapshot {
            view_mode: CompareViewMode::Unified,
            collapse_unchanged: true,
            context_lines: 4,
            ignore_leading_trailing_whitespace: true,
            word_wrap: true,
        };
        let spec = ReviewTabSpec::new(
            ReviewKind::Staged,
            std::path::PathBuf::from("/repo"),
            42,
            fingerprint.clone(),
            vec![ReviewFileSpec::new(b"file.txt".to_vec())],
            settings,
        );

        assert_eq!(fingerprint.token(), "token");
        assert_eq!(spec.review_kind, ReviewKind::Staged);
        assert_eq!(spec.snapshot_generation_at_creation, 42);
        assert_eq!(spec.fingerprint_at_creation, fingerprint);
        assert_eq!(spec.settings_snapshot, settings);
    }
}
