use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk4::{gio, glib, prelude::*};

use crate::git_process::GitRepoContext;
use crate::git_status::GitStatusSnapshot;
use crate::source_control::{SourceControlState, SourceStateRef};

use super::live_scheduler::{LiveScheduler, ScheduledRefresh};
use super::refresh::{RefreshOrigin, refresh_status, refresh_status_with_origin};

const PORTAL_POLL: Duration = Duration::from_secs(4);

pub(crate) struct SourceControlLiveRefresh {
    context: GitRepoContext,
    metadata_targets: Vec<MonitorTarget>,
    metadata_monitors: Vec<gio::FileMonitor>,
    branch_ref_path: Option<PathBuf>,
    branch_ref_target: Option<MonitorTarget>,
    branch_ref_monitor: Option<gio::FileMonitor>,
    poll_source: Option<glib::SourceId>,
    scheduler: LiveScheduler,
}

impl SourceControlLiveRefresh {
    pub(super) fn new(context: GitRepoContext, on_refresh: Rc<dyn Fn(ScheduledRefresh)>) -> Self {
        let use_polling = use_polling(&context);
        let scheduler =
            LiveScheduler::new(context.index_lock_path.clone(), !use_polling, on_refresh);
        let poll_source = if use_polling {
            Some(start_polling(&scheduler))
        } else {
            None
        };
        let mut live = Self {
            context,
            metadata_targets: Vec::new(),
            metadata_monitors: Vec::new(),
            branch_ref_path: None,
            branch_ref_target: None,
            branch_ref_monitor: None,
            poll_source,
            scheduler,
        };
        live.refresh_metadata_monitors();
        live
    }

    pub(super) fn schedule(&self) {
        self.scheduler.schedule();
    }

    pub(super) fn index_lock_exists(&self) -> bool {
        self.scheduler.index_lock_exists()
    }

    pub(super) fn refresh_metadata_monitors(&mut self) {
        if self.poll_source.is_some() {
            return;
        }
        let targets = metadata_targets(&self.context);
        if targets == self.metadata_targets {
            return;
        }
        cancel_monitors(&mut self.metadata_monitors);
        self.metadata_monitors = targets
            .iter()
            .filter_map(|target| monitor_target(target, &self.scheduler))
            .collect();
        self.metadata_targets = targets;
    }

    pub(super) fn rebind_branch_ref(&mut self, path: Option<PathBuf>) {
        if self.poll_source.is_some() {
            self.branch_ref_path = path;
            self.branch_ref_target = None;
            return;
        }
        let target = path.as_ref().and_then(|path| monitor_target_for_path(path));
        if self.branch_ref_path == path && self.branch_ref_target == target {
            return;
        }
        if let Some(monitor) = self.branch_ref_monitor.take() {
            let _cancelled = monitor.cancel();
        }
        self.branch_ref_monitor = target
            .as_ref()
            .and_then(|target| monitor_target(target, &self.scheduler));
        self.branch_ref_path = path;
        self.branch_ref_target = target;
    }

    pub(super) fn cancel(&mut self) {
        self.scheduler.cancel();
        if let Some(source) = self.poll_source.take() {
            source.remove();
        }
        cancel_monitors(&mut self.metadata_monitors);
        if let Some(monitor) = self.branch_ref_monitor.take() {
            let _cancelled = monitor.cancel();
        }
    }
}

pub(super) fn install(state: &SourceStateRef) {
    cancel(state);
    let Some(context) = state
        .borrow()
        .process
        .as_ref()
        .map(|process| process.context().clone())
    else {
        return;
    };
    let weak = Rc::downgrade(state);
    let live = SourceControlLiveRefresh::new(
        context,
        Rc::new(move |kind| {
            let Some(state) = weak.upgrade() else {
                return;
            };
            match kind {
                ScheduledRefresh::Normal => refresh_status(&state),
                ScheduledRefresh::LockWaitExpired => {
                    refresh_status_with_origin(&state, RefreshOrigin::LockWaitExpired);
                }
            }
        }),
    );
    state.borrow_mut().live_refresh = Some(live);
}

pub(super) fn cancel(state: &SourceStateRef) {
    if let Some(mut live) = state.borrow_mut().live_refresh.take() {
        live.cancel();
    }
}

pub(super) fn index_lock_exists(state: &SourceStateRef) -> bool {
    state
        .borrow()
        .live_refresh
        .as_ref()
        .is_some_and(SourceControlLiveRefresh::index_lock_exists)
}

pub(super) fn schedule(state: &SourceStateRef) {
    if let Some(live) = state.borrow().live_refresh.as_ref() {
        live.schedule();
    }
}

