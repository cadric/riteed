use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

use super::io_pump::IO_CHUNK_BYTE_LIMIT;
use super::ops::STATUS_CAP;
use super::test_hooks::install_output_peak_observer;
use super::{
    GIT_BLOB_BYTE_LIMIT, GitDeadlineConfig, GitProcessError, GitRunOutput, GitSpec, STDERR_CAP,
    run_git_with_deadlines,
};

const FLOOD_BYTES: usize = 256 * 1024;
const TEST_CAP: usize = 32;
const EXPECTED_BOUNDED_PEAK: usize = TEST_CAP + 1 + 2 * IO_CHUNK_BYTE_LIMIT;
type HeldGrace = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

struct OutputRun {
    mode: &'static str,
    count: usize,
    stdin: Vec<u8>,
    stdout_cap: usize,
    kill_on_cancel: bool,
}

const FLOOD_CHILD: &str = r"
import pathlib, sys, time
ready, emit, release, drained, exited = map(pathlib.Path, sys.argv[1:6])
mode, count, stdin_count = sys.argv[6], int(sys.argv[7]), int(sys.argv[8])
ready.write_text('ready')
while not emit.exists(): time.sleep(0.005)
if mode in ('stdout', 'both'):
    sys.stdout.buffer.write(b'x' * count)
    sys.stdout.buffer.flush()
if mode in ('stderr', 'both'):
    sys.stderr.buffer.write(b'e' * count)
    sys.stderr.buffer.flush()
if len(sys.stdin.buffer.read()) != stdin_count:
    sys.exit(9)
drained.write_text('drained')
while not release.exists(): time.sleep(0.005)
exited.write_text('exited')
";

const REAPED_PIPE_HOLDER: &str = r"
import os, pathlib, sys, time
ready, release, pid_file, exited = map(pathlib.Path, sys.argv[1:5])
if os.fork() == 0:
    pid_file.write_text(str(os.getpid()))
    ready.write_text('ready')
    deadline = time.monotonic() + 15
    while not release.exists() and time.monotonic() < deadline: time.sleep(0.005)
    try: exited.write_text('exited')
    except OSError: pass
    sys.exit(0)
sys.exit(0)
";

#[test]
fn stdout_overflow_retains_only_cap_plus_bounded_chunk() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("stdout-red");
    let peak = Rc::new(Cell::new(0_usize));
    let peak_for_observer = Rc::clone(&peak);
    let _peak_guard = install_output_peak_observer(Rc::new(move |bytes| {
        peak_for_observer.set(peak_for_observer.get().max(bytes));
    }));
    let cancellable = gio::Cancellable::new();
    let results = fixture.start(
        OutputRun {
            mode: "stdout",
            count: FLOOD_BYTES,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: true,
        },
        &cancellable,
        |_| {},
    );

    fixture.wait_ready();
    fixture.emit();
    fixture.release();
    pump("bounded output callback", || !results.borrow().is_empty());

    assert!(fixture.reaped.get(), "controlled child must be reaped");
    assert!(fixture.forced.get(), "read-only overflow must force exit");
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::OutputTooLarge))
    ));
    assert!(
        peak.get() <= EXPECTED_BOUNDED_PEAK,
        "peak logical output bytes {} exceeded {EXPECTED_BOUNDED_PEAK}",
        peak.get()
    );
}

#[test]
fn exact_stdout_cap_is_accepted_without_forcing_the_child() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("exact-cap");
    let results = fixture.start(
        OutputRun {
            mode: "stdout",
            count: TEST_CAP,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: true,
        },
        &gio::Cancellable::new(),
        |_| {},
    );
    fixture.wait_ready();
    fixture.emit();
    fixture.release();
    pump("exact-cap output callback", || !results.borrow().is_empty());
    assert!(!fixture.forced.get());
    assert!(fixture.reaped.get());
    let results = results.borrow();
    assert!(matches!(results.first(), Some(Ok(_))));
    if let Some(Ok(output)) = results.first() {
        assert_eq!(output.stdout, vec![b'x'; TEST_CAP]);
    }
}

