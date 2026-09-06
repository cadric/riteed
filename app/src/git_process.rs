use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

mod lifecycle;
mod log;
mod ops;
mod repo;
mod support;
#[cfg(test)]
pub(crate) mod test_hooks;
#[cfg(test)]
pub(crate) mod test_support;
use lifecycle::{
    GitCapture, GitDeadlineConfig, GitLifecycle, GitRunOutput, GitSpec, capture_result,
};
#[cfg(test)]
pub(super) type ChildStartedObserver = Rc<dyn Fn(TestChild)>;
pub(crate) use log::{GitCommitSummary, GitLogState};
pub(crate) use repo::GitRepoContext;
use repo::fallback_base;
use support::{base_args, detect_repo_spec, git_env, identity_part_is_valid, stderr_text};

const STDERR_CAP: usize = 64 * 1024;
const GIT_OPERATION_TIMEOUT: Duration = Duration::from_secs(30);
const GIT_CANCEL_KILL_GRACE: Duration = Duration::from_secs(2);

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

fn run_git(spec: GitSpec, cancellable: &gio::Cancellable, callback: GitCallback<GitRunOutput>) {
    run_git_with_deadlines(spec, cancellable, callback, GitDeadlineConfig::production());
}
fn run_git_with_deadlines(
    spec: GitSpec,
    cancellable: &gio::Cancellable,
    callback: GitCallback<GitRunOutput>,
    deadlines: GitDeadlineConfig,
) {
    #[cfg(test)]
    let (spec, deadlines) = test_hooks::prepare(spec, deadlines, cancellable);
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
    #[cfg(test)]
    test_hooks::started(&spec.env, TestChild(subprocess.clone()));
    let stdin_bytes = glib::Bytes::from_owned(spec.stdin.unwrap_or_default());
    #[cfg(test)]
    if let Some(observer) = deadlines.child_started.as_ref() {
        observer(TestChild(subprocess.clone()));
    }
    let state = Rc::new(RefCell::new(GitRunState::new(
        subprocess.clone(),
        spec.stdout_cap,
        spec.allow_failure,
        spec.kill_on_cancel,
        callback,
        deadlines,
    )));
    let (user_cancelled, timeout_requested) = {
        let run = state.borrow();
        (
            Arc::clone(&run.user_cancelled),
            Arc::clone(&run.timeout_requested),
        )
    };
    let cancelled_handler = cancellable.connect_cancelled(move |_| {
        if !timeout_requested.load(Ordering::SeqCst) {
            user_cancelled.store(true, Ordering::SeqCst);
        }
    });
    state.borrow_mut().cancelled_handler = cancelled_handler.map(|id| (cancellable.clone(), id));
    install_operation_deadline(&state, cancellable);
    let state_for_result = Rc::clone(&state);
    subprocess.communicate_async(Some(&stdin_bytes), Some(cancellable), move |result| {
        let mut state = state_for_result.borrow_mut();
        #[cfg(test)]
        let result = if state.deadlines.communication_error {
            Err(glib::Error::new(
                gio::IOErrorEnum::Failed,
                "injected communication failure",
            ))
        } else {
            result
        };
        match result {
            Ok((stdout, stderr)) => {
                let stdout = stdout.map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec());
                let stderr = stderr.map_or_else(Vec::new, |bytes| bytes.as_ref().to_vec());
                state.output = Some(Ok((stdout, stderr)));
            }
            Err(error) => {
                let cancelled = error.matches(gio::IOErrorEnum::Cancelled);
                let reason = if cancelled {
                    state
                        .lifecycle
                        .reason
                        .clone()
                        .unwrap_or(GitProcessError::Cancelled)
                } else {
                    GitProcessError::CommandFailed(error.message().to_string())
                };
                state.lifecycle.record_reason(reason);
                state.output = Some(Err(()));
            }
        }
        state.lifecycle.communicated();
        let should_kill = state.kill_on_cancel && matches!(state.output, Some(Err(())));
        drop(state);
        #[cfg(test)]
        observe(&state_for_result, |config| {
            config.communication_settled.clone()
        });
        if should_kill {
            force_exit(&state_for_result);
        }
        start_uncancelled_wait(&state_for_result);
    });
}
struct GitRunState {
    subprocess: gio::Subprocess,
    stdout_cap: usize,
    allow_failure: bool,
    kill_on_cancel: bool,
    callback: Option<GitCallback<GitRunOutput>>,
    output: Option<GitCapture>,
    lifecycle: GitLifecycle,
    user_cancelled: Arc<AtomicBool>,
    timeout_requested: Arc<AtomicBool>,
    cancelled_handler: Option<(gio::Cancellable, gio::CancelledHandlerId)>,
    wait_retry: Option<glib::SourceId>,
    operation: Option<glib::SourceId>,
    grace: Option<glib::SourceId>,
    force_grace: Option<glib::SourceId>,
    deadlines: GitDeadlineConfig,
}
impl GitRunState {
    fn new(
        subprocess: gio::Subprocess,
        stdout_cap: usize,
        allow_failure: bool,
        kill_on_cancel: bool,
        callback: GitCallback<GitRunOutput>,
        deadlines: GitDeadlineConfig,
    ) -> Self {
        Self {
            subprocess,
            stdout_cap,
            allow_failure,
            kill_on_cancel,
            callback: Some(callback),
            output: None,
            lifecycle: GitLifecycle::default(),
            user_cancelled: Arc::new(AtomicBool::new(false)),
            timeout_requested: Arc::new(AtomicBool::new(false)),
            cancelled_handler: None,
            wait_retry: None,
            operation: None,
            grace: None,
            force_grace: None,
            deadlines,
        }
    }

