use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, prelude::*};
use libadwaita as adw;

use crate::git_process::TestChild;
use crate::git_process::test_hooks::{
    self, ControlledMutation, ControlledMutationGuard, GraceAdvance, Hooks, Resume,
};
use crate::git_process::test_support::{
    FixtureRepoFile, FixtureRepoKind, ModifiedFixtureRepo, init_modified_fixture_repo_for_tests,
};
use crate::gtk_tests::{build_window, drain_events, spin_until};
use crate::source_control::{SourceControlState, actions::GitRowAction};
use crate::window::Window;

type HeldStatus = Rc<RefCell<Option<Resume>>>;

#[derive(Default)]
struct LifecycleObservers {
    deadline_fired: Option<Rc<dyn Fn()>>,
    io_settled: Option<Rc<dyn Fn()>>,
    cancellation_accepted: Option<Rc<dyn Fn()>>,
    wait_completed: Option<Rc<dyn Fn()>>,
    grace_checkpoint: Option<Rc<dyn Fn(GraceAdvance)>>,
}

struct GraceSignals {
    held_status: HeldStatus,
    deadline_fired: Rc<Cell<bool>>,
    io_settled: Rc<Cell<bool>>,
    wait_completed: Rc<Cell<bool>>,
    held_grace: Rc<RefCell<Option<GraceAdvance>>>,
}

struct ControlledChild {
    ready: PathBuf,
    release: PathBuf,
    argv: Vec<String>,
}

impl ControlledChild {
    fn new(repo: &Path, name: &str, exit_code: i32, partial_path: Option<&Path>) -> Self {
        let directory = repo.join(".git/riteed-controlled-mutation");
        assert!(fs::create_dir_all(&directory).is_ok());
        let script = directory.join(format!("{name}.py"));
        let ready = directory.join(format!("{name}.ready"));
        let release = directory.join(format!("{name}.release"));
        assert!(fs::write(&script, CONTROLLED_CHILD).is_ok());
        let partial = partial_path.map_or_else(String::new, |path| path.display().to_string());
        let argv = vec![
            String::from("/usr/bin/python3"),
            script.display().to_string(),
            ready.display().to_string(),
            release.display().to_string(),
            partial,
            exit_code.to_string(),
        ];
        Self {
            ready,
            release,
            argv,
        }
    }

    fn release(&self) {
        assert!(fs::write(&self.release, b"release\n").is_ok());
    }
}

impl Drop for ControlledChild {
    fn drop(&mut self) {
        let _released = fs::write(&self.release, b"release\n");
    }
}

const CONTROLLED_CHILD: &str = r#"import pathlib
import signal
import sys
import time

ready = pathlib.Path(sys.argv[1])
release = pathlib.Path(sys.argv[2])
partial = sys.argv[3]
signal.signal(signal.SIGTERM, lambda _signal, _frame: None)
ready.write_bytes(b"ready\n")
while not release.exists():
    time.sleep(0.005)
if partial:
    pathlib.Path(partial).write_bytes(b"partial mutation\n")
raise SystemExit(int(sys.argv[4]))
"#;

struct Fixture {
    repo_a: ModifiedFixtureRepo,
    repo_b: ModifiedFixtureRepo,
    window: Rc<Window>,
    state: Rc<RefCell<SourceControlState>>,
    children: Rc<RefCell<Vec<TestChild>>>,
    cancellations: Rc<RefCell<Vec<gio::Cancellable>>>,
    dispatches: Rc<RefCell<Vec<Vec<String>>>>,
    labels: Rc<RefCell<Vec<String>>>,
}

