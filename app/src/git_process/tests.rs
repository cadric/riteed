use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib};

use super::support::base_args;
use super::{
    GIT_CANCEL_KILL_GRACE, GIT_OPERATION_TIMEOUT, GitCallback, GitIdentity, GitProcess,
    GitProcessError, GitRepoContext, GitSpec, run_git,
};
use crate::git_status::GitPath;

#[test]
fn base_args_include_git_safety_flags() {
    let args = base_args();
    assert!(args.iter().any(|arg| arg == "--no-pager"));
    assert!(args.iter().any(|arg| arg == "--no-optional-locks"));
    assert!(args.iter().any(|arg| arg == "core.fsmonitor=false"));
}

#[test]
fn identity_rejects_config_injection_bytes() {
    assert!(GitIdentity::new(String::from("Ada"), String::from("ada@example.test")).is_ok());
    assert!(GitIdentity::new(String::from("Ada\nBad"), String::from("ada@example.test")).is_err());
    assert!(GitIdentity::new(String::from("Ada"), String::from("a\rb")).is_err());
    assert!(GitIdentity::new(String::from("Ada"), String::from("a\0b")).is_err());
}

#[test]
fn git_operations_have_wall_clock_timeout_and_kill_grace() {
    assert_eq!(GIT_OPERATION_TIMEOUT, Duration::from_secs(30));
    assert_eq!(GIT_CANCEL_KILL_GRACE, Duration::from_secs(2));
}

#[test]
fn detect_repo_does_not_retry_cancel_or_timeout() {
    let source = include_str!("../git_process.rs");
    assert!(source.contains("GitProcessError::Cancelled | GitProcessError::TimedOut"));
    assert!(source.contains("callback(Err(error));"));
}

#[test]
fn read_only_git_ops_work_against_current_repo() {
    let _guard = crate::test_support::lock_for_tests();
    let app_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = GitPath::from_bytes(b"app/Cargo.toml");

    let detected = wait_git(|cancellable, callback| {
        GitProcess::detect_repo(&app_dir, cancellable, callback);
    });
    let Ok(repo) = detected else {
        return;
    };
    assert!(repo.work_tree.join("app/Cargo.toml").is_file());
    let process = GitProcess::new(repo);

    let status = wait_git(|cancellable, callback| process.status(cancellable, callback));
    assert!(status.is_ok());

    let capabilities = wait_git(|cancellable, callback| {
        process.check_repo_capabilities(cancellable, callback);
    });
    assert!(capabilities.is_ok());

    let attrs = wait_git(|cancellable, callback| {
        process.check_attrs(std::slice::from_ref(&path), cancellable, callback);
    });
    assert!(attrs.is_ok());

    let head_entry = wait_git(|cancellable, callback| {
        process.read_head_index_entry(&path, cancellable, callback);
    });
    assert!(matches!(head_entry, Ok(Some(_))));
    let oid = match head_entry {
        Ok(Some((_mode, oid))) => oid,
        Ok(None) | Err(_) => String::new(),
    };
    if oid.is_empty() {
        return;
    }
    let blob = wait_git(|cancellable, callback| process.cat_blob(&oid, cancellable, callback));
    assert!(blob.is_ok_and(|bytes| !bytes.is_empty()));

    let identity =
        wait_git(|cancellable, callback| process.read_git_identity(cancellable, callback));
    assert!(identity.is_ok());

    let commits =
        wait_git(|cancellable, callback| process.recent_commits(25, cancellable, callback));
    assert!(commits.is_ok());
}

#[test]
fn status_expands_untracked_directories_to_files() {
    let _guard = crate::test_support::lock_for_tests();
    let repo = temp_repo("riteed-git-process-untracked-files");
    assert!(run_git_command(&repo, ["init"]).is_ok());
    assert!(fs::create_dir_all(repo.join("apps/desktop/src/main/kotlin")).is_ok());
    let nested = repo.join("apps/desktop/src/main/kotlin/Main.kt");
    assert!(fs::write(&nested, b"fun main() = Unit\n").is_ok());

    let detected = wait_git(|cancellable, callback| {
        GitProcess::detect_repo(&repo, cancellable, callback);
    });
    assert!(detected.is_ok());
    let Ok(context) = detected else {
        let _removed = fs::remove_dir_all(&repo);
        return;
    };
    let process = GitProcess::new(context);
    let status = wait_git(|cancellable, callback| process.status(cancellable, callback));
    let _removed = fs::remove_dir_all(&repo);
    assert!(status.is_ok());
    let Ok(snapshot) = status else {
        return;
    };

    assert!(
        snapshot
            .entries
            .iter()
            .any(|entry| entry.path.as_utf8() == Some("apps/desktop/src/main/kotlin/Main.kt"))
    );
    assert!(
        !snapshot
            .entries
            .iter()
            .any(|entry| entry.path.as_utf8() == Some("apps/"))
    );
}

