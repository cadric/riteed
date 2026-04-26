use std::fs;
use std::path::PathBuf;

use super::{entry_disabled_reason, mode_for_path, reference_oid, reference_text};
use crate::git_process::GitProcessError;
use crate::git_status::{GitAttrs, GitFileStatus, GitPath, GitStatusEntry};

#[test]
fn disabled_reasons_cover_unsupported_paths_and_modes() {
    let repo = temp_repo("riteed-git-actions-disabled");
    assert!(fs::write(repo.join("tracked.txt"), b"tracked").is_ok());
    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry_bytes(b"\xff", GitFileStatus::Modified),
            &GitAttrs::default(),
            &[],
        )
        .as_deref(),
        Some("This path uses an unsupported encoding.")
    );
    assert_eq!(
        entry_disabled_reason(
            Some(&repo),
            &entry("tracked.txt", GitFileStatus::Conflicted, true, true),
            &GitAttrs::default(),
            &[],
        )
        .as_deref(),
        Some("This Git state is visible but not editable in Riteed.")
    );
    assert_eq!(
        entry_disabled_reason(
            None,
            &entry("tracked.txt", GitFileStatus::Modified, true, true),
            &GitAttrs::default(),
            &[],
        )
        .as_deref(),
        Some("No Git repository is active.")
    );

    #[cfg(unix)]
    {
        let link = repo.join("linked.txt");
        let _removed = fs::remove_file(&link);
        assert!(std::os::unix::fs::symlink(repo.join("tracked.txt"), &link).is_ok());
        assert_eq!(
            entry_disabled_reason(
                Some(&repo),
                &entry("linked.txt", GitFileStatus::Modified, true, true),
                &GitAttrs::default(),
                &[],
            )
            .as_deref(),
            Some("Symlinks and unsupported file modes are visible only.")
        );
    }
}

#[test]
fn file_modes_and_reference_text_are_guarded() {
    let repo = temp_repo("riteed-git-actions-modes");
    let script = repo.join("script.sh");
    let dir = repo.join("dir");
    assert!(fs::write(&script, b"#!/bin/sh\n").is_ok());
    assert!(fs::create_dir_all(&dir).is_ok());
    assert_eq!(
        mode_for_path(&repo, &GitPath::from_bytes(b"script.sh")),
        Some("100644")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let metadata = fs::metadata(&script);
        assert!(metadata.is_ok());
        if let Ok(metadata) = metadata {
            let mut permissions = metadata.permissions();
            permissions.set_mode(0o755);
            assert!(fs::set_permissions(&script, permissions).is_ok());
        }
        assert_eq!(
            mode_for_path(&repo, &GitPath::from_bytes(b"script.sh")),
            Some("100755")
        );
    }
    assert_eq!(mode_for_path(&repo, &GitPath::from_bytes(b"dir")), None);
    assert_eq!(
        reference_text(b"hello".to_vec()).ok().as_deref(),
        Some("hello")
    );
    assert!(matches!(
        reference_text(vec![0]),
        Err(GitProcessError::OutputTooLarge)
    ));
    assert!(matches!(
        reference_text(vec![0xff]),
        Err(GitProcessError::ParseFailed)
    ));
}

#[test]
fn reference_oid_prefers_the_expected_side() {
    let staged = entry("tracked.txt", GitFileStatus::Modified, true, false);
    assert_eq!(reference_oid(&staged).as_deref(), Some("head"));
    let unstaged = entry("tracked.txt", GitFileStatus::Modified, false, true);
    assert_eq!(reference_oid(&unstaged).as_deref(), Some("index"));
}

fn entry(path: &str, status: GitFileStatus, staged: bool, unstaged: bool) -> GitStatusEntry {
    GitStatusEntry::new(
        GitPath::from_bytes(path.as_bytes()),
        status,
        Some(String::from("head")),
        Some(String::from("index")),
        staged,
        unstaged,
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
