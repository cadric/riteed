use std::path::Path;

use gettextrs::{gettext, pgettext};
use gtk4::{gio, prelude::*};

use crate::git_process::GitProcessError;
use crate::git_status::{
    GitActionState, GitAttrState, GitFileStatus, GitStatusEntry, GitStatusSnapshot, GitWorktreeMode,
};
use crate::source_control::git_attrs_unavailable_text;

pub(super) fn commit_sensitive(
    snapshot: &GitStatusSnapshot,
    attrs: &GitAttrState,
    writes_enabled: bool,
) -> bool {
    writes_enabled
        && !snapshot.too_large
        && !attrs.is_unavailable()
        && snapshot
            .entries
            .iter()
            .any(|entry| entry.staged && entry.unstage_action.enabled())
}

pub(super) fn discard_state(entry: &GitStatusEntry) -> GitActionState {
    if entry.status == GitFileStatus::Untracked {
        GitActionState::Disabled(pgettext("git action disabled", "Untracked file"))
    } else if !entry.unstaged {
        GitActionState::Disabled(pgettext("git action disabled", "No unstaged change"))
    } else {
        GitActionState::Enabled
    }
}

pub(super) fn entry_disabled_reason(
    repo: Option<&Path>,
    entry: &GitStatusEntry,
    attrs: &GitAttrState,
    dirty_uris: &[String],
) -> Option<String> {
    if entry.path.as_utf8().is_none() {
        return Some(gettext("This path uses an unsupported encoding."));
    }
    if matches!(
        entry.status,
        GitFileStatus::Conflicted | GitFileStatus::Unsupported
    ) {
        return Some(gettext(
            "This Git state is visible but not editable in Riteed.",
        ));
    }
    if attrs.is_unavailable() {
        return Some(git_attrs_unavailable_text());
    }
    if attrs.blocks(entry.path.raw()) {
        return Some(gettext(
            "Git content filters or EOL conversion are configured for this file.",
        ));
    }
    let Some((repo, path)) = repo.zip(entry.path.as_utf8()) else {
        return Some(gettext("No Git repository is active."));
    };
    if entry.worktree_mode.blocks_actions(entry.status) {
        return Some(gettext(
            "Directories, symlinks, and unsupported file modes are visible only.",
        ));
    }
    let uri = gio::File::for_path(repo.join(path)).uri().to_string();
    if dirty_uris.iter().any(|dirty| dirty == &uri) {
        return Some(gettext("Save the open document before using Git actions."));
    }
    None
}

pub(super) fn too_many_changes_text() -> String {
    gettext("Too many Git changes to display.")
}

pub(super) fn should_stage_delete(entry: &GitStatusEntry) -> bool {
    entry.status == GitFileStatus::Deleted
        && entry.unstaged
        && entry.worktree_mode == GitWorktreeMode::Absent
}

pub(super) fn stage_mode_for_entry(entry: &GitStatusEntry) -> Option<&'static str> {
    entry.worktree_mode.stage_mode()
}

pub(super) fn reference_oid(entry: &GitStatusEntry) -> Option<String> {
    if entry.staged && !entry.unstaged {
        return entry.head_oid.clone();
    }
    entry.index_oid.clone().or_else(|| entry.head_oid.clone())
}

pub(super) fn reference_text(output: Vec<u8>) -> Result<String, GitProcessError> {
    if output.contains(&0) {
        return Err(GitProcessError::BinaryContent);
    }
    String::from_utf8(output).map_err(|_| GitProcessError::ParseFailed)
}