impl Fixture {
    fn new(app: &adw::Application) -> Option<Self> {
        let repo_a = modified_repo(FixtureRepoKind::V9_SOURCE_CONTROL_TRACKED)?;
        let repo_b = modified_repo(FixtureRepoKind::V9_SOURCE_CONTROL_UNTRACKED)?;
        let window = build_window(app)?;
        let state = window.source_control_state_weak_for_tests().upgrade()?;
        let fixture = Self {
            repo_a,
            repo_b,
            window,
            state,
            children: Rc::new(RefCell::new(Vec::new())),
            cancellations: Rc::new(RefCell::new(Vec::new())),
            dispatches: Rc::new(RefCell::new(Vec::new())),
            labels: Rc::new(RefCell::new(Vec::new())),
        };
        fixture.install_hooks(None);
        fixture
            .window
            .handle_application_open(vec![gio::File::for_path(fixture.repo_a.path())]);
        spin_until("initial repository status", || {
            fixture.current_repo(&fixture.repo_a)
        });
        fixture.disable_live_refresh();
        fixture.action(GitRowAction::Stage);
        spin_until("fixture has a staged change", || {
            fixture.has_entry_state(true, false)
        });
        assert!(fs::write(fixture.path_a(), b"changed again\n").is_ok());
        fixture.refresh();
        spin_until("fixture has staged and unstaged changes", || {
            fixture.has_entry_state(true, true)
        });
        fixture.disable_live_refresh();
        let label = fixture.state().borrow().status_label.clone();
        let labels = Rc::clone(&fixture.labels);
        label.connect_notify_local(Some("label"), move |label, _| {
            labels.borrow_mut().push(label.label().to_string());
        });
        Some(fixture)
    }

    fn state(&self) -> Rc<RefCell<SourceControlState>> {
        Rc::clone(&self.state)
    }

    fn path_a(&self) -> PathBuf {
        self.repo_a.file_path(FixtureRepoFile::BASELINE)
    }

    fn uri_a(&self) -> String {
        gio::File::for_path(self.path_a()).uri().to_string()
    }

    fn action(&self, action: GitRowAction) {
        self.window
            .source_control_run_action_for_tests(&self.uri_a(), action);
    }

    fn refresh(&self) {
        assert!(
            gtk4::prelude::WidgetExt::activate_action(
                self.window.widget(),
                "win.git-refresh",
                None
            )
            .is_ok()
        );
    }

    fn current_repo(&self, repo: &ModifiedFixtureRepo) -> bool {
        let state = self.state();
        let state = state.borrow();
        state.repo.as_deref() == Some(repo.path())
            && !state.status_stale
            && state.snapshot_id.is_some()
    }

    fn has_entry_state(&self, staged: bool, unstaged: bool) -> bool {
        let state = self.state();
        state
            .borrow()
            .snapshot
            .entries
            .iter()
            .any(|entry| entry.staged == staged && entry.unstaged == unstaged)
    }

    fn disable_live_refresh(&self) {
        let live = self.state().borrow_mut().live_refresh.take();
        drop(live);
    }

    fn install_hooks(
        &self,
        status: Option<test_hooks::Hold<crate::git_status::GitStatusSnapshot>>,
    ) {
        let children = Rc::clone(&self.children);
        let cancellations = Rc::clone(&self.cancellations);
        let dispatches = Rc::clone(&self.dispatches);
        test_hooks::install(Some(Hooks {
            repo: self.repo_a.path().to_path_buf(),
            status,
            blob: None,
            dispatch: Some(Rc::new(move |argv, _mutation, cancellable| {
                dispatches.borrow_mut().push(argv);
                cancellations.borrow_mut().push(cancellable);
            })),
            started: Some(Rc::new(move |child| children.borrow_mut().push(child))),
        }));
    }

    fn controlled(
        &self,
        child: &ControlledChild,
        operation: Duration,
        grace: Duration,
        observers: LifecycleObservers,
    ) -> ControlledMutationGuard {
        test_hooks::install_controlled_mutation(ControlledMutation {
            repo: self.repo_a.path().to_path_buf(),
            argv: child.argv.clone(),
            operation,
            grace,
            deadline_fired: observers.deadline_fired,
            io_settled: observers.io_settled,
            io_fault: None,
            cancellation_accepted: observers.cancellation_accepted,
            wait_completed: observers.wait_completed,
            grace_checkpoint: observers.grace_checkpoint,
        })
    }

    fn stage_index_dispatches(&self) -> usize {
        self.dispatches
            .borrow()
            .iter()
            .filter(|argv| is_stage_index(argv))
            .count()
    }

