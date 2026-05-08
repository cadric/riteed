use std::fs;
use std::path::PathBuf;

use gtk4::prelude::FileExt;

use super::{
    commit_sensitive, discard_state, entry_disabled_reason, reference_oid, reference_text,
    should_stage_delete, stage_mode_for_entry,
};
use crate::git_process::GitProcessError;
use crate::git_status::{
    GitActionState, GitAttrState, GitAttrs, GitFileStatus, GitPath, GitStatusEntry,
    GitStatusSnapshot, GitWorktreeMode,
};

#[test]
fn disabled_reasons_cover_unsupported_paths_and_modes() {
    let repo = temp_repo("riteed-git-actions-disabled");
    assert!(fs::write(repo.join("tracked.txt"), b"tracked").is_ok());
    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry_bytes(b"\xff", GitFileStatus::Modified),
            &known_attrs(),
            &[],
        )
        .as_deref(),
        Some("This path uses an unsupported encoding.")
    );
    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry("tracked.txt", GitFileStatus::Conflicted, true, true),
            &known_attrs(),
            &[],
        )
        .as_deref(),
        Some("This Git state is visible but not editable in Riteed.")
    );
    assert_eq!(
        entry_disabled_reason(
            None,
            &entry("tracked.txt", GitFileStatus::Modified, true, true),
            &known_attrs(),
            &[],
        )
        .as_deref(),
        Some("No Git repository is active.")
    );
    let dirty_uri = gtk4::gio::File::for_path(repo.join("tracked.txt"))
        .uri()
        .to_string();
    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry("tracked.txt", GitFileStatus::Modified, true, true),
            &known_attrs(),
            &[dirty_uri],
        )
        .as_deref(),
        Some("Save the open document before using Git actions.")
    );

    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry_with_mode(
                "missing-link.txt",
                GitFileStatus::Modified,
                true,
                true,
                GitWorktreeMode::Symlink,
            ),
            &known_attrs(),
            &[],
        )
        .as_deref(),
        Some("Symlinks and unsupported file modes are visible only.")
    );
}

#[test]
fn unavailable_attrs_disable_git_actions_and_commit() {
    let repo = temp_repo("riteed-git-actions-attrs");
    assert!(fs::write(repo.join("tracked.txt"), b"tracked").is_ok());
    let attrs = GitAttrState::Unavailable;
    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry("tracked.txt", GitFileStatus::Modified, true, true),
            &attrs,
            &[],
        )
        .as_deref(),
        Some("Unable to read Git attributes. Git actions are disabled.")
    );

    let mut staged = entry("tracked.txt", GitFileStatus::Modified, true, false);
    staged.unstage_action = GitActionState::Enabled;
    let snapshot = GitStatusSnapshot {
        entries: vec![staged],
        ..GitStatusSnapshot::default()
    };
    assert!(commit_sensitive(&snapshot, &known_attrs(), false));
    assert!(!commit_sensitive(&snapshot, &attrs, false));
    assert!(!commit_sensitive(&snapshot, &known_attrs(), true));
}

#[test]
fn commit_controls_require_committable_staged_changes() {
    let mut unstaged = entry("tracked.txt", GitFileStatus::Modified, false, true);
    unstaged.stage_action = GitActionState::Enabled;
    assert!(!commit_sensitive(
        &GitStatusSnapshot {
            entries: vec![unstaged],
            ..GitStatusSnapshot::default()
        },
        &known_attrs(),
        false,
    ));

    let mut staged = entry("tracked.txt", GitFileStatus::Modified, true, false);
    staged.unstage_action = GitActionState::Enabled;
    assert!(commit_sensitive(
        &GitStatusSnapshot {
            entries: vec![staged],
            ..GitStatusSnapshot::default()
        },
        &known_attrs(),
        false,
    ));
}

#[test]
fn file_modes_and_reference_text_are_guarded() {
    let unknown = entry_with_mode(
        "script.sh",
        GitFileStatus::Modified,
        true,
        true,
        GitWorktreeMode::Unknown,
    );
    assert_eq!(stage_mode_for_entry(&unknown), None);
    assert_eq!(
        reference_text(b"hello".to_vec()).ok().as_deref(),
        Some("hello")
    );
    assert_eq!(reference_text(Vec::new()).ok().as_deref(), Some(""));
    assert!(matches!(
        reference_text(vec![0]),
        Err(GitProcessError::BinaryContent)
    ));
    let mut boundary_nul = vec![b'a'; 8192];
    boundary_nul.push(0);
    assert!(matches!(
        reference_text(boundary_nul),
        Err(GitProcessError::BinaryContent)
    ));
    let mut trailing_nul = vec![b'a'; 16_384];
    if let Some(last) = trailing_nul.last_mut() {
        *last = 0;
    }
    assert!(matches!(
        reference_text(trailing_nul),
        Err(GitProcessError::BinaryContent)
    ));
    assert!(matches!(
        reference_text(vec![b'a', 0, b'b', 0]),
        Err(GitProcessError::BinaryContent)
    ));
    assert!(matches!(
        reference_text(vec![0xff]),
        Err(GitProcessError::ParseFailed)
    ));
}

