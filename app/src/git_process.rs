use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk4::{gio, glib};

use crate::git_status::{
    GitAttrs, GitCapabilities, GitPath, GitStatusSnapshot, index_info_line, parse_attrs,
    parse_ls_tree_entry, parse_status, resolve_capabilities,
};

mod log;
mod support;
pub(crate) use log::{GitCommitSummary, GitLogState};
use support::{
    base_args, git_env, identity_part_is_valid, optional_text, redact_git_argv, stderr_text,
};

const STATUS_CAP: usize = 4 * 1024 * 1024;
const ATTR_CAP: usize = 2 * 1024 * 1024;
const BLOB_CAP: usize = 1_000_001;
const STDERR_CAP: usize = 64 * 1024;

pub(crate) type GitCallback<T> = Rc<dyn Fn(Result<T, GitProcessError>)>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum GitProcessError {
    Cancelled,
    InvalidPath,
    InvalidIdentity,
    OutputTooLarge,
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
    repo: PathBuf,
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
    pub(crate) fn new(repo: PathBuf) -> Self {
        Self { repo }
    }

    pub(crate) fn detect_repo(
        folder: &Path,
        cancellable: &gio::Cancellable,
        callback: GitCallback<PathBuf>,
    ) {
        let Some(folder) = folder.to_str() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        let spec = GitSpec {
            argv: base_args()
                .into_iter()
                .chain(["-C", folder, "rev-parse", "--show-toplevel"].map(String::from))
                .collect(),
            env: Vec::new(),
            stdin: None,
            stdout_cap: 4096,
            allow_failure: false,
        };
        run_git(
            spec,
            cancellable,
            Rc::new(move |result| {
                callback(result.and_then(|output| {
                    let text = String::from_utf8(output.stdout)
                        .map_err(|_| GitProcessError::ParseFailed)?;
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        Err(GitProcessError::ParseFailed)
                    } else {
                        Ok(PathBuf::from(trimmed))
                    }
                }));
            }),
        );
    }

    pub(crate) fn status(
        &self,
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitStatusSnapshot>,
    ) {
        self.run(
            ["status", "--porcelain=v2", "-z", "--branch", "--no-renames"],
            None,
            STATUS_CAP,
            false,
            cancellable,
            Rc::new(move |result| {
                callback(result.map(|output| parse_status(&output.stdout)));
            }),
        );
    }

    pub(crate) fn check_repo_capabilities(
        &self,
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitCapabilities>,
    ) {
        let repo = self.clone();
        let cancellable_for_autocrlf = cancellable.clone();
        self.run_text(
            ["rev-parse", "--show-object-format"],
            false,
            cancellable,
            Rc::new(move |object| {
                let Ok(object_format) = object else {
                    callback(object.map(|_| GitCapabilities::default()));
                    return;
                };
                let repo_for_eol = repo.clone();
                let callback_for_eol = Rc::clone(&callback);
                let cancellable_for_eol = cancellable_for_autocrlf.clone();
                repo.run_text(
                    ["config", "--get", "core.autocrlf"],
                    true,
                    &cancellable_for_autocrlf,
                    Rc::new(move |autocrlf| {
                        let autocrlf = optional_text(autocrlf);
                        let object_format = object_format.clone();
                        let callback_for_eol = Rc::clone(&callback_for_eol);
                        repo_for_eol.run_text(
                            ["config", "--get", "core.eol"],
                            true,
                            &cancellable_for_eol,
                            Rc::new(move |eol| {
                                let eol = optional_text(eol);
                                callback_for_eol(Ok(resolve_capabilities(
                                    &object_format,
                                    &autocrlf,
                                    &eol,
                                )));
                            }),
                        );
                    }),
                );
            }),
        );
    }

    pub(crate) fn check_attrs(
        &self,
        paths: &[GitPath],
        cancellable: &gio::Cancellable,
        callback: GitCallback<GitAttrs>,
    ) {
        let mut stdin = Vec::new();
        for path in paths {
            stdin.extend_from_slice(path.raw());
            stdin.push(0);
        }
        self.run(
            [
                "check-attr",
                "-z",
                "--stdin",
                "filter",
                "working-tree-encoding",
                "text",
                "eol",
            ],
            Some(stdin),
            ATTR_CAP,
            false,
            cancellable,
            Rc::new(move |result| {
                callback(result.and_then(|output| {
                    parse_attrs(&output.stdout).map_err(|_| GitProcessError::ParseFailed)
                }));
            }),
        );
    }

    pub(crate) fn cat_blob(
        &self,
        oid: &str,
        cancellable: &gio::Cancellable,
        callback: GitCallback<Vec<u8>>,
    ) {
        self.run(
            ["cat-file", "blob", oid],
            None,
            BLOB_CAP,
            false,
            cancellable,
            Rc::new(move |result| callback(result.map(|output| output.stdout))),
        );
    }

    pub(crate) fn hash_file_no_filters(
        &self,
        path: &GitPath,
        cancellable: &gio::Cancellable,
        callback: GitCallback<String>,
    ) {
        let Some(path) = path.as_utf8() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        let absolute_path = self.repo.join(path);
        let Some(path) = absolute_path.to_str() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        self.run_text(
            ["hash-object", "-w", "--no-filters", "--", path],
            false,
            cancellable,
            Rc::new(move |result| callback(result.map(|text| text.trim().to_string()))),
        );
    }

    pub(crate) fn stage_blob_index_info(
        &self,
        mode: &str,
        oid: &str,
        path: &GitPath,
        cancellable: &gio::Cancellable,
        callback: GitCallback<()>,
    ) {
        let stdin = index_info_line(mode, oid, path.raw());
        self.run(
            ["update-index", "--add", "-z", "--index-info"],
            Some(stdin),
            4096,
            false,
            cancellable,
            Rc::new(move |result| callback(result.map(|_output| ()))),
        );
    }

    pub(crate) fn read_head_index_entry(
        &self,
        path: &GitPath,
        cancellable: &gio::Cancellable,
        callback: GitCallback<Option<(String, String)>>,
    ) {
        let Some(path) = path.as_utf8() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        self.run(
            ["ls-tree", "-z", "HEAD", "--", path],
            None,
            4096,
            true,
            cancellable,
            Rc::new(move |result| {
                callback(result.map(|output| {
                    if output.status == 0 && !output.stdout.is_empty() {
                        parse_ls_tree_entry(&output.stdout)
                    } else {
                        None
                    }
                }));
            }),
        );
    }

    pub(crate) fn unstage_path(
        &self,
        path: &GitPath,
        cancellable: &gio::Cancellable,
        callback: GitCallback<()>,
    ) {
        let process = self.clone();
        let path_for_read = path.clone();
        let path_for_write = path.clone();
        let cancellable_for_write = cancellable.clone();
        self.read_head_index_entry(
            &path_for_read,
            cancellable,
            Rc::new(move |result| match result {
                Ok(Some((mode, oid))) => {
                    process.stage_blob_index_info(
                        &mode,
                        &oid,
                        &path_for_write,
                        &cancellable_for_write,
                        Rc::clone(&callback),
                    );
                }
                Ok(None) => {
                    process.remove_from_index(
                        &path_for_write,
                        &cancellable_for_write,
                        Rc::clone(&callback),
                    );
                }
                Err(error) => callback(Err(error)),
            }),
        );
    }

    pub(crate) fn remove_from_index(
        &self,
        path: &GitPath,
        cancellable: &gio::Cancellable,
        callback: GitCallback<()>,
    ) {
        let Some(path) = path.as_utf8() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        self.run(
            ["update-index", "--force-remove", "--", path],
            None,
            4096,
            false,
            cancellable,
            Rc::new(move |result| callback(result.map(|_output| ()))),
        );
    }

    pub(crate) fn restore_worktree_path(
        &self,
        path: &GitPath,
        cancellable: &gio::Cancellable,
        callback: GitCallback<()>,
    ) {
        let Some(path) = path.as_utf8() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        self.run(
            ["restore", "--worktree", "--", path],
            None,
            4096,
            false,
            cancellable,
            Rc::new(move |result| callback(result.map(|_output| ()))),
        );
    }

    pub(crate) fn read_git_identity(
        &self,
        cancellable: &gio::Cancellable,
        callback: GitCallback<Option<GitIdentity>>,
    ) {
        let repo = self.clone();
        let cancellable_for_global = cancellable.clone();
        self.read_identity_scope(
            "--local",
            cancellable,
            Rc::new(move |local| match local {
                Ok(Some(identity)) => callback(Ok(Some(identity))),
                Ok(None) => repo.read_identity_scope(
                    "--global",
                    &cancellable_for_global,
                    Rc::clone(&callback),
                ),
                Err(error) => callback(Err(error)),
            }),
        );
    }

    pub(crate) fn commit(
        &self,
        identity: &GitIdentity,
        message: &str,
        cancellable: &gio::Cancellable,
        callback: GitCallback<()>,
    ) {
        if GitIdentity::new(identity.name.clone(), identity.email.clone()).is_err() {
            callback(Err(GitProcessError::InvalidIdentity));
            return;
        }
        let name = format!("user.name={}", identity.name);
        let email = format!("user.email={}", identity.email);
        self.run(
            [
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                name.as_str(),
                "-c",
                email.as_str(),
                "commit",
                "--no-gpg-sign",
                "--no-status",
                "--no-verify",
                "-m",
                message,
            ],
            None,
            4096,
            false,
            cancellable,
            Rc::new(move |result| callback(result.map(|_output| ()))),
        );
    }

    fn read_identity_scope(
        &self,
        scope: &str,
        cancellable: &gio::Cancellable,
        callback: GitCallback<Option<GitIdentity>>,
    ) {
        let repo = self.clone();
        let scope = String::from(scope);
        let scope_for_email = scope.clone();
        let cancellable_for_email = cancellable.clone();
        self.run_text(
            ["config", scope.as_str(), "--get", "user.name"],
            true,
            cancellable,
            Rc::new(move |name| {
                let name = optional_text(name);
                let name_for_identity = name.clone();
                let callback = Rc::clone(&callback);
                repo.run_text(
                    ["config", scope_for_email.as_str(), "--get", "user.email"],
                    true,
                    &cancellable_for_email,
                    Rc::new(move |email| {
                        let email = optional_text(email);
                        if name_for_identity.trim().is_empty() || email.trim().is_empty() {
                            callback(Ok(None));
                        } else {
                            callback(
                                GitIdentity::new(
                                    name_for_identity.trim().to_string(),
                                    email.trim().to_string(),
                                )
                                .map(Some),
                            );
                        }
                    }),
                );
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
        let Some(repo) = self.repo.to_str() else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        let Some(git_dir) = self.repo.join(".git").to_str().map(String::from) else {
            callback(Err(GitProcessError::InvalidPath));
            return;
        };
        let spec = GitSpec {
            argv: base_args()
                .into_iter()
                .chain(args.into_iter().map(String::from))
                .collect(),
            env: vec![
                (String::from("GIT_DIR"), git_dir),
                (String::from("GIT_WORK_TREE"), String::from(repo)),
            ],
            stdin,
            stdout_cap,
            allow_failure,
        };
        run_git(spec, cancellable, callback);
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
    drop(redact_git_argv(&spec.argv));
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
