use super::{GitDeadlineConfig, GitProcessError, GitSpec, run_git_with_deadlines};
use gtk4::{gio, glib, prelude::*};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
type HeldGrace = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

#[test]
fn timed_out_mutation_remains_pending_during_its_grace_checkpoint() {
    let _guard = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = ControlledChild::new();
    let actions: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
    let actions_for_start = Rc::clone(&actions);
    let actions_for_deadline = Rc::clone(&actions);
    let actions_for_io = Rc::clone(&actions);
    let actions_for_wait = Rc::clone(&actions);
    let actions_for_term = Rc::clone(&actions);
    let actions_for_force = Rc::clone(&actions);
    let child_for_start = Rc::clone(&fixture.child);
    let held_grace: HeldGrace = Rc::new(RefCell::new(None));
    let held_grace_for_config = Rc::clone(&held_grace);
    let results: Rc<RefCell<Vec<Result<super::GitRunOutput, GitProcessError>>>> =
        Rc::new(RefCell::new(Vec::new()));
    let results_for_callback = Rc::clone(&results);
    let cancellable = gio::Cancellable::new();
    run_git_with_deadlines(
        fixture.spec(),
        &cancellable,
        Rc::new(move |result| results_for_callback.borrow_mut().push(result)),
        GitDeadlineConfig {
            operation: Duration::from_millis(30),
            grace: Duration::from_millis(20),
            deadline_fired: Some(Rc::new(move || {
                actions_for_deadline.borrow_mut().push("deadline");
            })),
            child_started: Some(Rc::new(move |child| {
                *child_for_start.borrow_mut() = Some(child);
                actions_for_start.borrow_mut().push("started");
            })),
            communication_error: false,
            wait_error: None,
            wait_failed: None,
            io_settled: Some(Rc::new(move || {
                actions_for_io.borrow_mut().push("io");
            })),
            io_fault: None,
            cancellation_accepted: None,
            wait_completed: Some(Rc::new(move || actions_for_wait.borrow_mut().push("wait"))),
            term_sent: Some(Rc::new(move || actions_for_term.borrow_mut().push("term"))),
            force_exited: Some(Rc::new(move || {
                actions_for_force.borrow_mut().push("force");
            })),
            grace_checkpoint: Some(Rc::new(move |advance| {
                *held_grace_for_config.borrow_mut() = Some(advance);
            })),
        },
    );
    spin_until(&context, Duration::from_secs(10), || {
        actions.borrow().contains(&"started")
    });
    fixture.wait_until_ready(&context);
    spin_until(&context, Duration::from_secs(10), || {
        actions.borrow().contains(&"deadline")
    });
    spin_until(&context, Duration::from_secs(10), || {
        held_grace.borrow().is_some()
    });
    assert!(
        held_grace.borrow().is_some(),
        "the grace callback must be held explicitly"
    );
    assert!(!actions.borrow().contains(&"io"));
    assert!(
        results.borrow().is_empty(),
        "a mutating child must remain supervised during the grace checkpoint"
    );
    assert!(!actions.borrow().contains(&"force"));
    assert!(!actions.borrow().contains(&"term"));
    fixture.release();
    spin_until(&context, Duration::from_secs(10), || {
        actions.borrow().contains(&"io")
    });
    spin_until(&context, Duration::from_secs(10), || {
        !results.borrow().is_empty()
    });
    assert_eq!(results.borrow().len(), 1);
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::TimedOut))
    ));
    assert!(actions.borrow().contains(&"wait"));
    let _advance = held_grace.borrow_mut().take();
}

#[test]
fn cancelled_mutation_defers_terminal_callback_until_reaped() {
    let _guard = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = ControlledChild::new();
    let results: Rc<RefCell<Vec<Result<super::GitRunOutput, GitProcessError>>>> =
        Rc::new(RefCell::new(Vec::new()));
    let results_for_callback = Rc::clone(&results);
    let child_for_start = Rc::clone(&fixture.child);
    let settled = Rc::new(Cell::new(false));
    let settled_for_config = Rc::clone(&settled);
    let accepted = Rc::new(Cell::new(false));
    let accepted_for_config = Rc::clone(&accepted);
    let cancellable = gio::Cancellable::new();
    run_git_with_deadlines(
        fixture.spec(),
        &cancellable,
        Rc::new(move |result| results_for_callback.borrow_mut().push(result)),
        GitDeadlineConfig {
            operation: Duration::from_secs(5),
            grace: Duration::from_millis(100),
            deadline_fired: None,
            child_started: Some(Rc::new(move |child| {
                *child_for_start.borrow_mut() = Some(child);
            })),
            communication_error: false,
            wait_error: None,
            wait_failed: None,
            io_settled: Some(Rc::new(move || settled_for_config.set(true))),
            io_fault: None,
            cancellation_accepted: Some(Rc::new(move || accepted_for_config.set(true))),
            wait_completed: None,
            term_sent: None,
            force_exited: None,
            grace_checkpoint: None,
        },
    );
    fixture.wait_until_ready(&context);
    cancellable.cancel();
    spin_until(&context, Duration::from_secs(10), || accepted.get());
    assert!(!settled.get());
    assert!(
        results.borrow().is_empty(),
        "mutating cancellation must retain the operation until the child reaps"
    );
    fixture.release();
    spin_until(&context, Duration::from_secs(10), || settled.get());
    spin_until(&context, Duration::from_secs(10), || {
        fixture.exited.is_file()
    });
    assert!(
        fixture.exited.is_file(),
        "controlled child must acknowledge release"
    );
    spin_until(&context, Duration::from_secs(10), || {
        !results.borrow().is_empty()
    });
    assert_eq!(results.borrow().len(), 1);
    assert!(
        matches!(
            results.borrow().first(),
            Some(Err(GitProcessError::Cancelled))
        ),
        "expected terminal cancellation after release, got {:?}",
        results.borrow().first()
    );
}

