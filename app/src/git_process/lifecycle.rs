#[cfg(test)]
use std::rc::Rc;
use std::time::Duration;

#[cfg(test)]
use super::ChildStartedObserver;
use super::{GIT_CANCEL_KILL_GRACE, GIT_OPERATION_TIMEOUT};

#[cfg(test)]
type GraceCheckpoint = Rc<dyn Fn(Rc<dyn Fn()>)>;

#[derive(Clone)]
pub(super) struct GitDeadlineConfig {
    pub(super) operation: Duration,
    pub(super) grace: Duration,
    #[cfg(test)]
    pub(super) communication_error: bool,
    #[cfg(test)]
    pub(super) wait_error: Option<Rc<dyn Fn() -> bool>>,
    #[cfg(test)]
    pub(super) wait_failed: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) deadline_fired: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) child_started: Option<ChildStartedObserver>,
    #[cfg(test)]
    pub(super) io_settled: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) io_fault: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) cancellation_accepted: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) wait_completed: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) term_sent: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) force_exited: Option<Rc<dyn Fn()>>,
    #[cfg(test)]
    pub(super) grace_checkpoint: Option<GraceCheckpoint>,
}

impl GitDeadlineConfig {
    pub(super) fn production() -> Self {
        Self {
            operation: GIT_OPERATION_TIMEOUT,
            grace: GIT_CANCEL_KILL_GRACE,
            #[cfg(test)]
            communication_error: false,
            #[cfg(test)]
            wait_error: None,
            #[cfg(test)]
            wait_failed: None,
            #[cfg(test)]
            deadline_fired: None,
            #[cfg(test)]
            child_started: None,
            #[cfg(test)]
            io_settled: None,
            #[cfg(test)]
            io_fault: None,
            #[cfg(test)]
            cancellation_accepted: None,
            #[cfg(test)]
            wait_completed: None,
            #[cfg(test)]
            term_sent: None,
            #[cfg(test)]
            force_exited: None,
            #[cfg(test)]
            grace_checkpoint: None,
        }
    }
}

// Pure lifecycle transitions. Communication settlement never implies reaping.
#[derive(Default)]
pub(super) struct GitLifecycle {
    pub(super) reason: Option<super::GitProcessError>,
    io: IoPhase,
    wait: WaitPhase,
    completed: bool,
    wait_failure_reported: bool,
}

#[derive(Default, Eq, PartialEq)]
enum IoPhase {
    #[default]
    Active,
    Settled,
}

#[derive(Default, Eq, PartialEq)]
enum WaitPhase {
    #[default]
    Idle,
    InFlight,
    Reaped,
}

impl GitLifecycle {
    pub(super) fn is_reaped(&self) -> bool {
        self.wait == WaitPhase::Reaped
    }

    pub(super) fn record_reason(&mut self, reason: super::GitProcessError) {
        if self.reason.is_none() {
            self.reason = Some(reason);
        }
    }
    pub(super) fn settle_io(&mut self) -> bool {
        if self.io == IoPhase::Settled {
            return false;
        }
        self.io = IoPhase::Settled;
        true
    }
    pub(super) fn begin_wait(&mut self) -> bool {
        if self.wait != WaitPhase::Idle || self.completed {
            return false;
        }
        self.wait = WaitPhase::InFlight;
        true
    }
    pub(super) fn wait_failed(&mut self) -> bool {
        self.wait = WaitPhase::Idle;
        let report = !self.wait_failure_reported;
        self.wait_failure_reported = true;
        report
    }
    pub(super) fn finish_wait(&mut self) -> bool {
        if self.wait != WaitPhase::InFlight {
            return false;
        }
        self.wait = WaitPhase::Reaped;
        true
    }

    pub(super) fn take_terminal(&mut self) -> bool {
        if self.completed || self.io != IoPhase::Settled || self.wait != WaitPhase::Reaped {
            return false;
        }
        self.completed = true;
        true
    }
}

pub(super) type GitCapture = Result<(Vec<u8>, Vec<u8>), ()>;
pub(super) fn capture_result(
    output: Option<GitCapture>,
    cap: usize,
    allow_failure: bool,
    status: Option<i32>,
) -> Result<super::GitRunOutput, super::GitProcessError> {
    use super::{GitProcessError, GitRunOutput, STDERR_CAP, stderr_text};
    let Some(Ok((stdout, stderr))) = output else {
        return Err(GitProcessError::CommandFailed(String::from(
            "Git communication ended without output.",
        )));
    };
    if stdout.len() > cap || stderr.len() > STDERR_CAP {
        return Err(GitProcessError::OutputTooLarge);
    }
    let Some(status) = status else {
        return Err(GitProcessError::CommandFailed(String::new()));
    };
    if !allow_failure && status != 0 {
        return Err(GitProcessError::CommandFailed(stderr_text(&stderr)));
    }
    Ok(GitRunOutput { status, stdout })
}

#[derive(Clone, Debug)]
pub(crate) struct GitRunOutput {
    pub(super) status: i32,
    pub(super) stdout: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(super) struct GitSpec {
    pub(super) argv: Vec<String>,
    pub(super) env: Vec<(String, String)>,
    pub(super) stdin: Option<Vec<u8>>,
    pub(super) stdout_cap: usize,
    pub(super) allow_failure: bool,
    pub(super) kill_on_cancel: bool,
}
