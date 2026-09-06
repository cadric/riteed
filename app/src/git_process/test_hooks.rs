use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::gio;

use super::lifecycle::{GitDeadlineConfig, GitSpec};
use super::{GitCallback, GitProcessError, TestChild};
use crate::git_status::GitStatusSnapshot;

pub(crate) type Resume = Box<dyn FnOnce()>;
pub(crate) type Hold<T> = Rc<dyn Fn(Result<T, GitProcessError>, Resume)>;
pub(crate) type Dispatch = Rc<dyn Fn(Vec<String>, bool, gio::Cancellable)>;
pub(crate) type GraceAdvance = Rc<dyn Fn()>;

pub(crate) struct Hooks {
    pub(crate) repo: PathBuf,
    pub(crate) status: Option<Hold<GitStatusSnapshot>>,
    pub(crate) blob: Option<Hold<Vec<u8>>>,
    pub(crate) dispatch: Option<Dispatch>,
    pub(crate) started: Option<Rc<dyn Fn(TestChild)>>,
}

#[derive(Clone)]
pub(crate) struct ControlledMutation {
    pub(crate) repo: PathBuf,
    pub(crate) argv: Vec<String>,
    pub(crate) operation: Duration,
    pub(crate) grace: Duration,
    pub(crate) deadline_fired: Option<Rc<dyn Fn()>>,
    pub(crate) communication_settled: Option<Rc<dyn Fn()>>,
    pub(crate) wait_completed: Option<Rc<dyn Fn()>>,
    pub(crate) grace_checkpoint: Option<Rc<dyn Fn(GraceAdvance)>>,
}

struct ControlledEntry {
    identity: Rc<()>,
    mutation: ControlledMutation,
}

pub(crate) struct ControlledMutationGuard(Rc<()>);

thread_local! {
    static HOOKS: RefCell<Option<Hooks>> = const { RefCell::new(None) };
    static CONTROLLED_MUTATION: RefCell<Option<ControlledEntry>> = const { RefCell::new(None) };
}

pub(crate) fn install(hooks: Option<Hooks>) {
    HOOKS.with(|slot| *slot.borrow_mut() = hooks);
}

pub(crate) fn install_controlled_mutation(mutation: ControlledMutation) -> ControlledMutationGuard {
    let identity = Rc::new(());
    CONTROLLED_MUTATION.with(|slot| {
        *slot.borrow_mut() = Some(ControlledEntry {
            identity: Rc::clone(&identity),
            mutation,
        });
    });
    ControlledMutationGuard(identity)
}

impl Drop for ControlledMutationGuard {
    fn drop(&mut self) {
        CONTROLLED_MUTATION.with(|slot| {
            let matching = slot
                .borrow()
                .as_ref()
                .is_some_and(|entry| Rc::ptr_eq(&entry.identity, &self.0));
            if matching {
                *slot.borrow_mut() = None;
            }
        });
    }
}

pub(super) fn prepare(
    spec: GitSpec,
    deadlines: GitDeadlineConfig,
    cancel: &gio::Cancellable,
) -> (GitSpec, GitDeadlineConfig) {
    dispatched(&spec.argv, &spec.env, !spec.kill_on_cancel, cancel);
    let controlled = CONTROLLED_MUTATION.with(|slot| {
        let matches = slot.borrow().as_ref().is_some_and(|entry| {
            Some(entry.mutation.repo.as_path()) == environment_repo(&spec.env)
                && is_stage_index_spec(&spec.argv)
                && !spec.kill_on_cancel
        });
        if matches {
            slot.borrow_mut().take()
        } else {
            None
        }
    });
    let Some(controlled) = controlled else {
        return (spec, deadlines);
    };
    let mutation = controlled.mutation;
    let spec = GitSpec {
        argv: mutation.argv,
        ..spec
    };
    let deadlines = GitDeadlineConfig {
        operation: mutation.operation,
        grace: mutation.grace,
        deadline_fired: mutation.deadline_fired,
        communication_settled: mutation.communication_settled,
        wait_completed: mutation.wait_completed,
        grace_checkpoint: mutation.grace_checkpoint,
        ..deadlines
    };
    (spec, deadlines)
}

fn environment_repo(env: &[(String, String)]) -> Option<&Path> {
    env.iter()
        .find(|(key, _)| key == "GIT_WORK_TREE")
        .map(|(_, value)| Path::new(value))
}

fn is_stage_index_spec(argv: &[String]) -> bool {
    const SUFFIX: [&str; 4] = ["update-index", "--add", "-z", "--index-info"];
    argv.iter()
        .rev()
        .take(SUFFIX.len())
        .map(String::as_str)
        .eq(SUFFIX.into_iter().rev())
}

fn dispatched(
    argv: &[String],
    env: &[(String, String)],
    mutation: bool,
    cancel: &gio::Cancellable,
) {
    let observer = HOOKS.with(|slot| {
        let hooks = slot.borrow();
        hooks
            .as_ref()
            .filter(|hooks| Some(hooks.repo.as_path()) == environment_repo(env))
            .and_then(|hooks| hooks.dispatch.clone())
    });
    if let Some(observer) = observer {
        observer(argv.to_vec(), mutation, cancel.clone());
    }
}
pub(crate) fn started(env: &[(String, String)], child: TestChild) {
    let observer = HOOKS.with(|slot| {
        let hooks = slot.borrow();
        hooks
            .as_ref()
            .filter(|hooks| Some(hooks.repo.as_path()) == environment_repo(env))
            .and_then(|hooks| hooks.started.clone())
    });
    if let Some(observer) = observer {
        observer(child);
    }
}
pub(crate) fn status(
    repo: &Path,
    result: Result<GitStatusSnapshot, GitProcessError>,
    callback: GitCallback<GitStatusSnapshot>,
) {
    let hold = HOOKS.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .filter(|hooks| hooks.repo == repo)
            .and_then(|hooks| hooks.status.take())
    });
    deliver(hold, result, callback);
}
pub(crate) fn blob(
    repo: &Path,
    result: Result<Vec<u8>, GitProcessError>,
    callback: GitCallback<Vec<u8>>,
) {
    let hold = HOOKS.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .filter(|hooks| hooks.repo == repo)
            .and_then(|hooks| hooks.blob.take())
    });
    deliver(hold, result, callback);
}
fn deliver<T: Clone + 'static>(
    hold: Option<Hold<T>>,
    result: Result<T, GitProcessError>,
    callback: GitCallback<T>,
) {
    if let Some(hold) = hold {
        let observed = result.clone();
        hold(observed, Box::new(move || callback(result)));
    } else {
        callback(result);
    }
}