#[test]
fn stderr_overflow_is_bounded_and_reaped() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("stderr-cap");
    let peak = Rc::new(Cell::new(0_usize));
    let peak_for_observer = Rc::clone(&peak);
    let _peak_guard = install_output_peak_observer(Rc::new(move |bytes| {
        peak_for_observer.set(peak_for_observer.get().max(bytes));
    }));
    let results = fixture.start(
        OutputRun {
            mode: "stderr",
            count: STDERR_CAP + 1,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: true,
        },
        &gio::Cancellable::new(),
        |_| {},
    );
    fixture.wait_ready();
    fixture.emit();
    fixture.release();
    pump("stderr overflow callback", || !results.borrow().is_empty());
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::OutputTooLarge))
    ));
    assert!(fixture.reaped.get());
    assert!(
        peak.get() <= STDERR_CAP + 1 + 2 * IO_CHUNK_BYTE_LIMIT,
        "stderr peak was {} logical bytes",
        peak.get()
    );
}

#[test]
fn blob_and_status_caps_stop_retaining_after_the_sentinel() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    for (label, cap) in [
        ("blob-cap", GIT_BLOB_BYTE_LIMIT),
        ("status-cap", STATUS_CAP),
    ] {
        let fixture = OutputFixture::new(label);
        let peak = Rc::new(Cell::new(0_usize));
        let peak_for_observer = Rc::clone(&peak);
        let peak_guard = install_output_peak_observer(Rc::new(move |bytes| {
            peak_for_observer.set(peak_for_observer.get().max(bytes));
        }));
        let results = fixture.start(
            OutputRun {
                mode: "stdout",
                count: cap + IO_CHUNK_BYTE_LIMIT,
                stdin: Vec::new(),
                stdout_cap: cap,
                kill_on_cancel: true,
            },
            &gio::Cancellable::new(),
            |_| {},
        );
        fixture.wait_ready();
        fixture.emit();
        fixture.release();
        pump("profile overflow callback", || !results.borrow().is_empty());
        assert!(matches!(
            results.borrow().first(),
            Some(Err(GitProcessError::OutputTooLarge))
        ));
        assert!(peak.get() <= cap + 1 + 2 * IO_CHUNK_BYTE_LIMIT);
        drop(peak_guard);
    }
}

#[test]
fn output_before_large_stdin_progresses_all_three_pipes() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("concurrent-pipes");
    let count = 48 * 1024;
    fixture.emit();
    fixture.release();
    let results = fixture.start(
        OutputRun {
            mode: "both",
            count,
            stdin: vec![b'i'; 256 * 1024],
            stdout_cap: count,
            kill_on_cancel: true,
        },
        &gio::Cancellable::new(),
        |_| {},
    );
    fixture.wait_ready();
    pump("concurrent pipe callback", || !results.borrow().is_empty());
    assert!(
        fixture.drained.is_file(),
        "child must consume the complete stdin"
    );
    assert!(!fixture.forced.get());
    let results = results.borrow();
    assert!(matches!(results.first(), Some(Ok(_))));
    if let Some(Ok(output)) = results.first() {
        assert_eq!(output.stdout.len(), count);
    }
}