#[test]
fn status_keeps_nested_untracked_repositories_as_directories() {
    let _guard = crate::test_support::lock_for_tests();
    let repo = temp_repo("riteed-git-process-nested-repo-parent");
    let nested = repo.join("RedReader");
    assert!(run_git_command(&repo, ["init"]).is_ok());
    assert!(fs::create_dir_all(&nested).is_ok());
    assert!(run_git_command(&nested, ["init"]).is_ok());
    assert!(fs::write(nested.join("README.md"), b"# Nested\n").is_ok());

    let detected = wait_git(|cancellable, callback| {
        GitProcess::detect_repo(&repo, cancellable, callback);
    });
    assert!(detected.is_ok());
    let Ok(context) = detected else {
        let _removed = fs::remove_dir_all(&repo);
        return;
    };
    let process = GitProcess::new(context);
    let status = wait_git(|cancellable, callback| process.status(cancellable, callback));
    let _removed = fs::remove_dir_all(&repo);
    assert!(status.is_ok());
    let Ok(snapshot) = status else {
        return;
    };

    assert!(
        snapshot
            .entries
            .iter()
            .any(|entry| entry.path.as_utf8() == Some("RedReader/")
                && entry.worktree_mode == crate::git_status::GitWorktreeMode::Directory)
    );
    assert!(
        !snapshot
            .entries
            .iter()
            .any(|entry| entry.path.as_utf8() == Some("RedReader/README.md"))
    );
}

#[test]
fn typed_ops_reject_invalid_inputs_before_spawning() {
    let process = GitProcess::new(context_for(Path::new("/tmp")));
    let bad_path = GitPath::from_bytes(b"\xff");
    let cancellable = gio::Cancellable::new();

    let hash = immediate_result(|callback| {
        process.hash_file_no_filters(&bad_path, &cancellable, callback);
    });
    assert!(matches!(hash, Some(Err(GitProcessError::InvalidPath))));

    let head = immediate_result(|callback| {
        process.read_head_index_entry(&bad_path, &cancellable, callback);
    });
    assert!(matches!(head, Some(Err(GitProcessError::InvalidPath))));

    let remove = immediate_result(|callback| {
        process.remove_from_index(&bad_path, &cancellable, callback);
    });
    assert!(matches!(remove, Some(Err(GitProcessError::InvalidPath))));

    let restore = immediate_result(|callback| {
        process.restore_worktree_path(&bad_path, &cancellable, callback);
    });
    assert!(matches!(restore, Some(Err(GitProcessError::InvalidPath))));

    let invalid_identity = GitIdentity {
        name: String::from("Ada\nBad"),
        email: String::from("ada@example.test"),
    };
    let commit = immediate_result(|callback| {
        process.commit(&invalid_identity, "message", &cancellable, callback);
    });
    assert!(matches!(
        commit,
        Some(Err(GitProcessError::InvalidIdentity))
    ));
}

#[test]
fn run_specs_use_resolved_git_env_without_joining_dot_git() {
    let context = GitRepoContext {
        work_tree: PathBuf::from("/tmp/worktree"),
        git_dir: PathBuf::from("/tmp/common/worktrees/worktree"),
        git_common_dir: PathBuf::from("/tmp/common"),
        head_path: PathBuf::from("/tmp/common/worktrees/worktree/HEAD"),
        index_path: PathBuf::from("/tmp/common/worktrees/worktree/index"),
        index_lock_path: PathBuf::from("/tmp/common/worktrees/worktree/index.lock"),
        refs_heads_dir: PathBuf::from("/tmp/common/refs/heads"),
        packed_refs_path: PathBuf::from("/tmp/common/packed-refs"),
    };
    let process = GitProcess::new(context);
    let spec_result = process.spec(["status"], None, 4096, false);
    assert!(spec_result.is_ok());
    let Ok(spec) = spec_result else {
        return;
    };
    assert!(spec.argv.iter().any(|arg| arg == "status"));
    assert!(spec.env.contains(&(
        String::from("GIT_DIR"),
        String::from("/tmp/common/worktrees/worktree")
    )));
    assert!(
        spec.env
            .contains(&(String::from("GIT_WORK_TREE"), String::from("/tmp/worktree")))
    );
    assert!(
        !spec
            .env
            .iter()
            .any(|(_, value)| value == "/tmp/worktree/.git")
    );
}

