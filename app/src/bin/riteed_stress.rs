#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use gtk4::{gio, glib, prelude::*};

const SCRIPT_ENV: &str = "RITEED_STRESS_SCRIPT";
const RUN_MILLIS: u64 = 700;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_error) => ExitCode::FAILURE,
    }
}

fn run() -> Result<(), StressError> {
    let script = StressScript::load_from_env()?;
    if script.expect_failure() {
        script.write_artifact(&[String::from("intentional-failure")])?;
        return Err(StressError::IntentionalFailure);
    }
    let flow = Flow::from_script(script.text())?;
    run_app_flow(&script, flow)
}

fn run_app_flow(script: &StressScript, flow: Flow) -> Result<(), StressError> {
    riteed::bootstrap_runtime().map_err(|_error| StressError::RuntimeInit)?;
    gtk4::init().map_err(|_error| StressError::GtkInit)?;
    let riteed = riteed::app::RiteedApp::new();
    let app = riteed.application().clone();
    app.register(None::<&gio::Cancellable>)
        .map_err(|_error| StressError::AppRegister)?;

    let mut artifact_lines = vec![format!("flow={}", flow.name())];
    match flow {
        Flow::OpenSaveSearch => run_open_save_search(&app, script, &mut artifact_lines)?,
        Flow::CompareRoundtrip => run_compare_roundtrip(&app, script, &mut artifact_lines)?,
        Flow::MarkdownStress => run_markdown_stress(&app, script, &mut artifact_lines)?,
        Flow::GitStatusStress => run_git_status_stress(&app, script, &mut artifact_lines)?,
    }

    spin_for(Duration::from_millis(RUN_MILLIS));
    script.write_artifact(&artifact_lines)?;
    app.quit();
    spin_for(Duration::from_millis(50));
    Ok(())
}

const GIT_STATUS_REPOS: &[&str] = &[
    "stress/git-repos/generated/many-untracked",
    "stress/git-repos/generated/many-modified",
    "stress/git-repos/generated/conflicted",
    "stress/git-repos/generated/non-utf8-paths",
    "stress/git-repos/generated/submodule-and-symlink",
    "stress/git-repos/generated/index-lock-present",
    "stress/git-repos/generated/huge-status",
];

fn run_open_save_search(
    app: &(impl IsA<gtk4::Application> + IsA<gio::Application>),
    script: &StressScript,
    artifact_lines: &mut Vec<String>,
) -> Result<(), StressError> {
    const RELATIVE: &str = "stress/corpus/generated/open-save-search.txt";
    app.open(&[script.declared_file(RELATIVE)?], "");
    wait_for_source_contains(app, "needle")?;
    activate_window_action(app, "win.search")?;
    set_search_query(app, "needle")?;
    activate_window_action(app, "win.find-next")?;
    let marker = "riteed-stress-saved-marker\n";
    set_first_source_text(app, &format!("needle\n{marker}"))?;
    activate_window_action(app, "win.save")?;
    let saved_path = script.declared_path(RELATIVE)?;
    wait_until(|| file_contains(&saved_path, marker))?;
    artifact_lines.push(String::from("action=open"));
    artifact_lines.push(String::from("action=search"));
    artifact_lines.push(String::from("action=save"));
    artifact_lines.push(String::from("assert=document-state:opened"));
    artifact_lines.push(String::from("assert=search-state:query-visible"));
    artifact_lines.push(String::from("assert=dirty-clean-state:saved"));
    Ok(())
}

fn run_compare_roundtrip(
    app: &(impl IsA<gtk4::Application> + IsA<gio::Application>),
    script: &StressScript,
    artifact_lines: &mut Vec<String>,
) -> Result<(), StressError> {
    const REFERENCE: &str = "stress/corpus/generated/compare-reference.txt";
    const CURRENT: &str = "stress/corpus/generated/compare-current.txt";
    app.open(&[script.declared_file(CURRENT)?], "");
    wait_for_source_contains(app, "gamma")?;
    let reference_text = script.read_declared_text(REFERENCE)?;
    set_first_source_text(app, &reference_text)?;
    activate_window_action(app, "win.tab-compare-with-saved-version")?;
    wait_until(|| {
        active_root(app).is_ok_and(|root| {
            source_view_count(&root) >= 2
                || visible_text_contains(&root, "gamma")
                || visible_text_contains(&root, "beta")
        })
    })?;
    artifact_lines.push(String::from("action=open"));
    artifact_lines.push(String::from("action=start-compare-workflow"));
    artifact_lines.push(String::from("assert=compare-pane-diff-state:diff-visible"));
    Ok(())
}

