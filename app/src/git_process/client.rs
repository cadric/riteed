use std::path::Path;
use std::rc::Rc;

use gtk4::gio;

use super::repo::fallback_base;
use super::support::{base_args, detect_repo_spec, git_env, identity_part_is_valid};
use super::{
    GitCallback, GitIdentity, GitProcess, GitProcessError, GitRunOutput, GitSpec, run_git,
};

impl GitIdentity {
    pub(crate) fn new(name: String, email: String) -> Result<Self, GitProcessError> {
        (identity_part_is_valid(&name) && identity_part_is_valid(&email))
            .then_some(Self { name, email })
            .ok_or(GitProcessError::InvalidIdentity)
    }
}

impl GitProcess {
    #[must_use]
    pub(crate) fn new(repo: super::GitRepoContext) -> Self {
        Self { repo }
    }

    #[must_use]
    pub(crate) fn context(&self) -> &super::GitRepoContext {
        &self.repo
    }

    pub(crate) fn detect_repo(
        folder: &Path,
        cancellable: &gio::Cancellable,
        callback: GitCallback<super::GitRepoContext>,
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
                Ok(output) => match super::GitRepoContext::parse(&output.stdout, &base, true) {
                    Ok(repo) => callback(Ok(repo)),
                    Err(_error) => run_git(
                        detect_repo_spec(&folder_for_retry, false),
                        &cancellable_for_retry,
                        Rc::new({
                            let base = base_for_retry.clone();
                            let callback = Rc::clone(&callback_for_retry);
                            move |fallback| {
                                callback(fallback.and_then(|output| {
                                    super::GitRepoContext::parse(&output.stdout, &base, false)
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
                                super::GitRepoContext::parse(&output.stdout, &base, false)
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
    pub(super) fn run<const N: usize>(
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

    pub(super) fn spec<const N: usize>(
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
        let mut env: Vec<(String, String)> = git_env()
            .into_iter()
            .map(|(name, value)| (String::from(name), String::from(value)))
            .collect();
        env.extend([
            (String::from("GIT_DIR"), git_dir),
            (String::from("GIT_WORK_TREE"), work_tree),
        ]);
        Ok(GitSpec {
            argv: base_args()
                .into_iter()
                .chain(args.into_iter().map(String::from))
                .collect(),
            env,
            stdin,
            stdout_cap,
            allow_failure,
            kill_on_cancel,
        })
    }

    pub(super) fn run_text<const N: usize>(
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
