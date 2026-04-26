use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib};

use super::support::{base_args, redact_git_argv};
use super::{GitCallback, GitIdentity, GitProcess, GitProcessError};
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
fn redaction_hides_identity_config_values() {
    let argv = vec![
        String::from("-c"),
        String::from("user.name=Ada"),
        String::from("-c"),
        String::from("user.email=ada@example.test"),
    ];
    assert_eq!(
        redact_git_argv(&argv),
        vec![
            String::from("-c"),
            String::from("user.name=<redacted>"),
            String::from("-c"),
            String::from("user.email=<redacted>"),
        ]
    );
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
    assert!(repo.ends_with("riteed"));
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
}

#[test]
fn typed_ops_reject_invalid_inputs_before_spawning() {
    let process = GitProcess::new(PathBuf::from("/tmp"));
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
        while glib::MainContext::default().iteration(false) {}
        if slot.borrow().is_some() {
            break;
        }
        let fired = Rc::new(Cell::new(false));
        let fired_for_timeout = Rc::clone(&fired);
        let source = glib::timeout_add_local_once(Duration::from_millis(10), move || {
            fired_for_timeout.set(true);
        });
        while !fired.get() && slot.borrow().is_none() {
            let _dispatched = glib::MainContext::default().iteration(true);
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