#[test]
fn linked_worktree_detection_uses_resolved_git_dir() {
    let _guard = crate::test_support::lock_for_tests();
    let main = temp_repo("riteed-git-process-main");
    let linked = std::env::temp_dir().join("riteed-git-process-linked");
    let _removed = fs::remove_dir_all(&linked);
    assert!(run_git_command(&main, ["init"]).is_ok());
    assert!(fs::write(main.join("tracked.txt"), b"tracked").is_ok());
    assert!(run_git_command(&main, ["add", "tracked.txt"]).is_ok());
    assert!(
        run_git_command(
            &main,
            [
                "-c",
                "user.name=Riteed",
                "-c",
                "user.email=riteed@example.test",
                "commit",
                "--no-gpg-sign",
                "-m",
                "initial",
            ],
        )
        .is_ok()
    );
    let Some(linked_arg) = linked.to_str() else {
        let _removed = fs::remove_dir_all(&main);
        return;
    };
    assert!(run_git_command(&main, ["worktree", "add", linked_arg]).is_ok());

    let detected = wait_git(|cancellable, callback| {
        GitProcess::detect_repo(&linked, cancellable, callback);
    });
    let Ok(context) = detected else {
        let _removed = run_git_command(&main, ["worktree", "remove", "--force", linked_arg]);
        let _removed = fs::remove_dir_all(&main);
        return;
    };
    assert_ne!(context.git_dir, linked.join(".git"));
    let process = GitProcess::new(context);
    let status = wait_git(|cancellable, callback| process.status(cancellable, callback));
    assert!(status.is_ok());

    let _removed = run_git_command(&main, ["worktree", "remove", "--force", linked_arg]);
    let _removed = fs::remove_dir_all(&main);
}

fn immediate_result<T: 'static>(
    start: impl FnOnce(GitCallback<T>),
) -> Option<Result<T, GitProcessError>> {
    let slot: Rc<RefCell<Option<Result<T, GitProcessError>>>> = Rc::new(RefCell::new(None));
    let slot_for_callback = Rc::clone(&slot);
    start(Rc::new(move |result| {
        *slot_for_callback.borrow_mut() = Some(result);
    }));
    slot.borrow_mut().take()
}

fn wait_git<T: 'static>(
    start: impl FnOnce(&gio::Cancellable, GitCallback<T>),
) -> Result<T, GitProcessError> {
    let context = glib::MainContext::default();
    let Ok(_guard) = context.acquire() else {
        return Err(GitProcessError::Cancelled);
    };
    let slot: Rc<RefCell<Option<Result<T, GitProcessError>>>> = Rc::new(RefCell::new(None));
    let slot_for_callback = Rc::clone(&slot);
    let cancellable = gio::Cancellable::new();
    start(
        &cancellable,
        Rc::new(move |result| {
            *slot_for_callback.borrow_mut() = Some(result);
        }),
    );
    for _ in 0..600 {
        while context.iteration(false) {}
        if slot.borrow().is_some() {
            break;
        }
        let fired = Rc::new(Cell::new(false));
        let fired_for_timeout = Rc::clone(&fired);
        let source = glib::timeout_add_local_once(Duration::from_millis(10), move || {
            fired_for_timeout.set(true);
        });
        while !fired.get() && slot.borrow().is_none() {
            let _dispatched = context.iteration(true);
        }
        if !fired.get() {
            source.remove();
        }
    }
    let result = slot.borrow_mut().take();
    let Some(result) = result else {
        return Err(GitProcessError::Cancelled);
    };
    result
}

fn run_git_command<const N: usize>(
    directory: &Path,
    command_args: [&str; N],
) -> Result<(), GitProcessError> {
    let Some(directory) = directory.to_str() else {
        return Err(GitProcessError::InvalidPath);
    };
    let mut git_argv = base_args();
    git_argv.extend(["-C", directory].map(String::from));
    git_argv.extend(command_args.map(String::from));
    wait_git(|cancellable, callback| {
        run_git(
            GitSpec {
                argv: git_argv,
                env: Vec::new(),
                stdin: None,
                stdout_cap: 256 * 1024,
                allow_failure: false,
            },
            cancellable,
            Rc::new(move |result| callback(result.map(|_output| ()))),
        );
    })
}

fn context_for(work_tree: &Path) -> GitRepoContext {
    let git = work_tree.join(".git");
    GitRepoContext {
        work_tree: work_tree.to_path_buf(),
        git_dir: git.clone(),
        git_common_dir: git.clone(),
        head_path: git.join("HEAD"),
        index_path: git.join("index"),
        index_lock_path: git.join("index.lock"),
        refs_heads_dir: git.join("refs/heads"),
        packed_refs_path: git.join("packed-refs"),
    }
}

fn temp_repo(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _removed = fs::remove_dir_all(&path);
    assert!(fs::create_dir_all(&path).is_ok());
    path
}
