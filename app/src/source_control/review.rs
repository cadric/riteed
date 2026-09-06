use std::fmt::Write as _;
use std::path::Path;
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};

use crate::editor_tab::{
    EditorTab, ReviewFileSpec, ReviewKind, ReviewSnapshotFingerprint, ReviewTabSpec,
};
#[cfg(test)]
use crate::git_status::GitStatusSnapshot;
use crate::git_status::{GitAttrState, GitFileStatus, GitStatusEntry};
use crate::source_control::slots::SnapshotId;
use crate::source_control::{
    SourceStateRef, git_attrs_unavailable_text, review_loader, set_status_label,
};
use crate::workspace::Workspace;

struct ReviewBuild {
    spec: ReviewTabSpec,
    entries: Vec<GitStatusEntry>,
    snapshot_id: SnapshotId,
}

pub(super) fn install_actions(state: &SourceStateRef, window: &impl IsA<gio::ActionMap>) {
    let (staged, unstaged) = {
        let state = state.borrow();
        (
            state.review_staged_action.clone(),
            state.review_unstaged_action.clone(),
        )
    };
    add_review_action(state, window, &staged, ReviewKind::Staged);
    add_review_action(state, window, &unstaged, ReviewKind::Unstaged);
    sync_actions(state);
}

pub(super) fn review_menu_model() -> gio::Menu {
    let menu = gio::Menu::new();
    menu.append(
        Some(&pgettext(
            "source control menu item",
            "Review Staged Changes",
        )),
        Some("win.git-review-staged"),
    );
    menu.append(
        Some(&pgettext(
            "source control menu item",
            "Review Unstaged Changes",
        )),
        Some("win.git-review-unstaged"),
    );
    menu
}

#[must_use]
pub(crate) fn reviewable_staged(entry: &GitStatusEntry) -> bool {
    entry.staged && reviewable_entry(entry)
}

#[must_use]
pub(crate) fn reviewable_unstaged(entry: &GitStatusEntry) -> bool {
    entry.unstaged && reviewable_entry(entry)
}

pub(super) fn sync_actions(state: &SourceStateRef) {
    let (
        staged,
        unstaged,
        repo,
        status_stale,
        mutation_active,
        snapshot_current,
        snapshot,
        attrs,
        workspace,
    ) = {
        let state = state.borrow();
        (
            state.review_staged_action.clone(),
            state.review_unstaged_action.clone(),
            state.repo.clone(),
            state.status_stale,
            state.operations.mutation_active(),
            state
                .snapshot_id
                .as_ref()
                .is_some_and(|snapshot| state.operations.is_snapshot_current(snapshot)),
            state.snapshot.clone(),
            state.attrs.clone(),
            state.workspace.upgrade(),
        )
    };
    let dirty_uris = dirty_open_uris(workspace.as_ref());
    staged.set_enabled(can_review(
        repo.as_deref(),
        status_stale || !snapshot_current,
        mutation_active,
        &snapshot,
        &attrs,
        ReviewKind::Staged,
        &dirty_uris,
    ));
    unstaged.set_enabled(can_review(
        repo.as_deref(),
        status_stale || !snapshot_current,
        mutation_active,
        &snapshot,
        &attrs,
        ReviewKind::Unstaged,
        &dirty_uris,
    ));
}

fn add_review_action(
    state: &SourceStateRef,
    window: &impl IsA<gio::ActionMap>,
    action: &gio::SimpleAction,
    kind: ReviewKind,
) {
    let weak = Rc::downgrade(state);
    action.connect_activate(move |_, _| {
        if let Some(state) = weak.upgrade() {
            open_review(&state, kind);
        }
    });
    window.add_action(action);
}

fn open_review(state: &SourceStateRef, kind: ReviewKind) {
    let build = match build_spec(state, kind) {
        Ok(Some(build)) => build,
        Ok(None) => {
            show_empty_toast(state, kind);
            return;
        }
        Err(message) => {
            set_status_label(state, &message);
            return;
        }
    };
    let Some(workspace) = state.borrow().workspace.upgrade() else {
        return;
    };
    let tab = EditorTab::new_git_review(&workspace.settings, build.spec.clone());
    workspace.add_tab(tab.clone(), true);
    review_loader::start(state, &tab, build.spec, build.entries, build.snapshot_id);
}