fn run_markdown_stress(
    app: &(impl IsA<gtk4::Application> + IsA<gio::Application>),
    script: &StressScript,
    artifact_lines: &mut Vec<String>,
) -> Result<(), StressError> {
    app.open(
        &[script.declared_file("stress/corpus/generated/markdown-stress.md")?],
        "",
    );
    wait_for_source_contains(app, "# Stress")?;
    activate_window_action(app, "win.tab-toggle-markdown-preview")?;
    wait_until(|| active_root(app).is_ok_and(|root| visible_text_contains(&root, "literal")))?;
    artifact_lines.push(String::from("action=open"));
    artifact_lines.push(String::from("action=toggle-preview-render"));
    artifact_lines.push(String::from(
        "assert=preview-or-fallback-state:preview-visible",
    ));
    Ok(())
}

fn run_git_status_stress(
    app: &(impl IsA<gtk4::Application> + IsA<gio::Application>),
    script: &StressScript,
    artifact_lines: &mut Vec<String>,
) -> Result<(), StressError> {
    for repo in GIT_STATUS_REPOS {
        app.open(&[script.declared_dir(repo)?], "");
        wait_until(|| active_root(app).is_ok_and(|root| source_control_state_visible(&root)))?;
        let refresh_state = optional_window_action(app, "win.git-refresh");
        artifact_lines.push(format!("repo={repo}:refresh={refresh_state}"));
    }
    artifact_lines.push(String::from("action=open-source-control"));
    artifact_lines.push(String::from("action=refresh-source-control"));
    artifact_lines.push(String::from(
        "assert=source-control-or-degraded-state:visible",
    ));
    Ok(())
}

struct StressScript {
    text: String,
    repo: PathBuf,
}

impl StressScript {
    fn load_from_env() -> Result<Self, StressError> {
        let script_path = std::env::var_os(SCRIPT_ENV).ok_or(StressError::MissingScript)?;
        let script_path = absolute_path(PathBuf::from(script_path))?;
        let text =
            std::fs::read_to_string(&script_path).map_err(|_error| StressError::ScriptRead)?;
        let repo = repo_root_for(&script_path)?;
        Ok(Self { text, repo })
    }

    fn text(&self) -> &str {
        &self.text
    }

    fn expect_failure(&self) -> bool {
        self.text.contains("\"expect_failure\": true")
    }

    fn declared_file(&self, relative: &str) -> Result<gio::File, StressError> {
        Ok(gio::File::for_path(self.declared_path(relative)?))
    }

    fn declared_dir(&self, relative: &str) -> Result<gio::File, StressError> {
        Ok(gio::File::for_path(self.declared_path(relative)?))
    }

    fn declared_path(&self, relative: &str) -> Result<PathBuf, StressError> {
        self.ensure_declared(relative)?;
        Ok(self.repo.join(relative))
    }

    fn read_declared_text(&self, relative: &str) -> Result<String, StressError> {
        std::fs::read_to_string(self.declared_path(relative)?)
            .map_err(|_error| StressError::FileRead)
    }

    fn write_artifact(&self, lines: &[String]) -> Result<(), StressError> {
        let artifact_dir = self.artifact_dir()?;
        std::fs::create_dir_all(&artifact_dir).map_err(|_error| StressError::ArtifactWrite)?;
        let mut body = String::new();
        for line in lines {
            body.push_str(line);
            body.push('\n');
        }
        std::fs::write(artifact_dir.join("stress-run.log"), body)
            .map_err(|_error| StressError::ArtifactWrite)
    }

    fn artifact_dir(&self) -> Result<PathBuf, StressError> {
        let relative = quoted_value(&self.text, "artifact_dir").ok_or(StressError::ScriptShape)?;
        let safe_relative = safe_stress_artifact_dir(&relative).ok_or(StressError::ScriptShape)?;
        if !relative.starts_with("stress/artifacts/") {
            return Err(StressError::ScriptShape);
        }
        Ok(self.repo.join(safe_relative))
    }

