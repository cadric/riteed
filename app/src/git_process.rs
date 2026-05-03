use std::path::Path;
use std::rc::Rc;

use gtk4::{gio, glib};

mod log;
mod ops;
mod repo;
mod support;
pub(crate) use log::{GitCommitSummary, GitLogState};
pub(crate) use repo::GitRepoContext;
use repo::fallback_base;
use support::{base_args, git_env, identity_part_is_valid, stderr_text};

const STDERR_CAP: usize = 64 * 1024;

pub(crate) type GitCallback<T> = Rc<dyn Fn(Result<T, GitProcessError>)>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitProcessError {
    Cancelled,
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
            Rc::new(move |result| {
                match result.and_then(|output| GitRepoContext::parse(&output.stdout, &base, true)) {
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
                }
            }),
        );
    }

    fn run<const N: usize>(
        &self,
        args: [&str; N],
        stdin: Option<Vec<u8>>,
        stdout_cap: usize,
        allow_failure: bool,
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitRunOutput>,
    ) {
        match self.spec(args, stdin, stdout_cap, allow_failure) {
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
    let stdin_bytes = glib::Bytes::from_owned(spec.stdin.unwrap_or_default());
    let callback_for_result = callback;
    let subprocess_for_result = subprocess.clone();
    subprocess.communicate_async(
        Some(&stdin_bytes),
        Some(cancellable),
        move |result| match result {
            Ok((stdout, stderr)) => {
                let stdout = stdout.map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec());
                let stderr = stderr.map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec());
                if stdout.len() > spec.stdout_cap || stderr.len() > STDERR_CAP {
                    callback_for_result(Err(GitProcessError::OutputTooLarge));
                    return;
                }
                let status = subprocess_for_result.exit_status();
                if !spec.allow_failure && !subprocess_for_result.is_successful() {
                    callback_for_result(Err(GitProcessError::CommandFailed(stderr_text(&stderr))));
                    return;
                }
                callback_for_result(Ok(GitRunOutput { status, stdout }));
            }
            Err(error) => {
                if error.matches(gio::IOErrorEnum::Cancelled) {
                    callback_for_result(Err(GitProcessError::Cancelled));
                } else {
                    callback_for_result(Err(GitProcessError::CommandFailed(
                        error.message().to_string(),
                    )));
                }
            }
        },
    );
}

#[cfg(test)]
mod tests;
