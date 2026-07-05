use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

mod log;
mod ops;
mod repo;
mod support;
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) use log::{GitCommitSummary, GitLogState};
pub(crate) use repo::GitRepoContext;
use repo::fallback_base;
use support::{base_args, git_env, identity_part_is_valid, stderr_text};

const STDERR_CAP: usize = 64 * 1024;
const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const GIT_CANCEL_KILL_GRACE: Duration = Duration::from_secs(2);

#[derive(Clone, Default)]
struct GitTimeoutHandle {
    operation: Rc<RefCell<Option<glib::SourceId>>>,
    grace: Rc<RefCell<Option<glib::SourceId>>>,
}

impl GitTimeoutHandle {
    fn cancel(&self) {
        if let Some(source) = self.operation.borrow_mut().take() {
            source.remove();
        }
        if let Some(source) = self.grace.borrow_mut().take() {
            source.remove();
        }
    }

    fn clear_operation(&self) {
        let _source = self.operation.borrow_mut().take();
    }

    fn set_operation(&self, source: glib::SourceId) {
        *self.operation.borrow_mut() = Some(source);
    }

    fn clear_grace(&self) {
        let _source = self.grace.borrow_mut().take();
    }

    fn set_grace(&self, source: glib::SourceId) {
        *self.grace.borrow_mut() = Some(source);
    }
}

