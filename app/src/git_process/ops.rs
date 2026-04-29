use std::path::PathBuf;
use std::rc::Rc;

use gtk4::gio;

use crate::git_status::{
    GitAttrs, GitCapabilities, GitPath, GitStatusSnapshot, index_info_line, parse_attrs,
    parse_ls_tree_entry, parse_status, resolve_capabilities,
};

use super::repo::parse_single_git_path;
use super::support::optional_text;
use super::{GitCallback, GitIdentity, GitProcess, GitProcessError};

const STATUS_CAP: usize = 4 * 1024 * 1024;
const ATTR_CAP: usize = 2 * 1024 * 1024;
const BLOB_CAP: usize = 1_000_001;

impl GitProcess {
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
        let absolute_path = self.repo.work_tree.join(path);
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

    pub(crate) fn resolve_branch_ref_path(
        &self,
        branch: &str,
        cancellable: &gio::Cancellable,
        callback: GitCallback<PathBuf>,
    ) {
        if branch.is_empty() || branch == "(detached)" || branch.contains('\0') {
            callback(Err(GitProcessError::InvalidPath));
            return;
        }
        let git_path = format!("refs/heads/{branch}");
        self.resolve_git_path(&git_path, cancellable, callback);
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

    fn resolve_git_path(
        &self,
        git_path: &str,
        cancellable: &gio::Cancellable,
        callback: GitCallback<PathBuf>,
    ) {
        let process = self.clone();
        let git_path_for_retry = String::from(git_path);
        let cancellable_for_retry = cancellable.clone();
        let base = self.repo.work_tree.clone();
        let base_for_retry = base.clone();
        self.run_text(
            [
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                git_path,
            ],
            false,
            cancellable,
            Rc::new(move |result| match result {
                Ok(text) => callback(parse_single_git_path(&text, &base, true)),
                Err(_error) => {
                    let callback = Rc::clone(&callback);
                    let base = base_for_retry.clone();
                    process.run_text(
                        ["rev-parse", "--git-path", git_path_for_retry.as_str()],
                        false,
                        &cancellable_for_retry,
                        Rc::new(move |fallback| {
                            callback(
                                fallback
                                    .and_then(|text| parse_single_git_path(&text, &base, false)),
                            );
                        }),
                    );
                }
            }),
        );
    }
}