#[test]
fn parsed_worktree_modes_drive_action_guards() {
    let missing_repo = temp_repo("riteed-git-actions-no-stat");
    let regular = entry_with_mode(
        "missing.txt",
        GitFileStatus::Modified,
        true,
        true,
        GitWorktreeMode::Regular("100644"),
    );
    assert!(entry_disabled_reason(Some(&missing_repo), &regular, &known_attrs(), &[]).is_none());
    assert_eq!(stage_mode_for_entry(&regular), Some("100644"));

    let absent_delete = entry_with_mode(
        "deleted.txt",
        GitFileStatus::Deleted,
        false,
        true,
        GitWorktreeMode::Absent,
    );
    assert!(
        entry_disabled_reason(Some(&missing_repo), &absent_delete, &known_attrs(), &[]).is_none()
    );
    assert!(should_stage_delete(&absent_delete));

    let staged_delete = entry_with_mode(
        "deleted.txt",
        GitFileStatus::Deleted,
        true,
        false,
        GitWorktreeMode::Absent,
    );
    assert!(
        entry_disabled_reason(Some(&missing_repo), &staged_delete, &known_attrs(), &[]).is_none()
    );
    assert!(!should_stage_delete(&staged_delete));
    assert_eq!(stage_mode_for_entry(&staged_delete), None);

    let recreated = entry_with_mode(
        "deleted.txt",
        GitFileStatus::Deleted,
        true,
        true,
        GitWorktreeMode::Regular("100755"),
    );
    assert!(!should_stage_delete(&recreated));
    assert_eq!(stage_mode_for_entry(&recreated), Some("100755"));

    let absent_modified = entry_with_mode(
        "missing.txt",
        GitFileStatus::Modified,
        true,
        true,
        GitWorktreeMode::Absent,
    );
    assert_eq!(
        entry_disabled_reason(Some(&missing_repo), &absent_modified, &known_attrs(), &[])
            .as_deref(),
        Some("Symlinks and unsupported file modes are visible only.")
    );
}

#[test]
fn reference_oid_prefers_the_expected_side() {
    let staged = entry("tracked.txt", GitFileStatus::Modified, true, false);
    assert_eq!(reference_oid(&staged).as_deref(), Some("head"));
    let unstaged = entry("tracked.txt", GitFileStatus::Modified, false, true);
    assert_eq!(reference_oid(&unstaged).as_deref(), Some("index"));
}

#[test]
fn discard_only_enables_tracked_unstaged_worktree_changes() {
    let tracked = entry("tracked.txt", GitFileStatus::Modified, false, true);
    assert!(discard_state(&tracked).enabled());

    let staged_only = entry("tracked.txt", GitFileStatus::Modified, true, false);
    assert!(!discard_state(&staged_only).enabled());

    let untracked = entry("new.txt", GitFileStatus::Untracked, false, true);
    assert!(!discard_state(&untracked).enabled());
}

fn known_attrs() -> GitAttrState {
    GitAttrState::Known(GitAttrs::default())
}

fn entry(path: &str, status: GitFileStatus, staged: bool, unstaged: bool) -> GitStatusEntry {
    entry_with_mode(
        path,
        status,
        staged,
        unstaged,
        GitWorktreeMode::Regular("100644"),
    )
}

fn entry_with_mode(
    path: &str,
    status: GitFileStatus,
    staged: bool,
    unstaged: bool,
    mode: GitWorktreeMode,
) -> GitStatusEntry {
    GitStatusEntry::with_worktree_mode(
        GitPath::from_bytes(path.as_bytes()),
        status,
        Some(String::from("head")),
        Some(String::from("index")),
        staged,
        unstaged,
        mode,
    )
}

fn entry_bytes(path: &[u8], status: GitFileStatus) -> GitStatusEntry {
    GitStatusEntry::new(
        GitPath::from_bytes(path),
        status,
        Some(String::from("head")),
        Some(String::from("index")),
        true,
        true,
    )
}

fn temp_repo(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _removed = fs::remove_dir_all(&path);
    assert!(fs::create_dir_all(&path).is_ok());
    path
}