    fn clear_sources(&mut self) {
        for source in [
            self.operation.take(),
            self.grace.take(),
            self.force_grace.take(),
            self.wait_retry.take(),
        ]
        .into_iter()
        .flatten()
        {
            source.remove();
        }
    }
}

fn install_operation_deadline(state: &Rc<RefCell<GitRunState>>, cancellable: &gio::Cancellable) {
    let state_for_timeout = Rc::clone(state);
    let cancellable_for_timeout = cancellable.clone();
    let operation = state.borrow().deadlines.operation;
    let source = glib::timeout_add_local_once(operation, move || {
        let mut run = state_for_timeout.borrow_mut();
        run.operation = None;
        if run.lifecycle.is_reaped() {
            return;
        }
        if run.lifecycle.reason.is_none() {
            run.lifecycle.reason = Some(
                if run.user_cancelled.load(Ordering::SeqCst)
                    || (cancellable_for_timeout.is_cancelled()
                        && !run.timeout_requested.load(Ordering::SeqCst))
                {
                    GitProcessError::Cancelled
                } else {
                    GitProcessError::TimedOut
                },
            );
        }
        let kill_now = run.kill_on_cancel;
        run.timeout_requested.store(true, Ordering::SeqCst);
        drop(run);
        #[cfg(test)]
        observe(&state_for_timeout, |config| config.deadline_fired.clone());
        cancellable_for_timeout.cancel();
        if kill_now {
            force_exit(&state_for_timeout);
        } else {
            begin_mutation_grace(&state_for_timeout);
        }
    });
    state.borrow_mut().operation = Some(source);
}

fn begin_mutation_grace(state: &Rc<RefCell<GitRunState>>) {
    let (duration, already_scheduled) = {
        let run = state.borrow();
        (
            run.deadlines.grace,
            run.grace.is_some() || run.lifecycle.is_reaped(),
        )
    };
    if already_scheduled {
        return;
    }
    let state_for_term = Rc::clone(state);
    let source = glib::timeout_add_local_once(duration, move || {
        let mut run = state_for_term.borrow_mut();
        run.grace = None;
        if run.lifecycle.is_reaped() {
            return;
        }
        #[cfg(test)]
        let checkpoint = run.deadlines.grace_checkpoint.clone();
        drop(run);
        let advance: Rc<dyn Fn()> = {
            let state = Rc::clone(&state_for_term);
            Rc::new(move || send_term_then_force(&state))
        };
        #[cfg(test)]
        if let Some(checkpoint) = checkpoint {
            checkpoint(advance);
            return;
        }
        advance();
    });
    state.borrow_mut().grace = Some(source);
}