pub(super) fn sync_branch_monitor(state: &SourceStateRef, snapshot: &GitStatusSnapshot) {
    if let Some(live) = state.borrow_mut().live_refresh.as_mut() {
        live.refresh_metadata_monitors();
    }
    if snapshot.detached {
        clear_branch_monitor(state);
        return;
    }
    let Some(branch) = snapshot.branch.clone() else {
        clear_branch_monitor(state);
        return;
    };
    if branch == "(detached)" {
        clear_branch_monitor(state);
        return;
    }
    let (process, cancellable) = {
        let state = state.borrow();
        let Some(process) = state.process.clone() else {
            return;
        };
        let Some(cancellable) = state.cancellable.clone() else {
            return;
        };
        (process, cancellable)
    };
    let weak = Rc::downgrade(state);
    let cancellable_for_callback = cancellable.clone();
    process.resolve_branch_ref_path(
        &branch,
        &cancellable,
        Rc::new(move |result| {
            if cancellable_for_callback.is_cancelled() {
                return;
            }
            let Some(state) = weak.upgrade() else {
                return;
            };
            if let Some(live) = state.borrow_mut().live_refresh.as_mut() {
                live.rebind_branch_ref(result.ok());
            }
        }),
    );
}

fn clear_branch_monitor(state: &SourceStateRef) {
    if let Some(live) = state.borrow_mut().live_refresh.as_mut() {
        live.rebind_branch_ref(None);
    }
}

pub(super) fn saved_file_in_repo(state: &SourceControlState, file: &gio::File) -> bool {
    let Some(repo) = state.repo.as_ref() else {
        return false;
    };
    file.path().is_some_and(|path| path.starts_with(repo))
}

