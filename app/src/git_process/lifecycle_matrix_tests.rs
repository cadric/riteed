//! Characterization of the subprocess lifecycle table, using real GIO children.
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::prelude::*;
use gtk4::{gio, glib};

use super::{GitDeadlineConfig, GitProcessError, GitRunOutput, GitSpec, run_git_with_deadlines};

type Results = Rc<RefCell<Vec<Result<GitRunOutput, GitProcessError>>>>;
type Continuation = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

// A single process, without shell children. Readiness means the TERM handler is
// installed. Files form an explicit control channel independent of owned pipes.
const CHILD: &str = r"
import os, pathlib, signal, sys, time
directory = pathlib.Path(sys.argv[1])
mode = sys.argv[2]
def terminate(signum, frame):
    (directory / 'term').write_text('received')
    if mode != 'ignore-term':
        sys.exit(0)
signal.signal(signal.SIGTERM, terminate)
(directory / 'ready').write_text('ready')
while not (directory / 'release').exists():
    time.sleep(0.005)
if mode == 'signal':
    os.kill(os.getpid(), signal.SIGKILL)
if mode == 'nonzero':
    sys.stderr.write('controlled failure')
    sys.exit(7)
sys.stdout.write('controlled output')
";

struct ChildRun {
    directory: PathBuf,
    child: Rc<RefCell<Option<super::TestChild>>>,
    results: Results,
    events: Rc<RefCell<Vec<&'static str>>>,
    grace: Continuation,
    cancellable: gio::Cancellable,
}

impl ChildRun {
    fn start(mode: &str, kill_on_cancel: bool, deadline: bool) -> Self {
        Self::start_with(mode, kill_on_cancel, deadline, |_| {})
    }

    fn start_with(
        mode: &str,
        kill_on_cancel: bool,
        deadline: bool,
        configure: impl FnOnce(&mut GitDeadlineConfig),
    ) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "riteed-lifecycle-matrix-{}-{mode}",
            std::process::id()
        ));
        assert!(!directory.exists(), "fixture directory must be fresh");
        assert!(fs::create_dir(&directory).is_ok());
        let run = Self {
            directory,
            child: Rc::new(RefCell::new(None)),
            results: Rc::new(RefCell::new(Vec::new())),
            events: Rc::new(RefCell::new(Vec::new())),
            grace: Rc::new(RefCell::new(None)),
            cancellable: gio::Cancellable::new(),
        };
        let mut config = GitDeadlineConfig::production();
        if deadline {
            config.operation = Duration::from_millis(20);
        }
        // The first grace is held explicitly, so child readiness never races a
        // TERM. The second interval has no time-based assertion: we await wait.
        config.grace = Duration::from_millis(50);
        let child = Rc::clone(&run.child);
        config.child_started = Some(Rc::new(move |process| {
            *child.borrow_mut() = Some(process);
        }));
        config.deadline_fired = Some(run.observer("deadline"));
        config.io_settled = Some(run.observer("io"));
        config.io_fault = Some(run.observer("io-fault"));
        config.cancellation_accepted = Some(run.observer("cancellation-accepted"));
        config.wait_completed = Some(run.observer("wait"));
        config.term_sent = Some(run.observer("term"));
        config.force_exited = Some(run.observer("force"));
        let grace = Rc::clone(&run.grace);
        config.grace_checkpoint = Some(Rc::new(move |advance| {
            *grace.borrow_mut() = Some(advance);
        }));
        configure(&mut config);
        let spec = GitSpec {
            argv: vec![
                String::from("/usr/bin/python3"),
                String::from("-c"),
                String::from(CHILD),
                run.directory.to_string_lossy().into_owned(),
                String::from(mode),
            ],
            env: Vec::new(),
            stdin: None,
            stdout_cap: 1024,
            allow_failure: false,
            kill_on_cancel,
        };
        let events = Rc::clone(&run.events);
        let results = Rc::clone(&run.results);
        run_git_with_deadlines(
            spec,
            &run.cancellable,
            Rc::new(move |result| {
                events.borrow_mut().push("callback");
                results.borrow_mut().push(result);
            }),
            config,
        );
        assert!(
            run.child.borrow().is_some(),
            "runner must spawn the fixture"
        );
        pump("child readiness or supervised I/O fault", || {
            run.directory.join("ready").exists() || (kill_on_cancel && run.has("io-fault"))
        });
        run
    }

    fn observer(&self, event: &'static str) -> Rc<dyn Fn()> {
        let events = Rc::clone(&self.events);
        Rc::new(move || events.borrow_mut().push(event))
    }

    fn release(&self) {
        assert!(fs::write(self.directory.join("release"), b"release").is_ok());
    }

    fn advance_grace(&self) {
        pump("held grace", || self.grace.borrow().is_some());
        let advance = self.grace.borrow_mut().take();
        assert!(advance.is_some());
        if let Some(advance) = advance {
            advance();
        }
    }

    fn await_terminal(&self) {
        pump("terminal callback", || !self.results.borrow().is_empty());
        assert_eq!(self.results.borrow().len(), 1);
        let events = self.events.borrow();
        assert_eq!(events.iter().filter(|&&item| item == "wait").count(), 1);
        assert_eq!(events.iter().filter(|&&item| item == "callback").count(), 1);
        let wait = events.iter().position(|&event| event == "wait");
        let callback = events.iter().position(|&event| event == "callback");
        assert!(
            wait < callback,
            "terminal callback must follow successful wait"
        );
    }

    fn has(&self, event: &str) -> bool {
        self.events.borrow().contains(&event)
    }
}

