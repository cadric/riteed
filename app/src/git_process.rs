use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

mod client;
mod io_pump;
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
#[cfg(test)]
use support::detect_repo_spec;
use support::{git_env, stderr_text};

const STDERR_CAP: usize = 64 * 1024;
pub(crate) const GIT_BLOB_BYTE_LIMIT: usize = 1_000_001;
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

#[derive(Clone, Debug)]
pub(crate) struct GitProcess {
    repo: GitRepoContext,
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
    #[cfg(test)]
    if let Some(observer) = deadlines.child_started.as_ref() {
        observer(TestChild(subprocess.clone()));
    }
    #[cfg(test)]
    let inject_stdin_error = deadlines.communication_error;
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
    install_cancellation_watch(&state);
    install_operation_deadline(&state, cancellable);
    let state_for_fault = Rc::clone(&state);
    let state_for_complete = Rc::clone(&state);
    io_pump::start(
        subprocess.stdin_pipe(),
        subprocess.stdout_pipe(),
        subprocess.stderr_pipe(),
        io_pump::GitIoConfig {
            stdin: spec.stdin.unwrap_or_default(),
            stdout_cap: spec.stdout_cap,
            stderr_cap: STDERR_CAP,
            cleanup: state.borrow().io_cleanup.clone(),
            #[cfg(test)]
            inject_stdin_error,
        },
        Rc::new(move |error| record_io_fault(&state_for_fault, error)),
        Rc::new(move |output| finish_io(&state_for_complete, output)),
    );
    start_uncancelled_wait(&state);
}

fn record_io_fault(state: &Rc<RefCell<GitRunState>>, error: GitProcessError) {
    let _accepted = accept_user_cancellation(state);
    let should_kill = {
        let mut run = state.borrow_mut();
        run.lifecycle.record_reason(error);
        run.policy.kill_on_cancel && !run.lifecycle.is_reaped()
    };
    if should_kill {
        force_exit(state);
    }
    request_io_cleanup(state);
    #[cfg(test)]
    observe(state, |config| config.io_fault.clone());
}

fn finish_io(state: &Rc<RefCell<GitRunState>>, output: io_pump::GitIoCapture) {
    let settled = {
        let mut run = state.borrow_mut();
        run.output = Some(output);
        run.lifecycle.settle_io()
    };
    if settled {
        #[cfg(test)]
        observe(state, |config| config.io_settled.clone());
    }
    finish_terminal(state);
}
struct GitRunState {
    subprocess: gio::Subprocess,
    stdout_cap: usize,
    policy: GitRunPolicy,
    callback: Option<GitCallback<GitRunOutput>>,
    output: Option<GitCapture>,
    lifecycle: GitLifecycle,
    user_cancelled: Arc<AtomicBool>,
    timeout_requested: Arc<AtomicBool>,
    io_cleanup: gio::Cancellable,
    cleanup: GitCleanupState,
    cancelled_handler: Option<(gio::Cancellable, gio::CancelledHandlerId)>,
    cancel_watch: Option<glib::SourceId>,
    wait_retry: Option<glib::SourceId>,
    operation: Option<glib::SourceId>,
    grace: Option<glib::SourceId>,
    force_grace: Option<glib::SourceId>,
    deadlines: GitDeadlineConfig,
}

struct GitRunPolicy {
    allow_failure: bool,
    kill_on_cancel: bool,
}

#[derive(Default)]
struct GitCleanupState {
    cancellation_accepted: bool,
    io_requested: bool,
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
            policy: GitRunPolicy {
                allow_failure,
                kill_on_cancel,
            },
            callback: Some(callback),
            output: None,
            lifecycle: GitLifecycle::default(),
            user_cancelled: Arc::new(AtomicBool::new(false)),
            timeout_requested: Arc::new(AtomicBool::new(false)),
            io_cleanup: gio::Cancellable::new(),
            cleanup: GitCleanupState::default(),
            cancelled_handler: None,
            cancel_watch: None,
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
            self.cancel_watch.take(),
        ]
        .into_iter()
        .flatten()
        {
            source.remove();
        }
    }
}