#[test]
fn mutation_overflow_drains_and_finishes_naturally_during_held_grace() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("mutation-grace");
    let io_fault = Rc::new(Cell::new(false));
    let io_fault_for_config = Rc::clone(&io_fault);
    let held_grace: HeldGrace = Rc::new(RefCell::new(None));
    let held_for_config = Rc::clone(&held_grace);
    let term_sent = Rc::new(Cell::new(false));
    let term_for_config = Rc::clone(&term_sent);
    let results = fixture.start(
        OutputRun {
            mode: "stdout",
            count: FLOOD_BYTES,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: false,
        },
        &gio::Cancellable::new(),
        move |config| {
            config.operation = Duration::from_millis(100);
            config.grace = Duration::from_millis(30);
            config.io_fault = Some(Rc::new(move || io_fault_for_config.set(true)));
            config.grace_checkpoint = Some(Rc::new(move |advance| {
                *held_for_config.borrow_mut() = Some(advance);
            }));
            config.term_sent = Some(Rc::new(move || term_for_config.set(true)));
        },
    );
    fixture.wait_ready();
    fixture.emit();
    pump("mutation output overflow", || io_fault.get());
    pump("mutation output drain", || fixture.drained.is_file());
    assert!(!fixture.forced.get());
    assert!(results.borrow().is_empty());
    pump("held mutation grace", || held_grace.borrow().is_some());
    fixture.release();
    pump("natural mutation completion", || {
        !results.borrow().is_empty()
    });
    assert!(fixture.exited.is_file());
    assert!(fixture.reaped.get());
    assert!(!fixture.forced.get());
    assert!(!term_sent.get());
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::OutputTooLarge))
    ));
    if let Some(advance) = held_grace.borrow_mut().take() {
        advance();
    }
    assert!(!term_sent.get());
}

#[test]
fn already_cancelled_read_only_run_is_supervised_and_reaped_once() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("already-cancelled");
    let cancellable = gio::Cancellable::new();
    cancellable.cancel();
    let accepted = Rc::new(Cell::new(false));
    let accepted_for_config = Rc::clone(&accepted);
    let results = fixture.start(
        OutputRun {
            mode: "stdout",
            count: TEST_CAP,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: true,
        },
        &cancellable,
        move |config| {
            config.cancellation_accepted = Some(Rc::new(move || accepted_for_config.set(true)));
        },
    );
    pump("already-cancelled terminal callback", || {
        !results.borrow().is_empty()
    });
    assert!(accepted.get());
    assert!(fixture.forced.get());
    assert!(fixture.reaped.get());
    assert_eq!(results.borrow().len(), 1);
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::Cancelled))
    ));
}

#[test]
fn output_overflow_remains_first_when_cancellation_races_cleanup() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("overflow-before-cancel");
    let cancellable = gio::Cancellable::new();
    let cancel_for_fault = cancellable.clone();
    let results = fixture.start(
        OutputRun {
            mode: "stdout",
            count: FLOOD_BYTES,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: true,
        },
        &cancellable,
        move |config| {
            config.io_fault = Some(Rc::new(move || cancel_for_fault.cancel()));
        },
    );
    fixture.wait_ready();
    fixture.emit();
    fixture.release();
    pump("overflow/cancellation race", || {
        !results.borrow().is_empty()
    });
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::OutputTooLarge))
    ));
}

#[test]
fn accepted_cancellation_remains_first_during_final_output_drain() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = OutputFixture::new("cancel-before-drain");
    let cancellable = gio::Cancellable::new();
    let accepted = Rc::new(Cell::new(false));
    let accepted_for_config = Rc::clone(&accepted);
    let results = fixture.start(
        OutputRun {
            mode: "stdout",
            count: FLOOD_BYTES,
            stdin: Vec::new(),
            stdout_cap: TEST_CAP,
            kill_on_cancel: false,
        },
        &cancellable,
        move |config| {
            config.cancellation_accepted = Some(Rc::new(move || accepted_for_config.set(true)));
        },
    );
    fixture.wait_ready();
    cancellable.cancel();
    pump("cancellation acceptance", || accepted.get());
    fixture.emit();
    fixture.release();
    pump("cancelled final drain", || !results.borrow().is_empty());
    assert!(!fixture.forced.get());
    assert!(fixture.reaped.get());
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::Cancelled))
    ));
}