    fn commit_dispatches(&self) -> usize {
        self.dispatches
            .borrow()
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == "commit"))
            .count()
    }

    fn set_root(&self, repo: &ModifiedFixtureRepo) {
        self.window
            .set_source_control_project_root_for_tests(gio::File::for_path(repo.path()));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        test_hooks::install(None);
        for cancellable in self.cancellations.borrow_mut().drain(..) {
            cancellable.cancel();
        }
        for child in self.children.borrow_mut().drain(..) {
            let reaped = child.force_reap();
            if !std::thread::panicking() {
                assert!(reaped, "every observed child must be reaped");
            }
        }
        self.window.widget().destroy();
    }
}

pub(crate) fn exercise_mutation_grace_root_round_trip(app: &adw::Application) {
    exercise_grace_root_round_trip(app);
    exercise_user_cancellation(app);
    exercise_normal_completion(app);
    exercise_commit_success(app);
    exercise_partial_failure(app);
    exercise_spawn_failure(app);
    exercise_identity_failure(app);
}

fn exercise_grace_root_round_trip(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    let child = ControlledChild::new(fixture.repo_a.path(), "grace", 0, None);
    let (_guard, signals) = install_grace_control(&fixture, &child);

    let (commit_entry, commit_button) = {
        let state = fixture.state();
        let state = state.borrow();
        (state.commit_entry.clone(), state.commit_button.clone())
    };
    commit_entry.set_text("must remain pending");
    let children_before = fixture.children.borrow().len();
    let stage_dispatches_before = fixture.stage_index_dispatches();
    fixture.action(GitRowAction::Stage);
    spin_until("controlled writer reports ready", || child.ready.exists());
    spin_until("Task 1 deadline and grace checkpoints", || {
        signals.deadline_fired.get() && signals.held_grace.borrow().is_some()
    });
    assert!(!signals.io_settled.get());
    assert_grace_deadline_state(&fixture, &signals, children_before);
    let before_repeat = fixture.dispatches.borrow().len();
    fixture.action(GitRowAction::Stage);
    commit_button.emit_clicked();
    assert_eq!(fixture.dispatches.borrow().len(), before_repeat);
    assert_eq!(
        fixture.stage_index_dispatches(),
        stage_dispatches_before + 1
    );
    assert!(fixture.state().borrow().mutation_active_for_tests());

    fixture.set_root(&fixture.repo_b);
    spin_until("repository B becomes current during old grace", || {
        fixture.current_repo(&fixture.repo_b)
    });
    fixture.disable_live_refresh();
    fixture.set_root(&fixture.repo_a);
    spin_until("repository A returns while its writer is retiring", || {
        let state = fixture.state();
        let state = state.borrow();
        state.repo.as_deref() == Some(fixture.repo_a.path())
            && state.status_stale
            && state.mutation_active_for_tests()
    });
    fixture.disable_live_refresh();
    let old_timeout = gettextrs::gettext("The Git operation timed out.");
    fixture.labels.borrow_mut().clear();
    child.release();
    spin_until("controlled writer settles owned pipes", || {
        signals.io_settled.get()
    });
    spin_until("controlled writer reaches terminal wait", || {
        signals.wait_completed.get()
    });
    spin_until("fresh repository A status returned", || {
        signals.held_status.borrow().is_some()
    });
    assert!(!fixture.state().borrow().mutation_active_for_tests());
    assert!(
        !fixture
            .labels
            .borrow()
            .iter()
            .any(|label| label == &old_timeout),
        "retired writer result must not alter current-root UI"
    );
    let resume = signals.held_status.borrow_mut().take();
    assert!(resume.is_some(), "fresh status continuation must be held");
    if let Some(resume) = resume {
        resume();
    }
    spin_until("repository A recovers a current snapshot", || {
        fixture.current_repo(&fixture.repo_a) && fixture.has_entry_state(true, true)
    });
    assert_eq!(
        fixture.stage_index_dispatches(),
        stage_dispatches_before + 1
    );
    let advance = signals.held_grace.borrow_mut().take();
    assert!(advance.is_some(), "grace continuation must be held");
    if let Some(advance) = advance {
        advance();
    }
}