impl Drop for ChildRun {
    fn drop(&mut self) {
        let _held = self.grace.borrow_mut().take();
        if let Some(child) = self.child.borrow_mut().take() {
            // Teardown also runs after assertion failure: never leave a child
            // relying on a release marker that is about to be removed.
            let reaped = child.force_reap();
            if !std::thread::panicking() {
                assert!(reaped, "fixture teardown must reap its child");
            }
        }
        let _removed = fs::remove_dir_all(&self.directory);
    }
}

fn pump(label: &str, done: impl Fn() -> bool) {
    let expired = Rc::new(Cell::new(false));
    let expired_for_source = Rc::clone(&expired);
    let source = glib::timeout_add_local_once(Duration::from_secs(10), move || {
        expired_for_source.set(true);
    });
    let tick = glib::timeout_add_local(Duration::from_millis(5), || glib::ControlFlow::Continue);
    let context = glib::MainContext::default();
    while !done() && !expired.get() {
        let _dispatched = context.iteration(true);
    }
    tick.remove();
    if !expired.get() {
        source.remove();
    }
    assert!(done(), "{label}");
}

#[test]
fn ordinary_success_nonzero_and_signal_exit_are_reaped_before_result() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    for mode in ["success", "nonzero", "signal"] {
        let run = ChildRun::start(mode, true, false);
        run.release();
        run.await_terminal();
        let results = run.results.borrow();
        if mode == "success" {
            assert!(matches!(results.first(), Some(Ok(_))));
            if let Some(Ok(output)) = results.first() {
                assert_eq!(output.status, 0);
                assert_eq!(output.stdout, b"controlled output");
            }
        } else {
            assert!(matches!(
                results.first(),
                Some(Err(GitProcessError::CommandFailed(_)))
            ));
            if mode == "nonzero"
                && let Some(Err(GitProcessError::CommandFailed(message))) = results.first()
            {
                assert_eq!(message, "controlled failure");
            }
        }
        assert!(!run.has("deadline"));
        assert!(!run.has("force"));
    }
}

#[test]
fn read_only_cancellation_forces_exit_and_waits_before_callback() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    let run = ChildRun::start("ignore-term", true, false);
    run.cancellable.cancel();
    run.await_terminal();
    assert!(run.has("force"));
    assert!(!run.has("term"));
    assert!(matches!(
        run.results.borrow().first(),
        Some(Err(GitProcessError::Cancelled))
    ));
}

#[test]
fn timeout_term_response_and_ignored_term_have_one_terminal_timeout() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    for mode in ["term-exit", "ignore-term"] {
        let run = ChildRun::start(mode, false, true);
        pump("deadline reaches held grace", || {
            run.grace.borrow().is_some()
        });
        assert!(!run.has("io"));
        assert!(run.results.borrow().is_empty());
        assert!(!run.has("term"));
        assert!(!run.has("force"));
        run.advance_grace();
        run.await_terminal();
        assert!(run.directory.join("term").exists());
        assert!(run.has("term"));
        assert_eq!(run.has("force"), mode == "ignore-term");
        assert!(matches!(
            run.results.borrow().first(),
            Some(Err(GitProcessError::TimedOut))
        ));
    }
}