#[test]
fn timeout_cleans_pipes_held_after_the_direct_child_is_reaped() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = PipeHolderFixture::new("timeout-after-reap");
    let results = fixture.start(&gio::Cancellable::new(), Duration::from_millis(50));
    pump("descendant inherits direct-child pipes", || {
        fixture.is_ready()
    });
    pump("timeout closes inherited pipes", || {
        !results.borrow().is_empty()
    });
    assert!(
        fixture.reaped.get(),
        "direct child must reap before terminal"
    );
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::TimedOut))
    ));
}

#[test]
fn cancellation_cleans_pipes_held_after_the_direct_child_is_reaped() {
    let _lock = crate::test_support::lock_for_tests();
    let context = glib::MainContext::default();
    let acquired = context.acquire();
    assert!(acquired.is_ok(), "test must own the GLib main context");
    let _context_guard = acquired.ok();
    let fixture = PipeHolderFixture::new("cancel-after-reap");
    let cancellable = gio::Cancellable::new();
    let results = fixture.start(&cancellable, Duration::from_secs(10));
    pump("descendant inherits direct-child pipes", || {
        fixture.is_ready() && fixture.reaped.get()
    });
    cancellable.cancel();
    pump("cancellation closes inherited pipes", || {
        !results.borrow().is_empty()
    });
    assert!(matches!(
        results.borrow().first(),
        Some(Err(GitProcessError::Cancelled))
    ));
}

struct PipeHolderFixture {
    directory: PathBuf,
    ready: PathBuf,
    release: PathBuf,
    pid_file: PathBuf,
    exited: PathBuf,
    child: Rc<RefCell<Option<super::TestChild>>>,
    reaped: Rc<Cell<bool>>,
}

impl PipeHolderFixture {
    fn new(label: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "riteed-git-pipe-holder-{}-{label}",
            std::process::id()
        ));
        let _removed = fs::remove_dir_all(&directory);
        assert!(fs::create_dir(&directory).is_ok());
        Self {
            ready: directory.join("ready"),
            release: directory.join("release"),
            pid_file: directory.join("pid"),
            exited: directory.join("exited"),
            child: Rc::new(RefCell::new(None)),
            reaped: Rc::new(Cell::new(false)),
            directory,
        }
    }

    fn start(
        &self,
        cancellable: &gio::Cancellable,
        operation: Duration,
    ) -> Rc<RefCell<Vec<Result<GitRunOutput, GitProcessError>>>> {
        let child = Rc::clone(&self.child);
        let reaped = Rc::clone(&self.reaped);
        let results = Rc::new(RefCell::new(Vec::new()));
        let results_for_callback = Rc::clone(&results);
        run_git_with_deadlines(
            GitSpec {
                argv: vec![
                    String::from("/usr/bin/python3"),
                    String::from("-c"),
                    String::from(REAPED_PIPE_HOLDER),
                    self.ready.to_string_lossy().into_owned(),
                    self.release.to_string_lossy().into_owned(),
                    self.pid_file.to_string_lossy().into_owned(),
                    self.exited.to_string_lossy().into_owned(),
                ],
                env: Vec::new(),
                stdin: None,
                stdout_cap: TEST_CAP,
                allow_failure: false,
                kill_on_cancel: true,
            },
            cancellable,
            Rc::new(move |result| results_for_callback.borrow_mut().push(result)),
            GitDeadlineConfig {
                operation,
                child_started: Some(Rc::new(move |started| {
                    *child.borrow_mut() = Some(started);
                })),
                wait_completed: Some(Rc::new(move || reaped.set(true))),
                ..GitDeadlineConfig::production()
            },
        );
        results
    }

    fn is_ready(&self) -> bool {
        self.ready.is_file()
            && fs::read_to_string(&self.pid_file)
                .is_ok_and(|pid| pid.trim().parse::<u32>().is_ok_and(|pid| pid > 0))
    }
}