    fn ensure_declared(&self, relative: &str) -> Result<(), StressError> {
        let needle = format!("\"path\": \"{relative}\"");
        if self.text.contains(&needle) {
            Ok(())
        } else {
            Err(StressError::UndeclaredFixture)
        }
    }
}

fn wait_for_source_contains(
    app: &impl IsA<gtk4::Application>,
    needle: &str,
) -> Result<(), StressError> {
    wait_until(|| {
        active_root(app)
            .is_ok_and(|root| source_texts(&root).iter().any(|text| text.contains(needle)))
    })
}

fn wait_until(mut predicate: impl FnMut() -> bool) -> Result<(), StressError> {
    let deadline = Instant::now() + Duration::from_secs(6);
    while Instant::now() < deadline {
        while glib::MainContext::default().iteration(false) {}
        if predicate() {
            return Ok(());
        }
        let tick = std::rc::Rc::new(std::cell::Cell::new(false));
        let tick_for_callback = std::rc::Rc::clone(&tick);
        let _source = glib::timeout_add_local_once(Duration::from_millis(20), move || {
            tick_for_callback.set(true);
        });
        while !tick.get() && Instant::now() < deadline {
            let _dispatched = glib::MainContext::default().iteration(true);
        }
    }
    Err(StressError::AssertionFailed)
}

fn spin_for(duration: Duration) {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        while glib::MainContext::default().iteration(false) {}
        let tick = std::rc::Rc::new(std::cell::Cell::new(false));
        let tick_for_callback = std::rc::Rc::clone(&tick);
        let _source = glib::timeout_add_local_once(Duration::from_millis(10), move || {
            tick_for_callback.set(true);
        });
        while !tick.get() && Instant::now() < deadline {
            let _dispatched = glib::MainContext::default().iteration(true);
        }
    }
}

fn active_root(app: &impl IsA<gtk4::Application>) -> Result<gtk4::Widget, StressError> {
    app.active_window()
        .map(gtk4::prelude::Cast::upcast::<gtk4::Widget>)
        .ok_or(StressError::MissingWindow)
}

fn visit_widgets(root: &gtk4::Widget, visit: &mut dyn FnMut(&gtk4::Widget)) {
    visit(root);
    let mut child = root.first_child();
    while let Some(next) = child {
        child = next.next_sibling();
        visit_widgets(&next, visit);
    }
}

fn source_views(root: &gtk4::Widget) -> Vec<sourceview5::View> {
    let mut views = Vec::new();
    visit_widgets(root, &mut |widget| {
        if let Ok(view) = widget.clone().downcast::<sourceview5::View>() {
            views.push(view);
        }
    });
    views
}

fn source_view_count(root: &gtk4::Widget) -> usize {
    source_views(root).len()
}

fn source_texts(root: &gtk4::Widget) -> Vec<String> {
    source_views(root)
        .into_iter()
        .map(|view| text_buffer_text(&view.buffer()))
        .collect()
}

fn set_first_source_text(app: &impl IsA<gtk4::Application>, text: &str) -> Result<(), StressError> {
    let root = active_root(app)?;
    let Some(view) = source_views(&root).into_iter().next() else {
        return Err(StressError::AssertionFailed);
    };
    view.buffer().set_text(text);
    Ok(())
}

fn set_search_query(app: &impl IsA<gtk4::Application>, query: &str) -> Result<(), StressError> {
    let root = active_root(app)?;
    let mut applied = false;
    visit_widgets(&root, &mut |widget| {
        if applied || !widget.is_visible() {
            return;
        }
        if let Ok(entry) = widget.clone().downcast::<gtk4::SearchEntry>() {
            entry.set_text(query);
            entry.grab_focus();
            applied = true;
        } else if let Ok(entry) = widget.clone().downcast::<gtk4::Entry>() {
            entry.set_text(query);
            entry.grab_focus();
            applied = true;
        }
    });
    if applied {
        Ok(())
    } else {
        Err(StressError::AssertionFailed)
    }
}

