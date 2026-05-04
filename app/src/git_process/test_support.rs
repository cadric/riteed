use std::cell::{Cell, RefCell};
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib};

use super::support::base_args;
use super::{GitCallback, GitProcessError, GitSpec, run_git};

pub(crate) fn init_modified_fixture_repo_for_tests(
    directory: &Path,
    file_name: &str,
    baseline: &[u8],
    working: &[u8],
) -> Result<(), GitProcessError> {
    fs::create_dir_all(directory)
        .map_err(|error| GitProcessError::CommandFailed(error.to_string()))?;
    run_git_fixture_command(directory, &["init"])?;
    run_git_fixture_command(directory, &["config", "user.name", "Riteed Test"])?;
    run_git_fixture_command(
        directory,
        &["config", "user.email", "riteed-test@example.invalid"],
    )?;
    fs::write(directory.join(file_name), baseline)
        .map_err(|error| GitProcessError::CommandFailed(error.to_string()))?;
    run_git_fixture_command(directory, &["add", file_name])?;
    run_git_fixture_command(
        directory,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--no-gpg-sign",
            "-m",
            "baseline",
        ],
    )?;
    fs::write(directory.join(file_name), working)
        .map_err(|error| GitProcessError::CommandFailed(error.to_string()))
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

fn run_git_fixture_command(directory: &Path, command_args: &[&str]) -> Result<(), GitProcessError> {
    let Some(directory) = directory.to_str() else {
        return Err(GitProcessError::InvalidPath);
    };
    let mut argv = base_args();
    argv.extend(["-C", directory].map(String::from));
    argv.extend(command_args.iter().map(|arg| String::from(*arg)));
    wait_git(|cancellable, callback| {
        run_git(
            GitSpec {
                argv,
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