fn assert_grace_deadline_state(fixture: &Fixture, signals: &GraceSignals, children_before: usize) {
    assert_eq!(
        fixture.children.borrow().len(),
        children_before + 2,
        "hash and controlled mutation children are observed"
    );
    assert!(
        fixture
            .cancellations
            .borrow()
            .last()
            .is_some_and(gio::Cancellable::is_cancelled),
        "the real mutation cancellable is cancelled at its deadline"
    );
    assert!(
        !signals.wait_completed.get(),
        "terminal wait is still pending"
    );
    let state = fixture.state();
    let state = state.borrow();
    assert!(state.mutation_active_for_tests());
    assert!(state.status_stale);
    assert!(state.snapshot_id.is_none());
}

fn install_grace_control(
    fixture: &Fixture,
    child: &ControlledChild,
) -> (ControlledMutationGuard, GraceSignals) {
    let held_status: HeldStatus = Rc::new(RefCell::new(None));
    let held_status_for_hook = Rc::clone(&held_status);
    fixture.install_hooks(Some(Rc::new(move |result, resume| {
        assert!(
            result.is_ok(),
            "fresh post-mutation status child must succeed"
        );
        let snapshot = result.unwrap_or_default();
        assert_eq!(snapshot.entries.len(), 1);
        *held_status_for_hook.borrow_mut() = Some(resume);
    })));
    let deadline_fired = Rc::new(Cell::new(false));
    let io_settled = Rc::new(Cell::new(false));
    let wait_completed = Rc::new(Cell::new(false));
    let held_grace: Rc<RefCell<Option<GraceAdvance>>> = Rc::new(RefCell::new(None));
    let deadline_for_hook = Rc::clone(&deadline_fired);
    let io_for_hook = Rc::clone(&io_settled);
    let wait_for_hook = Rc::clone(&wait_completed);
    let grace_for_hook = Rc::clone(&held_grace);
    let guard = fixture.controlled(
        child,
        Duration::from_millis(30),
        Duration::from_millis(30),
        LifecycleObservers {
            deadline_fired: Some(Rc::new(move || deadline_for_hook.set(true))),
            io_settled: Some(Rc::new(move || io_for_hook.set(true))),
            cancellation_accepted: None,
            wait_completed: Some(Rc::new(move || wait_for_hook.set(true))),
            grace_checkpoint: Some(Rc::new(move |advance| {
                *grace_for_hook.borrow_mut() = Some(advance);
            })),
        },
    );
    (
        guard,
        GraceSignals {
            held_status,
            deadline_fired,
            io_settled,
            wait_completed,
            held_grace,
        },
    )
}

fn exercise_user_cancellation(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    fixture.install_hooks(None);
    let child = ControlledChild::new(fixture.repo_a.path(), "cancel", 0, None);
    let deadline_fired = Rc::new(Cell::new(false));
    let deadline_for_hook = Rc::clone(&deadline_fired);
    let io_settled = Rc::new(Cell::new(false));
    let io_for_hook = Rc::clone(&io_settled);
    let cancellation_accepted = Rc::new(Cell::new(false));
    let cancellation_for_hook = Rc::clone(&cancellation_accepted);
    let wait_completed = Rc::new(Cell::new(false));
    let wait_for_hook = Rc::clone(&wait_completed);
    let _guard = fixture.controlled(
        &child,
        Duration::from_secs(5),
        Duration::from_millis(30),
        LifecycleObservers {
            deadline_fired: Some(Rc::new(move || deadline_for_hook.set(true))),
            io_settled: Some(Rc::new(move || io_for_hook.set(true))),
            cancellation_accepted: Some(Rc::new(move || cancellation_for_hook.set(true))),
            wait_completed: Some(Rc::new(move || wait_for_hook.set(true))),
            grace_checkpoint: None,
        },
    );
    let (commit_entry, commit_button) = {
        let state = fixture.state();
        let state = state.borrow();
        (state.commit_entry.clone(), state.commit_button.clone())
    };
    commit_entry.set_text("cancelled writer remains owned");
    fixture.action(GitRowAction::Stage);
    spin_until("cancellable writer reports ready", || child.ready.exists());
    let cancellable = fixture.cancellations.borrow().last().cloned();
    assert!(
        cancellable.is_some(),
        "actual mutation cancellable must exist"
    );
    if let Some(cancellable) = cancellable {
        cancellable.cancel();
    }
    spin_until("supervisor accepts user cancellation", || {
        cancellation_accepted.get()
    });
    assert!(!io_settled.get());
    assert!(!deadline_fired.get());
    assert!(!wait_completed.get());
    assert!(fixture.state().borrow().mutation_active_for_tests());
    let before_repeat = fixture.dispatches.borrow().len();
    fixture.action(GitRowAction::Stage);
    commit_button.emit_clicked();
    assert_eq!(fixture.dispatches.borrow().len(), before_repeat);
    child.release();
    spin_until("cancelled child settles owned pipes", || io_settled.get());
    spin_until("cancelled child reaches terminal wait", || {
        wait_completed.get()
    });
    spin_until("cancelled mutation refreshes status", || {
        fixture.current_repo(&fixture.repo_a)
    });
    assert!(!fixture.state().borrow().mutation_active_for_tests());
}