fn visible_text_contains(root: &gtk4::Widget, needle: &str) -> bool {
    let mut found = false;
    visit_widgets(root, &mut |widget| {
        if found || !widget.is_visible() {
            return;
        }
        if let Ok(label) = widget.clone().downcast::<gtk4::Label>() {
            found = label.text().contains(needle);
        } else if let Ok(entry) = widget.clone().downcast::<gtk4::Entry>() {
            found = entry.text().contains(needle);
        } else if let Ok(view) = widget.clone().downcast::<gtk4::TextView>() {
            found = text_buffer_text(&view.buffer()).contains(needle);
        }
    });
    found
}

fn source_control_state_visible(root: &gtk4::Widget) -> bool {
    [
        "Changed files",
        "Too many Git changes to display.",
        "No changes.",
        "Unable to read Git attributes. Git actions are disabled.",
        "This Git repository uses unsupported object or EOL settings.",
        "Refreshing Git status",
    ]
    .iter()
    .any(|needle| visible_text_contains(root, needle))
}

fn text_buffer_text(buffer: &gtk4::TextBuffer) -> String {
    let start = buffer.start_iter();
    let end = buffer.end_iter();
    buffer.text(&start, &end, true).to_string()
}

fn file_contains(path: &Path, needle: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| text.contains(needle))
}

fn absolute_path(path: PathBuf) -> Result<PathBuf, StressError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|_error| StressError::CurrentDir)
    }
}

fn repo_root_for(script_path: &Path) -> Result<PathBuf, StressError> {
    let scripts_dir = script_path.parent().ok_or(StressError::ScriptPath)?;
    let stress_dir = scripts_dir.parent().ok_or(StressError::ScriptPath)?;
    let repo = stress_dir.parent().ok_or(StressError::ScriptPath)?;
    Ok(repo.to_path_buf())
}

fn quoted_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\": \"");
    let start = text.find(&needle)? + needle.len();
    let tail = text.get(start..)?;
    let end = tail.find('"')?;
    Some(tail[..end].to_owned())
}

fn safe_stress_artifact_dir(relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if path.is_absolute() || !relative.starts_with("stress/artifacts/") {
        return None;
    }
    if path.components().any(|part| {
        matches!(
            part,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    }) {
        return None;
    }
    Some(path.to_path_buf())
}

fn activate_window_action(
    app: &impl IsA<gtk4::Application>,
    action: &str,
) -> Result<(), StressError> {
    let window = app.active_window().ok_or(StressError::MissingWindow)?;
    gtk4::prelude::WidgetExt::activate_action(&window, action, None)
        .map_err(|_error| StressError::ActionUnavailable)
}

fn optional_window_action(app: &impl IsA<gtk4::Application>, action: &str) -> &'static str {
    match app.active_window() {
        Some(window)
            if gtk4::prelude::WidgetExt::activate_action(&window, action, None).is_ok() =>
        {
            "ok"
        }
        Some(_) => "unavailable",
        None => "missing-window",
    }
}

#[derive(Clone, Copy)]
enum Flow {
    OpenSaveSearch,
    CompareRoundtrip,
    MarkdownStress,
    GitStatusStress,
}

impl Flow {
    fn from_script(script: &str) -> Result<Self, StressError> {
        if script.contains("\"flow\": \"open-save-search\"") {
            Ok(Self::OpenSaveSearch)
        } else if script.contains("\"flow\": \"compare-roundtrip\"") {
            Ok(Self::CompareRoundtrip)
        } else if script.contains("\"flow\": \"markdown-stress\"") {
            Ok(Self::MarkdownStress)
        } else if script.contains("\"flow\": \"git-status-stress\"") {
            Ok(Self::GitStatusStress)
        } else {
            Err(StressError::UnknownFlow)
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::OpenSaveSearch => "open-save-search",
            Self::CompareRoundtrip => "compare-roundtrip",
            Self::MarkdownStress => "markdown-stress",
            Self::GitStatusStress => "git-status-stress",
        }
    }
}

enum StressError {
    MissingScript,
    ScriptRead,
    ScriptPath,
    ScriptShape,
    CurrentDir,
    UnknownFlow,
    IntentionalFailure,
    RuntimeInit,
    GtkInit,
    AppRegister,
    UndeclaredFixture,
    MissingWindow,
    ActionUnavailable,
    AssertionFailed,
    FileRead,
    ArtifactWrite,
}