pub(super) fn refresh_open_review(state: &SourceStateRef, tab: &Rc<EditorTab>) {
    let Some(spec) = tab.review_spec() else {
        return;
    };
    let repo = state.borrow().repo.clone();
    if repo.as_deref() != Some(spec.repo_root.as_path()) {
        return;
    }
    let build = match build_spec(state, spec.review_kind) {
        Ok(Some(build)) => build,
        Ok(None) => {
            show_empty_toast(state, spec.review_kind);
            return;
        }
        Err(message) => {
            set_status_label(state, &message);
            return;
        }
    };
    review_loader::start(state, tab, build.spec, build.entries, build.snapshot_id);
}

fn build_spec(state: &SourceStateRef, kind: ReviewKind) -> Result<Option<ReviewBuild>, String> {
    let (
        repo,
        status_stale,
        mutation_active,
        snapshot,
        attrs,
        snapshot_id,
        snapshot_current,
        generation,
        settings,
        workspace,
    ) = {
        let state = state.borrow();
        let snapshot_id = state.snapshot_id.clone();
        let snapshot_current = snapshot_id
            .as_ref()
            .is_some_and(|snapshot| state.operations.is_snapshot_current(snapshot));
        (
            state.repo.clone(),
            state.status_stale,
            state.operations.mutation_active(),
            state.snapshot.clone(),
            state.attrs.clone(),
            snapshot_id,
            snapshot_current,
            state.review_generation,
            state.settings.clone(),
            state.workspace.upgrade(),
        )
    };
    let Some(repo) = repo else {
        return Err(gettext("No Git repository is active."));
    };
    if status_stale || mutation_active {
        return Err(gettext("Refresh Source Control before reviewing changes."));
    }
    let Some(snapshot_id) = snapshot_id else {
        return Err(gettext("Refresh Source Control before reviewing changes."));
    };
    if !snapshot_current {
        return Err(gettext("Refresh Source Control before reviewing changes."));
    }
    if snapshot.too_large {
        return Err(gettext("Too many Git changes to display."));
    }
    if attrs.is_unavailable() {
        return Err(git_attrs_unavailable_text());
    }
    let dirty_uris = dirty_open_uris(workspace.as_ref());
    let entries = review_entries(&snapshot.entries, &attrs, repo.as_path(), kind, &dirty_uris);
    if entries.is_empty() {
        return Ok(None);
    }
    let files = entries
        .iter()
        .map(|entry| ReviewFileSpec::new(entry.path.raw().to_vec()))
        .collect();
    let fingerprint = match kind {
        ReviewKind::Staged => {
            fingerprint_for_staged(&repo, snapshot.head_oid.as_deref(), &attrs, &entries)
        }
        ReviewKind::Unstaged => fingerprint_for_unstaged(generation),
    };
    Ok(Some(ReviewBuild {
        spec: ReviewTabSpec::new(
            kind,
            repo,
            generation,
            fingerprint,
            files,
            settings.compare_review_settings_snapshot(),
        ),
        entries,
        snapshot_id,
    }))
}

fn can_review(
    repo: Option<&Path>,
    stale: bool,
    mutation: bool,
    snapshot: &crate::git_status::GitStatusSnapshot,
    attrs: &GitAttrState,
    kind: ReviewKind,
    dirty_uris: &[String],
) -> bool {
    if stale || mutation || snapshot.too_large || attrs.is_unavailable() {
        return false;
    }
    let Some(repo) = repo else {
        return false;
    };
    snapshot.entries.iter().any(|entry| {
        let reviewable = match kind {
            ReviewKind::Staged => reviewable_staged(entry),
            ReviewKind::Unstaged => {
                reviewable_unstaged(entry) && !entry_is_dirty(repo, entry, dirty_uris)
            }
        };
        reviewable && !attrs.blocks(entry.path.raw())
    })
}

fn show_empty_toast(state: &SourceStateRef, kind: ReviewKind) {
    let workspace = state.borrow().workspace.upgrade();
    let Some(workspace) = workspace else {
        return;
    };
    let message = match kind {
        ReviewKind::Staged => gettext("No staged changes to review."),
        ReviewKind::Unstaged => gettext("No unstaged changes to review."),
    };
    workspace.show_toast(&message);
}