impl Drop for PipeHolderFixture {
    fn drop(&mut self) {
        let _released = fs::write(&self.release, b"release");
        let expired = Rc::new(Cell::new(false));
        let expired_for_source = Rc::clone(&expired);
        let deadline = glib::timeout_add_local_once(Duration::from_secs(20), move || {
            expired_for_source.set(true);
        });
        let tick =
            glib::timeout_add_local(Duration::from_millis(5), || glib::ControlFlow::Continue);
        let context = glib::MainContext::default();
        while !self.exited.is_file() && !expired.get() {
            let _dispatched = context.iteration(true);
        }
        tick.remove();
        if !expired.get() {
            deadline.remove();
        }
        if let Some(child) = self.child.borrow_mut().take() {
            let _reaped = child.force_reap();
        }
        if self.exited.is_file() {
            let _removed = fs::remove_dir_all(&self.directory);
        }
    }
}

struct OutputFixture {
    directory: PathBuf,
    ready: PathBuf,
    emit: PathBuf,
    release: PathBuf,
    drained: PathBuf,
    exited: PathBuf,
    child: Rc<RefCell<Option<super::TestChild>>>,
    reaped: Rc<Cell<bool>>,
    forced: Rc<Cell<bool>>,
}

impl OutputFixture {
    fn new(label: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("riteed-git-output-{}-{label}", std::process::id()));
        let _removed = fs::remove_dir_all(&directory);
        assert!(fs::create_dir(&directory).is_ok());
        Self {
            ready: directory.join("ready"),
            emit: directory.join("emit"),
            release: directory.join("release"),
            drained: directory.join("drained"),
            exited: directory.join("exited"),
            child: Rc::new(RefCell::new(None)),
            reaped: Rc::new(Cell::new(false)),
            forced: Rc::new(Cell::new(false)),
            directory,
        }
    }

    fn start(
        &self,
        run: OutputRun,
        cancellable: &gio::Cancellable,
        configure: impl FnOnce(&mut GitDeadlineConfig),
    ) -> Rc<RefCell<Vec<Result<GitRunOutput, GitProcessError>>>> {
        let child = Rc::clone(&self.child);
        let reaped = Rc::clone(&self.reaped);
        let forced = Rc::clone(&self.forced);
        let results = Rc::new(RefCell::new(Vec::new()));
        let results_for_callback = Rc::clone(&results);
        let mut deadlines = GitDeadlineConfig {
            operation: Duration::from_secs(10),
            child_started: Some(Rc::new(move |started| {
                *child.borrow_mut() = Some(started);
            })),
            wait_completed: Some(Rc::new(move || reaped.set(true))),
            force_exited: Some(Rc::new(move || forced.set(true))),
            ..GitDeadlineConfig::production()
        };
        configure(&mut deadlines);
        run_git_with_deadlines(
            GitSpec {
                argv: vec![
                    String::from("/usr/bin/python3"),
                    String::from("-c"),
                    String::from(FLOOD_CHILD),
                    self.ready.to_string_lossy().into_owned(),
                    self.emit.to_string_lossy().into_owned(),
                    self.release.to_string_lossy().into_owned(),
                    self.drained.to_string_lossy().into_owned(),
                    self.exited.to_string_lossy().into_owned(),
                    String::from(run.mode),
                    run.count.to_string(),
                    run.stdin.len().to_string(),
                ],
                env: Vec::new(),
                stdin: Some(run.stdin),
                stdout_cap: run.stdout_cap,
                allow_failure: false,
                kill_on_cancel: run.kill_on_cancel,
            },
            cancellable,
            Rc::new(move |result| results_for_callback.borrow_mut().push(result)),
            deadlines,
        );
        results
    }

    fn wait_ready(&self) {
        pump("controlled output child readiness", || self.ready.is_file());
    }

    fn release(&self) {
        assert!(fs::write(&self.release, b"release").is_ok());
    }

    fn emit(&self) {
        assert!(fs::write(&self.emit, b"emit").is_ok());
    }
}

impl Drop for OutputFixture {
    fn drop(&mut self) {
        if let Some(child) = self.child.borrow_mut().take() {
            let reaped = child.force_reap();
            if !std::thread::panicking() {
                assert!(reaped, "output fixture teardown must reap");
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