impl Drop for SourceControlLiveRefresh {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MonitorKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MonitorTarget {
    path: PathBuf,
    kind: MonitorKind,
}

fn metadata_targets(context: &GitRepoContext) -> Vec<MonitorTarget> {
    [
        context.head_path.as_path(),
        context.index_path.as_path(),
        context.refs_heads_dir.as_path(),
        context.packed_refs_path.as_path(),
    ]
    .into_iter()
    .filter_map(monitor_target_for_path)
    .collect()
}

fn monitor_target_for_path(path: &Path) -> Option<MonitorTarget> {
    if path.exists() {
        return Some(MonitorTarget {
            path: path.to_path_buf(),
            kind: if path.is_dir() {
                MonitorKind::Directory
            } else {
                MonitorKind::File
            },
        });
    }
    let parent = path
        .ancestors()
        .skip(1)
        .find(|ancestor| ancestor.exists())?;
    Some(MonitorTarget {
        path: parent.to_path_buf(),
        kind: MonitorKind::Directory,
    })
}

fn monitor_target(target: &MonitorTarget, scheduler: &LiveScheduler) -> Option<gio::FileMonitor> {
    match target.kind {
        MonitorKind::File => monitor_file(&target.path, scheduler),
        MonitorKind::Directory => monitor_directory(&target.path, scheduler),
    }
}

fn monitor_file(path: &Path, scheduler: &LiveScheduler) -> Option<gio::FileMonitor> {
    let file = gio::File::for_path(path);
    let monitor = file
        .monitor_file(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
        .ok()?;
    let scheduler = scheduler.clone();
    monitor.connect_changed(move |_, _file, _other, event| {
        if refresh_event(event) {
            scheduler.schedule();
        }
    });
    Some(monitor)
}

fn monitor_directory(path: &Path, scheduler: &LiveScheduler) -> Option<gio::FileMonitor> {
    let file = gio::File::for_path(path);
    let monitor = file
        .monitor_directory(gio::FileMonitorFlags::NONE, None::<&gio::Cancellable>)
        .ok()?;
    let scheduler = scheduler.clone();
    monitor.connect_changed(move |_, _file, _other, event| {
        if refresh_event(event) {
            scheduler.schedule();
        }
    });
    Some(monitor)
}

fn cancel_monitors(monitors: &mut Vec<gio::FileMonitor>) {
    for monitor in monitors.drain(..) {
        let _cancelled = monitor.cancel();
    }
}

fn start_polling(scheduler: &LiveScheduler) -> glib::SourceId {
    let scheduler = scheduler.clone();
    glib::timeout_add_local(PORTAL_POLL, move || {
        scheduler.schedule();
        glib::ControlFlow::Continue
    })
}

fn use_polling(context: &GitRepoContext) -> bool {
    [
        &context.work_tree,
        &context.git_dir,
        &context.git_common_dir,
    ]
    .into_iter()
    .any(|path| path_requires_polling(path))
}

fn path_requires_polling(path: &Path) -> bool {
    let file = gio::File::for_path(path);
    !file.is_native() || document_portal_path(path)
}

fn document_portal_path(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.starts_with("/run/flatpak/doc/")
        || (path.starts_with("/run/user/") && path.contains("/doc/"))
}

fn refresh_event(event: gio::FileMonitorEvent) -> bool {
    matches!(
        event,
        gio::FileMonitorEvent::Changed
            | gio::FileMonitorEvent::ChangesDoneHint
            | gio::FileMonitorEvent::Created
            | gio::FileMonitorEvent::Deleted
            | gio::FileMonitorEvent::Moved
            | gio::FileMonitorEvent::Renamed
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use gtk4::gio;

    use super::{
        MonitorKind, document_portal_path, metadata_targets, monitor_target_for_path,
        path_requires_polling, refresh_event, use_polling,
    };
    use crate::git_process::GitRepoContext;

    #[test]
    fn native_git_events_trigger_refresh() {
        assert!(refresh_event(gio::FileMonitorEvent::Changed));
        assert!(refresh_event(gio::FileMonitorEvent::ChangesDoneHint));
        assert!(!refresh_event(gio::FileMonitorEvent::PreUnmount));
    }

    #[test]
    fn document_portal_paths_use_polling() {
        assert!(document_portal_path(std::path::Path::new(
            "/run/flatpak/doc/abc"
        )));
        assert!(document_portal_path(std::path::Path::new(
            "/run/user/1000/doc/abc"
        )));
        assert!(!document_portal_path(std::path::Path::new("/tmp/repo")));
    }

    #[test]
    fn monitor_targets_use_existing_files_and_nearest_existing_parent() {
        let root = temp_dir("riteed-live-monitor-targets");
        let git = root.join(".git");
        assert!(fs::create_dir_all(git.join("refs/heads/feature")).is_ok());
        assert!(fs::write(git.join("HEAD"), b"ref: refs/heads/main").is_ok());

        let head = monitor_target_for_path(&git.join("HEAD"));
        assert!(head.is_some_and(|target| target.kind == MonitorKind::File));
        let branch = monitor_target_for_path(&git.join("refs/heads/feature/topic"));
        assert!(branch.is_some_and(|target| {
            target.kind == MonitorKind::Directory && target.path.ends_with("refs/heads/feature")
        }));
        let packed = monitor_target_for_path(&git.join("packed-refs"));
        assert!(packed.is_some_and(|target| {
            target.kind == MonitorKind::Directory && target.path.ends_with(".git")
        }));

        let _removed = fs::remove_dir_all(root);
    }

    #[test]
    fn metadata_targets_use_resolved_git_paths() {
        let root = temp_dir("riteed-live-metadata-targets");
        let context = context_for(&root);
        assert!(fs::create_dir_all(&context.refs_heads_dir).is_ok());
        assert!(fs::write(&context.head_path, b"ref: refs/heads/main").is_ok());
        assert!(fs::write(&context.index_path, b"").is_ok());

        let targets = metadata_targets(&context);
        assert!(
            targets
                .iter()
                .any(|target| target.path == context.head_path)
        );
        assert!(
            targets
                .iter()
                .any(|target| target.path == context.index_path)
        );
        assert!(
            targets
                .iter()
                .any(|target| target.path == context.refs_heads_dir)
        );

        let _removed = fs::remove_dir_all(root);
    }

    #[test]
    fn polling_checks_worktree_and_metadata_roots() {
        let context = GitRepoContext {
            work_tree: PathBuf::from("/tmp/repo"),
            git_dir: PathBuf::from("/run/flatpak/doc/git"),
            git_common_dir: PathBuf::from("/tmp/repo/.git"),
            head_path: PathBuf::from("/tmp/repo/.git/HEAD"),
            index_path: PathBuf::from("/tmp/repo/.git/index"),
            index_lock_path: PathBuf::from("/tmp/repo/.git/index.lock"),
            refs_heads_dir: PathBuf::from("/tmp/repo/.git/refs/heads"),
            packed_refs_path: PathBuf::from("/tmp/repo/.git/packed-refs"),
        };
        assert!(use_polling(&context));
        assert!(path_requires_polling(Path::new("/run/user/1000/doc/repo")));
    }

    fn context_for(root: &Path) -> GitRepoContext {
        let git = root.join(".git");
        GitRepoContext {
            work_tree: root.to_path_buf(),
            git_dir: git.clone(),
            git_common_dir: git.clone(),
            head_path: git.join("HEAD"),
            index_path: git.join("index"),
            index_lock_path: git.join("index.lock"),
            refs_heads_dir: git.join("refs/heads"),
            packed_refs_path: git.join("packed-refs"),
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(name);
        let _removed = fs::remove_dir_all(&path);
        assert!(fs::create_dir_all(&path).is_ok());
        path
    }
}
