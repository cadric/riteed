use crate::git_process::TestChild;
use crate::git_process::test_hooks::{self, Hooks, Resume};
use crate::git_process::test_support::{
    FixtureRepoFile, FixtureRepoKind, ModifiedFixtureRepo, init_modified_fixture_repo_for_tests,
};
use crate::gtk_tests::build_window;
use crate::source_control::{SourceControlState, actions::GitRowAction};
use crate::window::Window;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::fs;
use std::rc::Rc;
use std::time::Duration;

type Held = Rc<RefCell<Option<Resume>>>;
struct Fixture {
    repo: ModifiedFixtureRepo,
    window: Rc<Window>,
    children: Rc<RefCell<Vec<TestChild>>>,
    cancellations: Rc<RefCell<Vec<gio::Cancellable>>>,
    writes: Rc<RefCell<Vec<Vec<String>>>>,
    dispatches: Rc<RefCell<Vec<Vec<String>>>>,
    signal_checks: Rc<Cell<usize>>,
    borrowed_signals: Rc<Cell<usize>>,
}
impl Fixture {
    fn new(app: &adw::Application) -> Self {
        let repo = init_modified_fixture_repo_for_tests(
            FixtureRepoKind::V9_SOURCE_CONTROL_TRACKED,
            FixtureRepoFile::BASELINE,
            b"base\n",
            b"changed\n",
        )
        .unwrap_or_else(|error| unreachable!("real repo fixture: {error:?}"));
        let window = build_window(app).unwrap_or_else(|| unreachable!("GTK fixture window"));
        let fixture = Self {
            repo,
            window,
            children: Rc::new(RefCell::new(Vec::new())),
            cancellations: Rc::new(RefCell::new(Vec::new())),
            writes: Rc::new(RefCell::new(Vec::new())),
            dispatches: Rc::new(RefCell::new(Vec::new())),
            signal_checks: Rc::new(Cell::new(0)),
            borrowed_signals: Rc::new(Cell::new(0)),
        };
        fixture.install(None, None);
        fixture
            .window
            .handle_application_open(vec![gio::File::for_path(fixture.repo.path())]);
        pump("real initial snapshot", || {
            let state = fixture.state();
            let state = state.borrow();
            state.repo.as_deref() == Some(fixture.repo.path())
                && !state.status_stale
                && state.snapshot.entries.len() == 1
        });
        // The test explicitly requests each refresh; unrelated monitor debounce
        // must not replace a deliberately held pipeline's identity.
        let live = fixture.state().borrow_mut().live_refresh.take();
        drop(live);
        fixture.observe_signal_borrows();
        fixture
    }
    fn state(&self) -> Rc<RefCell<SourceControlState>> {
        self.window
            .source_control_state_weak_for_tests()
            .upgrade()
            .unwrap_or_else(|| unreachable!("controller must be alive"))
    }
    fn uri(&self) -> String {
        gio::File::for_path(self.repo.file_path(FixtureRepoFile::BASELINE))
            .uri()
            .to_string()
    }
    fn install(
        &self,
        status: Option<test_hooks::Hold<crate::git_status::GitStatusSnapshot>>,
        blob: Option<test_hooks::Hold<Vec<u8>>>,
    ) {
        let writes = self.writes.clone();
        let dispatches = self.dispatches.clone();
        let cancellations = self.cancellations.clone();
        let children = self.children.clone();
        test_hooks::install(Some(Hooks {
            repo: self.repo.path().to_path_buf(),
            status,
            blob,
            dispatch: Some(Rc::new(move |argv, mutation, cancel| {
                cancellations.borrow_mut().push(cancel);
                dispatches.borrow_mut().push(argv.clone());
                if mutation
                    || (argv.iter().any(|arg| arg == "hash-object")
                        && argv.iter().any(|arg| arg == "-w"))
                {
                    writes.borrow_mut().push(argv);
                }
            })),
            started: Some(Rc::new(move |child| children.borrow_mut().push(child))),
        }));
    }
    fn observe_signal_borrows(&self) {
        let state = self.state();
        let objects: Vec<glib::Object> = {
            let state = state.borrow();
            vec![
                state.status_label.clone().upcast(),
                state.title.clone().upcast(),
                state.commit_button.clone().upcast(),
                state.commit_revealer.clone().upcast(),
                state.review_staged_action.clone().upcast(),
                state.review_unstaged_action.clone().upcast(),
            ]
        };
        for object in objects {
            let weak = Rc::downgrade(&state);
            let checks = self.signal_checks.clone();
            let borrowed = self.borrowed_signals.clone();
            object.connect_notify_local(None, move |_, _| {
                if let Some(state) = weak.upgrade() {
                    checks.set(checks.get() + 1);
                    if state.try_borrow_mut().is_err() {
                        borrowed.set(borrowed.get() + 1);
                    }
                }
            });
        }
    }
    fn assert_signal_borrows_released(&self) {
        assert!(
            self.signal_checks.get() > 0,
            "real GTK property notifications observed"
        );
        assert_eq!(
            self.borrowed_signals.get(),
            0,
            "SourceControl borrow must end before GTK signals"
        );
    }
    fn action(&self, action: GitRowAction) {
        self.window
            .source_control_run_action_for_tests(&self.uri(), action);
    }
    fn refresh(&self) {
        assert!(
            gtk4::prelude::WidgetExt::activate_action(
                self.window.widget(),
                "win.git-refresh",
                None
            )
            .is_ok(),
            "public refresh action exists"
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        test_hooks::install(None);
        for cancel in self.cancellations.borrow_mut().drain(..) {
            cancel.cancel();
        }
        for child in self.children.borrow_mut().drain(..) {
            let reaped = child.force_reap();
            if !std::thread::panicking() {
                assert!(reaped, "fixture child must reap");
            }
        }
        let _removed = fs::remove_file(self.repo.path().join(".git/index.lock"));
        self.window.widget().destroy();
    }
}
fn pump(label: &str, done: impl Fn() -> bool) {
    let expired = Rc::new(std::cell::Cell::new(false));
    let expired_for_timer = expired.clone();
    let timeout =
        glib::timeout_add_local_once(Duration::from_secs(15), move || expired_for_timer.set(true));
    let tick = glib::timeout_add_local(Duration::from_millis(5), || glib::ControlFlow::Continue);
    while !done() && !expired.get() {
        let _dispatched = glib::MainContext::default().iteration(true);
    }
    tick.remove();
    if !expired.get() {
        timeout.remove();
    }
    assert!(done(), "{label}");
}

pub(crate) fn exercise_status_diff_ownership(app: &adw::Application) {
    let fixture = Fixture::new(app);
    let status: Held = Rc::new(RefCell::new(None));
    let status_for_hook = status.clone();
    let blob: Held = Rc::new(RefCell::new(None));
    let blob_for_hook = blob.clone();
    assert_eq!(
        fixture.state().borrow().snapshot.entries[0].path.raw(),
        b"baseline.txt"
    );
    assert!(fs::write(fixture.repo.file_path(FixtureRepoFile::BASELINE), b"base\n").is_ok());
    fixture.install(
        Some(Rc::new(move |result, resume| {
            let actual = result
                .unwrap_or_else(|error| unreachable!("real status child must succeed: {error:?}"));
            assert!(
                actual.entries.is_empty(),
                "real disk restoration must produce clean status"
            );
            *status_for_hook.borrow_mut() = Some(resume);
        })),
        Some(Rc::new(move |result, resume| {
            assert_eq!(
                result.unwrap_or_else(|error| unreachable!("real diff blob read: {error:?}")),
                b"base\n"
            );
            *blob_for_hook.borrow_mut() = Some(resume);
        })),
    );
    fixture.refresh();
    pump("actual status returned and held", || {
        status.borrow().is_some()
    });
    fixture.action(GitRowAction::Diff);
    pump("actual diff blob returned and held", || {
        blob.borrow().is_some()
    });
    let resume_diff = blob
        .borrow_mut()
        .take()
        .unwrap_or_else(|| unreachable!("held diff"));
    resume_diff();
    pump("public Diff opened expected document", || {
        fixture.window.selected_compare_active_for_tests()
            && fixture.window.selected_saved_uri_for_tests() == fixture.uri()
    });
    let resume_status = status
        .borrow_mut()
        .take()
        .unwrap_or_else(|| unreachable!("held status"));
    resume_status();
    let state = fixture.state();
    let state = state.borrow();
    assert_eq!(
        state.repo.as_deref(),
        Some(fixture.repo.path()),
        "correct repository after both continuations"
    );
    assert!(
        state.snapshot.entries.is_empty(),
        "released real clean snapshot must apply after independent Diff"
    );
    assert!(!state.status_stale, "released snapshot is current");
    drop(state);
    fixture.assert_signal_borrows_released();
}

pub(crate) fn exercise_index_lock_entry_guards(app: &adw::Application) {
    let fixture = Fixture::new(app);
    fixture.install(None, None);
    fixture.action(GitRowAction::Stage);
    pump("setup public Stage completes", || {
        fixture
            .state()
            .borrow()
            .snapshot
            .entries
            .iter()
            .any(|entry| entry.staged && !entry.unstaged)
    });
    assert!(
        fs::write(
            fixture.repo.file_path(FixtureRepoFile::BASELINE),
            b"changed again\n"
        )
        .is_ok()
    );
    fixture.refresh();
    pump("snapshot has staged and unstaged changes", || {
        fixture
            .state()
            .borrow()
            .snapshot
            .entries
            .iter()
            .any(|entry| entry.staged && entry.unstaged)
    });
    let state = fixture.state();
    let entry = state.borrow().commit_entry.clone();
    entry.set_text("locked fixture commit");
    let button = state.borrow().commit_button.clone();
    assert!(
        button.is_sensitive(),
        "commit entry point is enabled before fixture lock"
    );
    let lock = fixture.repo.path().join(".git/index.lock");
    assert!(fs::write(&lock, b"fixture lock\n").is_ok());
    let before = fixture.writes.borrow().len();
    fixture.action(GitRowAction::Stage);
    let after_stage = fixture.writes.borrow().len();
    let before_commit_dispatch = fixture.dispatches.borrow().len();
    button.emit_clicked();
    let after_commit_dispatch = fixture.dispatches.borrow().len();

    assert_eq!(
        after_stage, before,
        "locked public Stage must not start a writer"
    );
    assert_eq!(
        after_commit_dispatch, before_commit_dispatch,
        "locked public Commit must be denied before identity or writer dispatch"
    );
    assert_eq!(
        fixture.writes.borrow().len(),
        before,
        "locked actions must not start writers"
    );
    assert_eq!(
        fixture.window.source_control_status_for_tests(),
        gettextrs::gettext("Waiting for another Git operation to finish") + "…"
    );
    assert!(fs::remove_file(&lock).is_ok());
    fixture.refresh();
    pump("current snapshot after lock removal", || {
        let state = fixture.state();
        let state = state.borrow();
        !state.status_stale && state.repo.as_deref() == Some(fixture.repo.path())
    });
    assert_eq!(
        fixture.writes.borrow().len(),
        before,
        "denied mutations are never replayed"
    );
    fixture.assert_signal_borrows_released();
}

pub(crate) fn exercise_reentrant_root_change(app: &adw::Application) {
    let fixture = Fixture::new(app);
    let other = init_modified_fixture_repo_for_tests(
        FixtureRepoKind::V9_SOURCE_CONTROL_UNTRACKED,
        FixtureRepoFile::BASELINE,
        b"other base\n",
        b"other change\n",
    )
    .unwrap_or_else(|error| unreachable!("second real repository: {error:?}"));
    let attempted = Rc::new(std::cell::Cell::new(false));
    let borrowed = Rc::new(std::cell::Cell::new(false));
    let attempted_for_signal = attempted.clone();
    let borrowed_for_signal = borrowed.clone();
    let state_weak = fixture.window.source_control_state_weak_for_tests();
    let window_weak = Rc::downgrade(&fixture.window);
    let other_path = other.path().to_path_buf();
    let label = fixture.state().borrow().status_label.clone();
    let handler = label.connect_notify_local(Some("label"), move |_, _| {
        if attempted_for_signal.replace(true) {
            return;
        }
        let Some(state) = state_weak.upgrade() else {
            return;
        };
        let can_reenter = state.try_borrow_mut().is_ok();
        if !can_reenter {
            borrowed_for_signal.set(true);
            return;
        }
        if let Some(window) = window_weak.upgrade() {
            window.set_source_control_project_root_for_tests(gio::File::for_path(&other_path));
        }
    });
    fixture
        .window
        .set_source_control_project_root_for_tests(gio::File::for_path(fixture.repo.path()));
    label.disconnect(handler);
    assert!(attempted.get(), "real root-reset notification must run");
    assert!(
        !borrowed.get(),
        "root-reset signal must permit reentry without a controller borrow"
    );
    pump("newer nested root request wins", || {
        let state = fixture.state();
        let state = state.borrow();
        state.repo.as_deref() == Some(other.path())
            && !state.status_stale
            && state.snapshot.entries.len() == 1
    });
}