fn install_cancellation_watch(state: &Rc<RefCell<GitRunState>>) {
    if accept_user_cancellation(state) {
        return;
    }
    let state_for_watch = Rc::clone(state);
    let source = glib::timeout_add_local(Duration::from_millis(10), move || {
        if accept_user_cancellation(&state_for_watch) {
            state_for_watch.borrow_mut().cancel_watch = None;
            glib::ControlFlow::Break
        } else {
            glib::ControlFlow::Continue
        }
    });
    state.borrow_mut().cancel_watch = Some(source);
}

fn accept_user_cancellation(state: &Rc<RefCell<GitRunState>>) -> bool {
    let (accepted, force_now) = {
        let mut run = state.borrow_mut();
        if run.cleanup.cancellation_accepted || !run.user_cancelled.load(Ordering::SeqCst) {
            return run.cleanup.cancellation_accepted;
        }
        run.cleanup.cancellation_accepted = true;
        run.lifecycle.record_reason(GitProcessError::Cancelled);
        (
            true,
            run.policy.kill_on_cancel && !run.lifecycle.is_reaped(),
        )
    };
    if force_now {
        force_exit(state);
    }
    request_io_cleanup(state);
    #[cfg(test)]
    observe(state, |config| config.cancellation_accepted.clone());
    accepted
}

fn install_operation_deadline(state: &Rc<RefCell<GitRunState>>, cancellable: &gio::Cancellable) {
    let state_for_timeout = Rc::clone(state);
    let cancellable_for_timeout = cancellable.clone();
    let operation = state.borrow().deadlines.operation;
    let source = glib::timeout_add_local_once(operation, move || {
        let mut run = state_for_timeout.borrow_mut();
        run.operation = None;
        if run.user_cancelled.load(Ordering::SeqCst) {
            let kill_now = run.policy.kill_on_cancel;
            drop(run);
            let _accepted = accept_user_cancellation(&state_for_timeout);
            #[cfg(test)]
            observe(&state_for_timeout, |config| config.deadline_fired.clone());
            if kill_now {
                force_exit(&state_for_timeout);
            } else {
                begin_mutation_grace(&state_for_timeout);
            }
            return;
        }
        if run.lifecycle.reason.is_none() {
            run.lifecycle.reason = Some(
                if cancellable_for_timeout.is_cancelled()
                    && !run.timeout_requested.load(Ordering::SeqCst)
                {
                    GitProcessError::Cancelled
                } else {
                    GitProcessError::TimedOut
                },
            );
        }
        let kill_now = run.policy.kill_on_cancel;
        run.timeout_requested.store(true, Ordering::SeqCst);
        drop(run);
        request_io_cleanup(&state_for_timeout);
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
    let finished = state.borrow_mut().lifecycle.finish_wait();
    if !finished {
        return;
    }
    #[cfg(test)]
    observe(state, |config| config.wait_completed.clone());
    cancel_requested_io_after_reap(state);
    finish_terminal(state);
}

fn request_io_cleanup(state: &Rc<RefCell<GitRunState>>) {
    state.borrow_mut().cleanup.io_requested = true;
    cancel_requested_io_after_reap(state);
}

fn cancel_requested_io_after_reap(state: &Rc<RefCell<GitRunState>>) {
    let cleanup = {
        let run = state.borrow();
        (run.cleanup.io_requested && run.lifecycle.is_reaped()).then(|| run.io_cleanup.clone())
    };
    if let Some(cleanup) = cleanup {
        cleanup.cancel();
    }
}

fn finish_terminal(state: &Rc<RefCell<GitRunState>>) {
    let _accepted = accept_user_cancellation(state);
    let (callback, handler) = {
        let mut run = state.borrow_mut();
        if !run.lifecycle.take_terminal() {
            return;
        }
        run.clear_sources();
        (run.callback.take(), run.cancelled_handler.take())
    };
    if let Some((cancellable, handler)) = handler {
        cancellable.disconnect_cancelled(handler);
    }
    let _accepted = accept_user_cancellation(state);
    let result = {
        let mut run = state.borrow_mut();
        if let Some(reason) = run.lifecycle.reason.clone() {
            Err(reason)
        } else {
            let status = (!run.subprocess.has_signaled()).then(|| run.subprocess.exit_status());
            let output = run.output.take();
            capture_result(output, run.stdout_cap, run.policy.allow_failure, status)
        }
    };
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
mod output_tests;
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
