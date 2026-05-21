#![forbid(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

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
    let script_path = std::env::var_os(SCRIPT_ENV).ok_or(StressError::MissingScript)?;
    let script = std::fs::read_to_string(script_path).map_err(|_error| StressError::ScriptRead)?;
    if script.contains("\"expect_failure\": true") {
        return Err(StressError::IntentionalFailure);
    }
    let flow = Flow::from_script(&script)?;
    run_app_flow(flow)
}

fn run_app_flow(flow: Flow) -> Result<(), StressError> {
    riteed::bootstrap_runtime().map_err(|_error| StressError::RuntimeInit)?;
    gtk4::init().map_err(|_error| StressError::GtkInit)?;
    let riteed = riteed::app::RiteedApp::new();
    let app = riteed.application().clone();
    app.register(None::<&gio::Cancellable>)
        .map_err(|_error| StressError::AppRegister)?;

    match flow {
        Flow::OpenSaveSearch => app.open(&[stress_file("open-save-search.txt", large_text())?], ""),
        Flow::CompareRoundtrip => app.open(
            &[
                stress_file("compare-reference.txt", "alpha\nbeta\n")?,
                stress_file("compare-current.txt", "alpha\ngamma\n")?,
            ],
            "",
        ),
        Flow::MarkdownStress => {
            app.open(&[stress_file("markdown-stress.md", markdown_text())?], "");
        }
        Flow::GitStatusStress => app.open(&[stress_folder("git-status-stress")?], ""),
    }

    spin_for(Duration::from_millis(RUN_MILLIS));
    app.quit();
    spin_for(Duration::from_millis(50));
    Ok(())
}

fn stress_file(name: &str, text: impl AsRef<[u8]>) -> Result<gio::File, StressError> {
    let path = std::env::temp_dir().join(format!("riteed-stress-{name}"));
    std::fs::write(&path, text.as_ref()).map_err(|_error| StressError::TempWrite)?;
    Ok(gio::File::for_path(path))
}

fn stress_folder(name: &str) -> Result<gio::File, StressError> {
    let path = std::env::temp_dir().join(format!("riteed-stress-{name}"));
    std::fs::create_dir_all(&path).map_err(|_error| StressError::TempWrite)?;
    Ok(gio::File::for_path(path))
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

fn large_text() -> String {
    let mut text = String::from("needle\n");
    text.push_str(&"alpha beta gamma\n".repeat(512));
    text
}

fn markdown_text() -> String {
    let mut text = String::from("---\ntitle: Stress\n---\n# Stress\n\n");
    text.push_str("- item\n- item\n\n```rust\nlet value = 1;\n```\n\n");
    text.push_str("![local](image.png)\n<div>literal</div>\n");
    text
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
}

enum StressError {
    MissingScript,
    ScriptRead,
    UnknownFlow,
    IntentionalFailure,
    RuntimeInit,
    GtkInit,
    AppRegister,
    TempWrite,
}