pub(crate) type GitCallback<T> = Rc<dyn Fn(Result<T, GitProcessError>)>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitProcessError {
    Cancelled,
    TimedOut,
    InvalidPath,
    InvalidIdentity,
    OutputTooLarge,
    BinaryContent,
    SpawnFailed(String),
    CommandFailed(String),
    ParseFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GitIdentity {
    pub(crate) name: String,
    pub(crate) email: String,
}

impl GitIdentity {
    pub(crate) fn new(name: String, email: String) -> Result<Self, GitProcessError> {
        (identity_part_is_valid(&name) && identity_part_is_valid(&email))
            .then_some(Self { name, email })
            .ok_or(GitProcessError::InvalidIdentity)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GitProcess {
    repo: GitRepoContext,
}

#[derive(Clone, Debug)]
pub(crate) struct GitRunOutput {
    status: i32,
    stdout: Vec<u8>,
}

#[derive(Clone, Debug)]
struct GitSpec {
    argv: Vec<String>,
    env: Vec<(String, String)>,
    stdin: Option<Vec<u8>>,
    stdout_cap: usize,
    allow_failure: bool,
    kill_on_cancel: bool,
}

impl GitProcess {
    #[must_use]
    pub(crate) fn new(repo: GitRepoContext) -> Self {
        Self { repo }
    }

    #[must_use]
    pub(crate) fn context(&self) -> &GitRepoContext {
        &self.repo
    }

    pub(crate) fn detect_repo(
        folder: &Path,
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitRepoContext>,
    ) {
        let Some(folder) = folder.to_str() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        let folder = String::from(folder);
        let base = fallback_base(Path::new(&folder));
        let cancellable_for_retry = cancellable.clone();
        let folder_for_retry = folder.clone();
        let base_for_retry = base.clone();
        let callback_for_retry = Rc::clone(&callback);
        run_git(
            detect_repo_spec(&folder, true),
            cancellable,
            Rc::new(move |result| match result {
                Ok(output) => match GitRepoContext::parse(&output.stdout, &base, true) {
                    Ok(repo) => callback(Ok(repo)),
                    Err(_error) => run_git(
                        detect_repo_spec(&folder_for_retry, false),
                        &cancellable_for_retry,
                        Rc::new({
                            let base = base_for_retry.clone();
                            let callback = Rc::clone(&callback_for_retry);
                            move |fallback| {
                                callback(fallback.and_then(|output| {
                                    GitRepoContext::parse(&output.stdout, &base, false)
                                }));
                            }
                        }),
                    ),
                },
                Err(error @ (GitProcessError::Cancelled | GitProcessError::TimedOut)) => {
                    callback(Err(error));
                }
                Err(_error) => run_git(
                    detect_repo_spec(&folder_for_retry, false),
                    &cancellable_for_retry,
                    Rc::new({
                        let base = base_for_retry.clone();
                        let callback = Rc::clone(&callback_for_retry);
                        move |fallback| {
                            callback(fallback.and_then(|output| {
                                GitRepoContext::parse(&output.stdout, &base, false)
                            }));
                        }
                    }),
                ),
            }),
        );
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "RIT-BATCH-2026-07-05-Task7 keeps this private runner signature explicit at call sites."
    )]
    fn run<const N: usize>(
        &self,
        args: [&str; N],
        stdin: Option<Vec<u8>>,
        stdout_cap: usize,
        allow_failure: bool,
        kill_on_cancel: bool,
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitRunOutput>,
    ) {
        match self.spec(args, stdin, stdout_cap, allow_failure, kill_on_cancel) {
            Ok(spec) => run_git(spec, cancellable, callback),
            Err(error) => callback(Err(error)),
        }
    }

    fn spec<const N: usize>(
        &self,
        args: [&str; N],
        stdin: Option<Vec<u8>>,
        stdout_cap: usize,
        allow_failure: bool,
        kill_on_cancel: bool,
    ) -> Result<GitSpec, GitProcessError> {
        let git_dir = self
            .repo
            .git_dir
            .to_str()
            .map(String::from)
            .ok_or(GitProcessError::InvalidPath)?;
        let work_tree = self
            .repo
            .work_tree
            .to_str()
            .map(String::from)
            .ok_or(GitProcessError::InvalidPath)?;
        Ok(GitSpec {
            argv: base_args()
                .into_iter()
                .chain(args.into_iter().map(String::from))
                .collect(),
            env: vec![
                (String::from("GIT_DIR"), git_dir),
                (String::from("GIT_WORK_TREE"), work_tree),
            ],
            stdin,
            stdout_cap,
            allow_failure,
            kill_on_cancel,
        })
    }

    fn run_text<const N: usize>(
        &self,
        args: [&str; N],
        allow_failure: bool,
        cancellable: &gio::Cancellable,
        callback: GitCallback<String>,
    ) {
        self.run(
            args,
            None,
            4096,
            allow_failure,
            true,
            cancellable,
            Rc::new(move |result| {
                callback(result.and_then(|output| {
                    String::from_utf8(output.stdout).map_err(|_| GitProcessError::ParseFailed)
                }));
            }),
        );
    }
}

fn detect_repo_spec(folder: &str, path_format_absolute: bool) -> GitSpec {
    let mut argv = base_args();
    argv.extend(["-C", folder, "rev-parse"].map(String::from));
    if path_format_absolute {
        argv.push(String::from("--path-format=absolute"));
    }
    argv.extend(
        [
            "--show-toplevel",
            "--absolute-git-dir",
            "--git-common-dir",
            "--git-path",
            "HEAD",
            "--git-path",
            "index",
            "--git-path",
            "index.lock",
            "--git-path",
            "refs/heads",
            "--git-path",
            "packed-refs",
        ]
        .map(String::from),
    );
    GitSpec {
        argv,
        env: Vec::new(),
        stdin: None,
        stdout_cap: 16 * 1024,
        allow_failure: false,
        kill_on_cancel: true,
    }
}

fn run_git(spec: GitSpec, cancellable: &gio::Cancellable, callback: GitCallback<GitRunOutput>) {
    let flags = gio::SubprocessFlags::STDIN_PIPE
        | gio::SubprocessFlags::STDOUT_PIPE
        | gio::SubprocessFlags::STDERR_PIPE;
    let launcher = gio::SubprocessLauncher::new(flags);
    launcher.set_cwd("/");
    for (name, value) in git_env() {
        launcher.setenv(name, value, true);
    }
    for (name, value) in &spec.env {
        launcher.setenv(name, value, true);
    }
    let argv_os: Vec<std::ffi::OsString> = spec.argv.iter().map(std::ffi::OsString::from).collect();
    let argv_refs: Vec<&std::ffi::OsStr> =
        argv_os.iter().map(std::ffi::OsString::as_os_str).collect();
    let subprocess = match launcher.spawn(&argv_refs) {
        Ok(process) => process,
        Err(error) => {
            callback(Err(GitProcessError::SpawnFailed(
                error.message().to_string(),
            )));
            return;
        }
    };
    let stdout_cap = spec.stdout_cap;
    let allow_failure = spec.allow_failure;
    let kill_on_cancel = spec.kill_on_cancel;
    let stdin_bytes = glib::Bytes::from_owned(spec.stdin.unwrap_or_default());
    let callback_for_result = callback;
    let subprocess_for_result = subprocess.clone();
    let finished = Rc::new(Cell::new(false));
    let timed_out = Rc::new(Cell::new(false));
    let timeout_handle = install_git_timeout(
        &subprocess,
        cancellable,
        Rc::clone(&finished),
        Rc::clone(&timed_out),
    );
    let finished_for_result = Rc::clone(&finished);
    subprocess.communicate_async(Some(&stdin_bytes), Some(cancellable), move |result| {
        finished_for_result.set(true);
        timeout_handle.cancel();
        match result {
            Ok((stdout, stderr)) => {
                let stdout = stdout.map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec());
                let stderr = stderr.map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec());
                finish_git_run_after_wait(
                    &subprocess_for_result,
                    stdout,
                    stderr,
                    stdout_cap,
                    allow_failure,
                    callback_for_result,
                );
            }
            Err(error) => {
                if error.matches(gio::IOErrorEnum::Cancelled) {
                    if kill_on_cancel || timed_out.get() {
                        kill_unfinished_git(&subprocess_for_result);
                    } else {
                        subprocess_for_result.wait_async(None::<&gio::Cancellable>, |_result| {});
                    }
                    let error = if timed_out.get() {
                        GitProcessError::TimedOut
                    } else {
                        GitProcessError::Cancelled
                    };
                    callback_for_result(Err(error));
                } else {
                    callback_for_result(Err(GitProcessError::CommandFailed(
                        error.message().to_string(),
                    )));
                }
            }
        }
    });
}