#[test]
fn completion_while_grace_is_held_prevents_late_kill_and_duplicate_callback() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    let run = ChildRun::start("success", false, true);
    pump("held grace", || run.grace.borrow().is_some());
    run.release();
    run.await_terminal();
    run.advance_grace();
    assert!(!run.has("term"));
    assert!(!run.has("force"));
    assert_eq!(run.results.borrow().len(), 1);
    assert!(matches!(
        run.results.borrow().first(),
        Some(Err(GitProcessError::TimedOut))
    ));
}

#[test]
fn spawn_failure_has_one_immediate_callback_and_no_child_wait() {
    let results: Results = Rc::new(RefCell::new(Vec::new()));
    let results_for_callback = Rc::clone(&results);
    let spec = GitSpec {
        argv: vec![String::from("/riteed-fixture-nonexistent-executable")],
        env: Vec::new(),
        stdin: None,
        stdout_cap: 1024,
        allow_failure: false,
        kill_on_cancel: true,
    };
    let child_started = Rc::new(Cell::new(false));
    let child_for_config = Rc::clone(&child_started);
    let mut config = GitDeadlineConfig::production();
    config.child_started = Some(Rc::new(move |_| child_for_config.set(true)));
    run_git_with_deadlines(
        spec,
        &gio::Cancellable::new(),
        Rc::new(move |result| {
            results_for_callback.borrow_mut().push(result);
        }),
        config,
    );
    assert!(!child_started.get());
    assert_eq!(results.borrow().len(), 1);
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::SpawnFailed(_)))
    ));
}

#[test]
fn readonly_io_failure_forces_cleanup_without_waiting_for_deadline() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    let run = ChildRun::start_with("io-failure", true, false, |config| {
        config.communication_error = true;
    });
    pump("I/O failure", || run.has("io-fault"));
    assert!(
        run.has("force"),
        "read-only I/O failure must force cleanup immediately"
    );
    assert!(!run.has("deadline"));
    run.await_terminal();
    assert!(matches!(
        run.results.borrow().first(),
        Some(Err(GitProcessError::CommandFailed(_)))
    ));
}

#[test]
fn wait_failure_is_reported_once_while_terminal_callback_remains_pending() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    let reject = Rc::new(Cell::new(true));
    let reject_for_config = Rc::clone(&reject);
    let reports = Rc::new(Cell::new(0));
    let reports_for_config = Rc::clone(&reports);
    let run = ChildRun::start_with("wait-failure", false, false, |config| {
        config.wait_error = Some(Rc::new(move || reject_for_config.get()));
        config.wait_failed = Some(Rc::new(move || {
            reports_for_config.set(reports_for_config.get() + 1);
        }));
    });
    run.release();
    pump("wait failure diagnostic", || reports.get() > 0);
    assert!(
        run.results.borrow().is_empty(),
        "failed wait is not terminal completion"
    );
    reject.set(false);
    run.await_terminal();
    assert_eq!(reports.get(), 1);
}

#[test]
fn mutation_io_failure_retains_deadline_and_grace_until_reaped() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok());
    let _guard = acquired.ok();
    let run = ChildRun::start_with("mutation-io-failure", false, false, |config| {
        config.communication_error = true;
        config.operation = Duration::from_secs(3);
    });
    pump("I/O failure", || run.has("io-fault"));
    assert!(!run.has("force"));
    assert!(!run.has("term"));
    assert!(!run.has("deadline"));
    assert!(run.results.borrow().is_empty());
    pump("original deadline and held grace", || {
        run.grace.borrow().is_some()
    });
    assert!(run.has("deadline"));
    assert!(!run.has("force"));
    assert!(!run.has("term"));
    assert!(run.results.borrow().is_empty());
    run.release();
    run.await_terminal();
    assert!(matches!(
        run.results.borrow().first(),
        Some(Err(GitProcessError::CommandFailed(_)))
    ));
}