fn exercise_normal_completion(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    fixture.install_hooks(None);
    let child = ControlledChild::new(fixture.repo_a.path(), "success", 0, None);
    let wait_completed = Rc::new(Cell::new(false));
    let wait_for_hook = Rc::clone(&wait_completed);
    let _guard = fixture.controlled(
        &child,
        Duration::from_secs(5),
        Duration::from_millis(30),
        LifecycleObservers {
            wait_completed: Some(Rc::new(move || wait_for_hook.set(true))),
            ..LifecycleObservers::default()
        },
    );
    fixture.action(GitRowAction::Stage);
    spin_until("normal controlled writer reports ready", || {
        child.ready.exists()
    });
    assert!(fixture.state().borrow().mutation_active_for_tests());
    child.release();
    spin_until("normal controlled writer is reaped", || {
        wait_completed.get()
    });
    spin_until("normal completion refreshes status", || {
        fixture.current_repo(&fixture.repo_a)
    });
    assert!(!fixture.state().borrow().mutation_active_for_tests());
    fixture.action(GitRowAction::Stage);
    spin_until("next writer starts after terminal completion", || {
        fixture.has_entry_state(true, false)
    });
}

fn exercise_commit_success(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    let (old_head, entry, button) = {
        let state = fixture.state();
        let state = state.borrow();
        (
            state.snapshot.head_oid.clone(),
            state.commit_entry.clone(),
            state.commit_button.clone(),
        )
    };
    assert!(old_head.is_some(), "fixture must start with a HEAD commit");
    let commits_before = fixture.commit_dispatches();
    entry.set_text("public commit succeeds once");
    assert!(button.is_sensitive());
    button.emit_clicked();
    spin_until("public commit publishes a fresh same-root snapshot", || {
        let state = fixture.state();
        let state = state.borrow();
        state.repo.as_deref() == Some(fixture.repo_a.path())
            && !state.status_stale
            && state.snapshot_id.is_some()
            && state.snapshot.head_oid.is_some()
            && state.snapshot.head_oid != old_head
            && state
                .snapshot
                .entries
                .iter()
                .any(|item| !item.staged && item.unstaged)
            && !state.mutation_active_for_tests()
    });
    assert!(entry.text().is_empty());
    assert_eq!(fixture.commit_dispatches(), commits_before + 1);
    drain_events(8);
    assert_eq!(fixture.commit_dispatches(), commits_before + 1);
}

fn exercise_partial_failure(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    fixture.install_hooks(None);
    let partial_path = fixture.path_a();
    let child = ControlledChild::new(
        fixture.repo_a.path(),
        "partial-failure",
        7,
        Some(&partial_path),
    );
    let _guard = fixture.controlled(
        &child,
        Duration::from_secs(5),
        Duration::from_millis(30),
        LifecycleObservers::default(),
    );
    fixture.action(GitRowAction::Stage);
    spin_until("partial-failure writer reports ready", || {
        child.ready.exists()
    });
    child.release();
    spin_until("partial failure refreshes actual disk state", || {
        fs::read(&partial_path).ok().as_deref() == Some(b"partial mutation\n")
            && fixture.current_repo(&fixture.repo_a)
    });
    assert!(!fixture.state().borrow().mutation_active_for_tests());
    assert!(fixture.has_entry_state(true, true));
}