#[cfg(test)]
fn review_files(
    entries: &[GitStatusEntry],
    attrs: &GitAttrState,
    repo: &Path,
    kind: ReviewKind,
    dirty_uris: &[String],
) -> Vec<ReviewFileSpec> {
    review_entries(entries, attrs, repo, kind, dirty_uris)
        .into_iter()
        .map(|entry| ReviewFileSpec::new(entry.path.raw().to_vec()))
        .collect()
}

fn review_entries(
    entries: &[GitStatusEntry],
    attrs: &GitAttrState,
    repo: &Path,
    kind: ReviewKind,
    dirty_uris: &[String],
) -> Vec<GitStatusEntry> {
    entries
        .iter()
        .filter(|entry| match kind {
            ReviewKind::Staged => reviewable_staged(entry),
            ReviewKind::Unstaged => {
                reviewable_unstaged(entry) && !entry_is_dirty(repo, entry, dirty_uris)
            }
        })
        .filter(|entry| !attrs.blocks(entry.path.raw()))
        .cloned()
        .collect()
}

fn reviewable_entry(entry: &GitStatusEntry) -> bool {
    !matches!(
        entry.status,
        GitFileStatus::Conflicted | GitFileStatus::Unsupported
    ) && !entry.worktree_mode.blocks_actions(entry.status)
}

fn dirty_open_uris(workspace: Option<&Rc<Workspace>>) -> Vec<String> {
    let Some(workspace) = workspace else {
        return Vec::new();
    };
    workspace
        .ordered_tabs()
        .into_iter()
        .filter(|tab| tab.is_dirty())
        .filter_map(|tab| tab.document_uri())
        .collect()
}

fn entry_is_dirty(repo: &Path, entry: &GitStatusEntry, dirty_uris: &[String]) -> bool {
    let Some(path) = entry.path.as_utf8() else {
        return false;
    };
    let uri = gio::File::for_path(repo.join(path)).uri().to_string();
    dirty_uris.iter().any(|dirty| dirty == &uri)
}

pub(crate) fn fingerprint_for_staged(
    repo: &Path,
    head_oid: Option<&str>,
    attrs: &GitAttrState,
    staged_entries: &[GitStatusEntry],
) -> ReviewSnapshotFingerprint {
    let mut token = String::new();
    let _ignored = write!(
        token,
        "kind=staged\nrepo={}\nattrs={attrs:?}\nhead={head_oid:?}\n",
        repo.display(),
    );
    for entry in staged_entries {
        append_staged_entry(&mut token, entry);
    }
    ReviewSnapshotFingerprint::new(token)
}

#[must_use]
pub(crate) fn fingerprint_for_unstaged(review_generation: u64) -> ReviewSnapshotFingerprint {
    ReviewSnapshotFingerprint::new(format!("unstaged-{review_generation}"))
}

pub(super) fn mark_open_reviews(state: &SourceStateRef) {
    let (workspace, repo, snapshot, attrs, generation) = {
        let state = state.borrow();
        (
            state.workspace.upgrade(),
            state.repo.clone(),
            state.snapshot.clone(),
            state.attrs.clone(),
            state.review_generation,
        )
    };
    let Some(workspace) = workspace else {
        return;
    };
    let Some(repo) = repo else {
        return;
    };
    for tab in workspace.ordered_tabs() {
        let Some(spec) = tab.review_spec() else {
            continue;
        };
        if spec.repo_root != repo {
            continue;
        }
        if snapshot.too_large {
            let fingerprint = ReviewSnapshotFingerprint::new(format!("too-large-{generation}"));
            let _stale = tab.mark_review_stale_if_mismatch(&fingerprint, generation);
            continue;
        }
        let fingerprint = match spec.review_kind {
            ReviewKind::Staged => {
                let entries =
                    review_entries(&snapshot.entries, &attrs, &repo, ReviewKind::Staged, &[]);
                fingerprint_for_staged(&repo, snapshot.head_oid.as_deref(), &attrs, &entries)
            }
            ReviewKind::Unstaged => fingerprint_for_unstaged(generation),
        };
        let _stale = tab.mark_review_stale_if_mismatch(&fingerprint, generation);
    }
}