fn finish_git_run_after_wait(
    subprocess: &gio::Subprocess,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_cap: usize,
    allow_failure: bool,
    callback: GitCallback<GitRunOutput>,
) {
    let output_too_large = stdout.len() > stdout_cap || stderr.len() > STDERR_CAP;
    let subprocess_for_wait = subprocess.clone();
    subprocess.wait_async(None::<&gio::Cancellable>, move |result| {
        if let Err(error) = result {
            callback(Err(GitProcessError::CommandFailed(
                error.message().to_string(),
            )));
            return;
        }
        if output_too_large {
            callback(Err(GitProcessError::OutputTooLarge));
            return;
        }
        let status = subprocess_for_wait.exit_status();
        if !allow_failure && !subprocess_for_wait.is_successful() {
            callback(Err(GitProcessError::CommandFailed(stderr_text(&stderr))));
            return;
        }
        callback(Ok(GitRunOutput { status, stdout }));
    });
}

fn install_git_timeout(
    subprocess: &gio::Subprocess,
    cancellable: &gio::Cancellable,
    finished: Rc<Cell<bool>>,
    timed_out: Rc<Cell<bool>>,
) -> GitTimeoutHandle {
    let handle = GitTimeoutHandle::default();
    let handle_for_timeout = handle.clone();
    let subprocess_for_timeout = subprocess.clone();
    let cancellable_for_timeout = cancellable.clone();
    let timeout_source = glib::timeout_add_local_once(GIT_OPERATION_TIMEOUT, move || {
        handle_for_timeout.clear_operation();
        if finished.get() {
            return;
        }
        timed_out.set(true);
        cancellable_for_timeout.cancel();
        let subprocess_for_kill = subprocess_for_timeout.clone();
        let finished_for_kill = Rc::clone(&finished);
        let handle_for_grace = handle_for_timeout.clone();
        let grace_source = glib::timeout_add_local_once(GIT_CANCEL_KILL_GRACE, move || {
            handle_for_grace.clear_grace();
            if !finished_for_kill.get() {
                kill_unfinished_git(&subprocess_for_kill);
            }
        });
        handle_for_timeout.set_grace(grace_source);
    });
    handle.set_operation(timeout_source);
    handle
}

fn kill_unfinished_git(subprocess: &gio::Subprocess) {
    if subprocess.has_exited() {
        return;
    }
    subprocess.force_exit();
    subprocess.wait_async(None::<&gio::Cancellable>, |_result| {});
}

#[cfg(test)]
mod tests;