fn exercise_spawn_failure(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    fixture.install_hooks(None);
    let child = ControlledChild {
        ready: fixture.repo_a.path().join(".git/spawn-never.ready"),
        release: fixture.repo_a.path().join(".git/spawn-never.release"),
        argv: vec![String::from("/riteed-test/missing-controlled-child")],
    };
    let before_children = fixture.children.borrow().len();
    let before_dispatches = fixture.dispatches.borrow().len();
    let stage_dispatches_before = fixture.stage_index_dispatches();
    let _guard = fixture.controlled(
        &child,
        Duration::from_secs(5),
        Duration::from_millis(30),
        LifecycleObservers::default(),
    );
    fixture.action(GitRowAction::Stage);
    spin_until(
        "spawn failure releases and refreshes mutation owner",
        || {
            fixture.stage_index_dispatches() == stage_dispatches_before + 1
                && fixture.current_repo(&fixture.repo_a)
        },
    );
    assert!(!fixture.state().borrow().mutation_active_for_tests());
    assert!(!child.ready.exists());
    assert_eq!(
        fixture.dispatches.borrow().len() - before_dispatches,
        fixture.children.borrow().len() - before_children + 1,
        "exactly one attempted dispatch has no actual child"
    );
    fixture.action(GitRowAction::Stage);
    spin_until("writer can start after spawn failure", || {
        fixture.has_entry_state(true, false)
    });
}

fn exercise_identity_failure(app: &adw::Application) {
    let Some(fixture) = prepared_fixture(app) else {
        return;
    };
    make_local_identity_invalid(fixture.repo_a.path());
    fixture.install_hooks(None);
    let (entry, button) = {
        let state = fixture.state();
        let state = state.borrow();
        state.settings.set_git_identity("", "");
        (state.commit_entry.clone(), state.commit_button.clone())
    };
    entry.set_text("identity failure");
    assert!(button.is_sensitive());
    button.emit_clicked();
    spin_until("identity failure releases mutation owner", || {
        fixture
            .dispatches
            .borrow()
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == "config"))
            .count()
            >= 2
            && fixture.current_repo(&fixture.repo_a)
    });
    assert!(!fixture.state().borrow().mutation_active_for_tests());
    assert_eq!(
        fixture
            .dispatches
            .borrow()
            .iter()
            .filter(|argv| argv.iter().any(|arg| arg == "commit"))
            .count(),
        0
    );
    fixture.action(GitRowAction::Stage);
    spin_until("writer can start after identity failure", || {
        fixture.has_entry_state(true, false)
    });
}

fn prepared_fixture(app: &adw::Application) -> Option<Fixture> {
    let fixture = Fixture::new(app);
    assert!(
        fixture.is_some(),
        "real GTK and Git fixture must initialize"
    );
    fixture
}

fn modified_repo(kind: FixtureRepoKind) -> Option<ModifiedFixtureRepo> {
    init_modified_fixture_repo_for_tests(kind, FixtureRepoFile::BASELINE, b"base\n", b"changed\n")
        .ok()
}

fn is_stage_index(argv: &[String]) -> bool {
    const SUFFIX: [&str; 4] = ["update-index", "--add", "-z", "--index-info"];
    argv.iter()
        .rev()
        .take(SUFFIX.len())
        .map(String::as_str)
        .eq(SUFFIX.into_iter().rev())
}

fn make_local_identity_invalid(repo: &Path) {
    let config_path = repo.join(".git/config");
    let config = fs::read_to_string(&config_path);
    assert!(config.is_ok(), "fixture Git config must remain readable");
    let config = config.unwrap_or_default();
    let mut retained = Vec::new();
    let mut in_user = false;
    for line in config.lines() {
        if line.starts_with('[') {
            in_user = line.trim() == "[user]";
        }
        if !in_user {
            retained.push(line);
        }
    }
    retained.extend([
        "[user]",
        "\tname = \"invalid\\nname\"",
        "\temail = invalid@example.test",
    ]);
    assert!(fs::write(config_path, retained.join("\n") + "\n").is_ok());
}