#[test]
fn cancelled_mutation_keeps_cancelled_reason_when_its_deadline_later_fires() {
    let _guard = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = ControlledChild::new();
    let child_for_start = Rc::clone(&fixture.child);
    let results: Rc<RefCell<Vec<Result<super::GitRunOutput, GitProcessError>>>> =
        Rc::new(RefCell::new(Vec::new()));
    let results_for_callback = Rc::clone(&results);
    let deadline_fired = Rc::new(Cell::new(false));
    let deadline_for_config = Rc::clone(&deadline_fired);
    let held_grace: HeldGrace = Rc::new(RefCell::new(None));
    let held_grace_for_config = Rc::clone(&held_grace);
    let settled = Rc::new(Cell::new(false));
    let settled_for_config = Rc::clone(&settled);
    let accepted = Rc::new(Cell::new(false));
    let accepted_for_config = Rc::clone(&accepted);
    let cancellable = gio::Cancellable::new();
    run_git_with_deadlines(
        fixture.spec(),
        &cancellable,
        Rc::new(move |result| results_for_callback.borrow_mut().push(result)),
        GitDeadlineConfig {
            operation: Duration::from_secs(3),
            grace: Duration::from_millis(20),
            deadline_fired: Some(Rc::new(move || deadline_for_config.set(true))),
            child_started: Some(Rc::new(move |child| {
                *child_for_start.borrow_mut() = Some(child);
            })),
            communication_error: false,
            wait_error: None,
            wait_failed: None,
            io_settled: Some(Rc::new(move || settled_for_config.set(true))),
            io_fault: None,
            cancellation_accepted: Some(Rc::new(move || accepted_for_config.set(true))),
            wait_completed: None,
            term_sent: None,
            force_exited: None,
            grace_checkpoint: Some(Rc::new(move |advance| {
                *held_grace_for_config.borrow_mut() = Some(advance);
            })),
        },
    );
    fixture.wait_until_ready(&context);
    cancellable.cancel();
    spin_until(&context, Duration::from_secs(10), || accepted.get());
    assert!(!settled.get());
    assert!(
        results.borrow().is_empty(),
        "cancellation must remain pending before deadline"
    );
    spin_until(&context, Duration::from_secs(10), || deadline_fired.get());
    spin_until(&context, Duration::from_secs(10), || {
        held_grace.borrow().is_some()
    });
    assert!(results.borrow().is_empty());
    fixture.release();
    spin_until(&context, Duration::from_secs(10), || settled.get());
    spin_until(&context, Duration::from_secs(10), || {
        !results.borrow().is_empty()
    });
    assert_eq!(results.borrow().len(), 1);
    assert!(
        matches!(
            results.borrow().first(),
            Some(Err(GitProcessError::Cancelled))
        ),
        "cancellation must keep first terminal reason, got {:?}",
        results.borrow().first()
    );
    let _advance = held_grace.borrow_mut().take();
}

struct ControlledChild {
    directory: PathBuf,
    ready: PathBuf,
    release: PathBuf,
    exited: PathBuf,
    child: Rc<RefCell<Option<super::TestChild>>>,
}

impl ControlledChild {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("riteed-git-lifecycle-{}", std::process::id()));
        let _removed = fs::remove_dir_all(&directory);
        assert!(
            fs::create_dir_all(&directory).is_ok(),
            "controlled fixture directory must exist"
        );
        Self {
            ready: directory.join("ready"),
            release: directory.join("release"),
            exited: directory.join("exited"),
            child: Rc::new(RefCell::new(None)),
            directory,
        }
    }

    fn spec(&self) -> GitSpec {
        GitSpec {
            argv: vec![
                String::from("/usr/bin/python3"),
                String::from("-c"),
                String::from(
                    "import pathlib, sys, time\nready, release, exited = map(pathlib.Path, sys.argv[1:])\nready.write_text('ready')\nwhile not release.exists(): time.sleep(0.005)\nexited.write_text('exited')",
                ),
                self.ready.to_string_lossy().into_owned(),
                self.release.to_string_lossy().into_owned(),
                self.exited.to_string_lossy().into_owned(),
            ],
            env: Vec::new(),
            stdin: None,
            stdout_cap: 1024,
            allow_failure: false,
            kill_on_cancel: false,
        }
    }

    fn wait_until_ready(&self, context: &glib::MainContext) {
        spin_until(context, Duration::from_secs(10), || self.ready.is_file());
        assert!(
            self.ready.is_file(),
            "controlled child must report readiness"
        );
    }

    fn release(&self) {
        assert!(
            fs::write(&self.release, b"release").is_ok(),
            "controlled child must receive release"
        );
    }
}

impl Drop for ControlledChild {
    fn drop(&mut self) {
        if let Some(child) = self.child.borrow_mut().take() {
            let reaped = child.force_reap();
            if !std::thread::panicking() {
                assert!(reaped, "controlled child teardown must reap");
            }
        }
        let _removed = fs::remove_dir_all(&self.directory);
    }
}

fn spin_until(context: &glib::MainContext, timeout: Duration, done: impl Fn() -> bool) {
    let timed_out = Rc::new(Cell::new(false));
    let timed_out_for_source = Rc::clone(&timed_out);
    let source = glib::timeout_add_local_once(timeout, move || {
        timed_out_for_source.set(true);
    });
    let tick = glib::timeout_add_local(Duration::from_millis(5), || glib::ControlFlow::Continue);
    while !done() && !timed_out.get() {
        let _dispatched = context.iteration(true);
    }
    tick.remove();
    if !timed_out.get() {
        source.remove();
    }
    assert!(done(), "event watchdog expired");
}