#[cfg(test)]
pub(crate) fn review_fingerprint(
    repo: &Path,
    snapshot: &GitStatusSnapshot,
    attrs: &GitAttrState,
    kind: ReviewKind,
    review_generation: u64,
) -> ReviewSnapshotFingerprint {
    match kind {
        ReviewKind::Staged => {
            let entries = snapshot
                .entries
                .iter()
                .filter(|entry| entry.staged)
                .cloned()
                .collect::<Vec<_>>();
            fingerprint_for_staged(repo, snapshot.head_oid.as_deref(), attrs, &entries)
        }
        ReviewKind::Unstaged => fingerprint_for_unstaged(review_generation),
    }
}

fn append_staged_entry(token: &mut String, entry: &GitStatusEntry) {
    let _ignored = write!(
        token,
        "path={:?}\nstatus={:?}\nmode={:?}\nhead={:?}\nindex={:?}\n",
        entry.path.raw(),
        entry.status,
        entry.worktree_mode,
        entry.head_oid,
        entry.index_oid
    );
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::editor_tab::ReviewKind;
    use crate::git_status::{
        GitAttrState, GitAttrs, GitFileStatus, GitPath, GitStatusEntry, GitStatusSnapshot,
        GitWorktreeMode,
    };
    use crate::source_control::review::{
        review_files, review_fingerprint, reviewable_staged, reviewable_unstaged,
    };

    #[test]
    fn review_predicates_match_review_kind() {
        let staged = entry("staged.txt", true, false);
        let unstaged = entry("unstaged.txt", false, true);
        let conflict = entry_with_status("both.txt", GitFileStatus::Conflicted, true, true);

        assert!(reviewable_staged(&staged));
        assert!(!reviewable_unstaged(&staged));
        assert!(reviewable_unstaged(&unstaged));
        assert!(!reviewable_staged(&unstaged));
        assert!(!reviewable_staged(&conflict));
        assert!(!reviewable_unstaged(&conflict));
    }

    #[test]
    fn raw_paths_are_preserved_in_specs() {
        let entries = vec![entry_bytes(b"dir/\xff.bin", true, false)];
        let files = review_files(
            &entries,
            &GitAttrState::Known(GitAttrs::default()),
            Path::new("/repo"),
            ReviewKind::Staged,
            &[],
        );

        assert_eq!(files[0].raw_path, b"dir/\xff.bin");

        let entries = vec![entry_bytes(b"dir/file.txt", true, false)];
        let files = review_files(
            &entries,
            &GitAttrState::Known(GitAttrs::default()),
            Path::new("/repo"),
            ReviewKind::Staged,
            &[],
        );
        assert_eq!(files[0].raw_path, b"dir/file.txt");
    }

    #[test]
    fn fingerprint_is_kind_specific() {
        let snapshot = GitStatusSnapshot {
            head_oid: Some(String::from("head")),
            entries: vec![entry("tracked.txt", true, true)],
            ..GitStatusSnapshot::default()
        };
        let attrs = GitAttrState::Known(GitAttrs::default());
        let staged =
            review_fingerprint(Path::new("/repo"), &snapshot, &attrs, ReviewKind::Staged, 1);
        let unstaged = review_fingerprint(
            Path::new("/repo"),
            &snapshot,
            &attrs,
            ReviewKind::Unstaged,
            1,
        );

        assert_ne!(staged, unstaged);
        assert!(staged.token().contains("index"));
        assert!(staged.token().contains("116, 114, 97, 99, 107, 101, 100"));
        assert!(!staged.token().contains("unstaged=true"));
    }

    fn entry(path: &str, staged: bool, unstaged: bool) -> GitStatusEntry {
        entry_bytes(path.as_bytes(), staged, unstaged)
    }

    fn entry_with_status(
        path: &str,
        status: GitFileStatus,
        staged: bool,
        unstaged: bool,
    ) -> GitStatusEntry {
        let mut entry = entry(path, staged, unstaged);
        entry.status = status;
        entry
    }

    fn entry_bytes(path: &[u8], staged: bool, unstaged: bool) -> GitStatusEntry {
        GitStatusEntry::with_worktree_mode(
            GitPath::from_bytes(path),
            GitFileStatus::Modified,
            Some(String::from("head")),
            Some(String::from("index")),
            staged,
            unstaged,
            GitWorktreeMode::Regular("100644"),
        )
    }
}
