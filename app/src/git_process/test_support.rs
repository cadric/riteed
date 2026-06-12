use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk4::{gio, glib};

use super::support::base_args;
use super::{GitCallback, GitProcessError, GitSpec, run_git};

#[derive(Clone, Copy)]
pub(crate) enum FixtureRepoKind {
    V7SidebarMinimap,
    V9SourceControlMinimap,
    V9SourceControlTracked,
    V9SourceControlUntracked,
    V11GitCompare,
}

impl FixtureRepoKind {
    pub(crate) const V7_SIDEBAR_MINIMAP: Self = Self::V7SidebarMinimap;
    pub(crate) const V9_SOURCE_CONTROL_MINIMAP: Self = Self::V9SourceControlMinimap;
    pub(crate) const V9_SOURCE_CONTROL_TRACKED: Self = Self::V9SourceControlTracked;
    pub(crate) const V9_SOURCE_CONTROL_UNTRACKED: Self = Self::V9SourceControlUntracked;
    pub(crate) const V11_GIT_COMPARE: Self = Self::V11GitCompare;

    fn label(self) -> &'static str {
        match self {
            Self::V7SidebarMinimap => "v7-sidebar-minimap",
            Self::V9SourceControlMinimap => "v9-source-control-minimap",
            Self::V9SourceControlTracked => "v9-source-control-tracked",
            Self::V9SourceControlUntracked => "v9-source-control-untracked",
            Self::V11GitCompare => "v11-git-compare",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FixtureRepoFile {
    Baseline,
    Marker,
    Minimap,
    SidebarMinimap,
    Tracked,
    Untracked,
}

impl FixtureRepoFile {
    pub(crate) const BASELINE: Self = Self::Baseline;
    pub(crate) const MARKER: Self = Self::Marker;
    pub(crate) const MINIMAP: Self = Self::Minimap;
    pub(crate) const SIDEBAR_MINIMAP: Self = Self::SidebarMinimap;
    pub(crate) const TRACKED: Self = Self::Tracked;
    pub(crate) const UNTRACKED: Self = Self::Untracked;

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline.txt",
            Self::Marker => "marker.txt",
            Self::Minimap => "minimap.txt",
            Self::SidebarMinimap => "sidebar-minimap.rs",
            Self::Tracked => "tracked.txt",
            Self::Untracked => "untracked.txt",
        }
    }
}

pub(crate) struct ModifiedFixtureRepo {
    directory: PathBuf,
}

impl ModifiedFixtureRepo {
    pub(crate) fn path(&self) -> &Path {
        &self.directory
    }

    pub(crate) fn file_path(&self, file: FixtureRepoFile) -> PathBuf {
        self.directory.join(file.name())
    }
}

impl Drop for ModifiedFixtureRepo {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.directory);
    }
}

pub(crate) fn init_modified_fixture_repo_for_tests(
    kind: FixtureRepoKind,
    file: FixtureRepoFile,
    baseline: &[u8],
    working: &[u8],
) -> Result<ModifiedFixtureRepo, GitProcessError> {
    let directory = fixture_repo_directory(kind);
    fs::create_dir_all(&directory)
        .map_err(|error| GitProcessError::CommandFailed(error.to_string()))?;
    run_git_fixture_command(&directory, &["init"])?;
    run_git_fixture_command(&directory, &["config", "user.name", "Riteed Test"])?;
    run_git_fixture_command(
        &directory,
        &["config", "user.email", "riteed-test@example.invalid"],
    )?;
    fs::write(directory.join(file.name()), baseline)
        .map_err(|error| GitProcessError::CommandFailed(error.to_string()))?;
    run_git_fixture_command(&directory, &["add", file.name()])?;
    run_git_fixture_command(
        &directory,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "commit",
            "--no-gpg-sign",
            "-m",
            "baseline",
        ],
    )?;
    fs::write(directory.join(file.name()), working)
        .map_err(|error| GitProcessError::CommandFailed(error.to_string()))?;
    Ok(ModifiedFixtureRepo { directory })
}

fn fixture_repo_directory(kind: FixtureRepoKind) -> PathBuf {
    let base = PathBuf::from("/tmp");
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    base.join(format!(
        "riteed-git-fixture-{}-{}-{nanos}",
        std::process::id(),
        kind.label()
    ))
}

fn wait_git<T: 'static>(
    start: impl FnOnce(&gio::Cancellable, GitCallback<T>),
) -> Result<T, GitProcessError> {
    let slot: Rc<RefCell<Option<Result<T, GitProcessError>>>> = Rc::new(RefCell::new(None));
    let slot_for_callback = Rc::clone(&slot);
    let cancellable = gio::Cancellable::new();
    start(
        &cancellable,
        Rc::new(move |result| {
            *slot_for_callback.borrow_mut() = Some(result);
        }),
    );
    for _ in 0..600 {
        while glib::MainContext::default().iteration(false) {}
        if slot.borrow().is_some() {
            break;
        }
        let fired = Rc::new(Cell::new(false));
        let fired_for_timeout = Rc::clone(&fired);
        let source = glib::timeout_add_local_once(Duration::from_millis(10), move || {
            fired_for_timeout.set(true);
        });
        while !fired.get() && slot.borrow().is_none() {
            let _dispatched = glib::MainContext::default().iteration(true);
        }
        if !fired.get() {
            source.remove();
        }
    }
    let result = slot.borrow_mut().take();
    let Some(result) = result else {
        return Err(GitProcessError::Cancelled);
    };
    result
}

fn run_git_fixture_command(directory: &Path, command_args: &[&str]) -> Result<(), GitProcessError> {
    let Some(directory) = directory.to_str() else {
        return Err(GitProcessError::InvalidPath);
    };
    let mut argv = base_args();
    argv.extend(["-C", directory].map(String::from));
    argv.extend(command_args.iter().map(|arg| String::from(*arg)));
    wait_git(|cancellable, callback| {
        run_git(
            GitSpec {
                argv,
                env: Vec::new(),
                stdin: None,
                stdout_cap: 256 * 1024,
                allow_failure: false,
            },
            cancellable,
            Rc::new(move |result| callback(result.map(|_output| ()))),
        );
    })
}