fn send_term_then_force(state: &Rc<RefCell<GitRunState>>) {
    let (subprocess, duration) = {
        let run = state.borrow_mut();
        if run.lifecycle.is_reaped() {
            return;
        }
        (run.subprocess.clone(), run.deadlines.grace)
    };
    #[cfg(test)]
    observe(state, |config| config.term_sent.clone());
    subprocess.send_signal(15);
    let state_for_force = Rc::clone(state);
    let force = glib::timeout_add_local_once(duration, move || force_exit(&state_for_force));
    state.borrow_mut().force_grace = Some(force);
}

fn force_exit(state: &Rc<RefCell<GitRunState>>) {
    let subprocess = {
        let mut run = state.borrow_mut();
        run.force_grace = None;
        if run.lifecycle.is_reaped() {
            return;
        }
        run.subprocess.clone()
    };
    #[cfg(test)]
    observe(state, |config| config.force_exited.clone());
    subprocess.force_exit();
}

fn start_uncancelled_wait(state: &Rc<RefCell<GitRunState>>) {
    let subprocess = {
        let mut run = state.borrow_mut();
        if !run.lifecycle.begin_wait() {
            return;
        }
        run.subprocess.clone()
    };
    let state_for_wait = Rc::clone(state);
    subprocess.wait_async(None::<&gio::Cancellable>, move |wait| {
        #[cfg(test)]
        let wait = if state_for_wait
            .borrow()
            .deadlines
            .wait_error
            .as_ref()
            .is_some_and(|inject| inject())
        {
            Err(glib::Error::new(
                gio::IOErrorEnum::Failed,
                "injected wait failure",
            ))
        } else {
            wait
        };
        if wait.is_err() {
            let report = state_for_wait.borrow_mut().lifecycle.wait_failed();
            if report {
                glib::g_warning!(
                    crate::APP_ID,
                    "Git child wait failed; cleanup supervision continues."
                );
                #[cfg(test)]
                observe(&state_for_wait, |config| config.wait_failed.clone());
            }
            let retry_state = Rc::clone(&state_for_wait);
            let retry = glib::timeout_add_local_once(Duration::from_millis(100), move || {
                retry_state.borrow_mut().wait_retry = None;
                start_uncancelled_wait(&retry_state);
            });
            state_for_wait.borrow_mut().wait_retry = Some(retry);
            return;
        }
        finish_reaped(&state_for_wait);
    });
}

fn finish_reaped(state: &Rc<RefCell<GitRunState>>) {
    let (callback, result, handler) = {
        let mut run = state.borrow_mut();
        if !run.lifecycle.finish_wait() {
            return;
        }
        run.clear_sources();
        let result = if let Some(reason) = run.lifecycle.reason.clone() {
            Err(reason)
        } else {
            let status = (!run.subprocess.has_signaled()).then(|| run.subprocess.exit_status());
            let output = run.output.take();
            capture_result(output, run.stdout_cap, run.allow_failure, status)
        };
        (run.callback.take(), result, run.cancelled_handler.take())
    };
    if let Some((cancellable, handler)) = handler {
        cancellable.disconnect_cancelled(handler);
    }
    #[cfg(test)]
    observe(state, |config| config.wait_completed.clone());
    if let Some(callback) = callback {
        callback(result);
    }
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct TestChild(gio::Subprocess);
#[cfg(test)]
impl TestChild {
    pub(crate) fn force_reap(&self) -> bool {
        self.0.force_exit();
        self.0.wait(None::<&gio::Cancellable>).is_ok()
    }
}

#[cfg(test)]
mod lifecycle_matrix_tests;
#[cfg(test)]
mod lifecycle_state_tests;
#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
fn observe(
    state: &Rc<RefCell<GitRunState>>,
    select: impl FnOnce(&GitDeadlineConfig) -> Option<Rc<dyn Fn()>>,
) {
    let observer = select(&state.borrow().deadlines);
    if let Some(observer) = observer {
        observer();
    }
}
