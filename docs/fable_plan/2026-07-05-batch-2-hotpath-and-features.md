# Batch 2 — Hot-Path Fixes & Core Editor Features Implementation Plan

> **For agentic workers (Codex):** Execute tasks strictly in order, one task at
> a time. Each task ends with a passing Gate run and one commit. Steps use
> checkbox (`- [ ]`) syntax. Read `AGENTS.md` at the repo root before starting
> — its Validation and Hard Limits sections are binding. Every finding below
> was **re-verified against the working tree at commit `681924a`** on
> 2026-07-05; file/line anchors refer to that state.

**Goal:** Fix the two keystroke-hot-path regressions and the print-preview
inconsistency introduced by batch 1, remove the codebase's only lint
suppression, and add three small core-purpose features: Go to Line, session
cursor restore, and copyable commit hashes.

**Architecture:** All changes live under `app/`. Tasks 1–4 are surgical fixes
in existing files. Tasks 5–7 add small features following existing module
patterns (`document_statistics::present`, `set_git_statuses` plumbing,
session-settings in `settings/window_session.rs`). Task 8 updates docs and
records explicit scope decisions.

**Tech Stack:** Rust (edition 2024), gtk4-rs 0.11, libadwaita 0.9 (`adw`),
sourceview5, gettext-rs. Tests: cargo suite + GTK smoke harness + Python
policy/coverage gates.

## The Gate (run after every task)

From the repo root (subshells so the block is paste-safe):

```bash
(cd app && cargo fmt --all --check)
(cd app && cargo check --workspace --all-targets --all-features)
(cd app && cargo clippy --workspace --all-targets --all-features -- -D warnings)
(cd app && cargo test --workspace --all-targets --all-features)
python3 -m tools.policy_check --root app --strict
```

Baseline: **403 tests pass** at `681924a`. Final verification (after Task 8)
additionally runs `python3 -m tools.coverage_check --root app`, and Task 7
also needs `glib-compile-schemas --strict --dry-run app/data/schemas`.

**Known flake:** `gtk_tests::gtk_surfaces_and_editor_flow_work` can fail at
"v7 narrow compare suppresses minimap" when the live desktop session maps the
test window wider than requested (960 sp breakpoint). If it fails with no
plausible connection to your change, re-run it in isolation before digging;
do not "fix" minimap code as part of an unrelated task. Do not interact with
the desktop while the smoke test runs.

## Global Constraints

- Working directory for cargo: `app/`. Python gates run from the repo root.
- No `unwrap`/`expect`/`panic!`/`unsafe` in runtime code; no process spawning
  outside `src/git_process.rs`; no bare `as` casts (use `try_from` +
  `map_or`).
- All user-visible strings via `gettext`/`pgettext`/`ngettext`; **sentence
  concatenation forbidden**. Every new string lands in
  `app/po/io.github.cadric.Riteed.pot` **and** translated in `app/po/da.po`
  in the same commit (POT check is bidirectional; `da.po` allows no missing/
  fuzzy/untranslated entries). Verify with
  `msgfmt --check-format --check-header -o /dev/null app/po/da.po`.
- **Line budget:** production ≤ 600, test-glob files ≤ 800, no waivers.
  Current counts for files this plan touches: `document_tools.rs` 231,
  `source_control/history.rs` 355, `app.rs` 529, `settings.rs` 298,
  `settings/window_session.rs` 140, `git_process.rs` ~430,
  `workspace/tabs.rs` ~460. All additions fit; check `wc -l` after each task.
- **Validation manifests** (`app/build-aux/validation/`) pin
  `path`/`line`/`match` anchors. Manifest map for this plan's files
  (verified 2026-07-05):

  | File | Manifests |
  |---|---|
  | `workspace/tabs.rs` | runtime-review-v13-c |
  | `source_control/active_row.rs` | (ingen; `active_uri`-entry ligger i runtime-review-v13-a under `source_control.rs`) |
  | `git_process.rs` | runtime-review-v13-a, parser-boundaries |
  | `git_process/{ops,log}.rs` | (ingen) |
  | `document_tools.rs` | runtime-review (v1) |
  | `source_control/history.rs` | i18n-review, runtime-review-v13-a |
  | `app.rs` | runtime-review (v1) / i18n-review — fix anchors if lines shift |
  | `settings/window_session.rs` + `data/schemas/*.xml` | **gsettings-review** (session keys have `key`/`trigger` entries at lines ~305+) |
  | `workspace_open.rs` | parser-boundaries, runtime-review-v13-c |
  | `workspace/session_state.rs` | (ingen fundet) |

  Include `app/build-aux/validation/` in `git add` whenever an anchored file
  shifts, and add new entries for new gettext strings / shared state / new
  GSettings keys.
- **Precondition:** worktree is clean at `681924a` (verified). Commit this
  plan file first (`docs/fable_plan/` is tracked, unlike
  `docs/superpowers/`):

  ```bash
  git add docs/fable_plan/2026-07-05-batch-2-hotpath-and-features.md
  git commit -m "Add batch 2 hot-path and features plan"
  ```

---

### Task 1: Stop the un-coalesced dirty notify on the keystroke hot path

**Re-verified finding:** `bind_tab_to_workspace`'s visual-change closure
(`app/src/workspace/tabs.rs:448–453`) calls
`workspace.notify_dirty_state_changed()` directly. The visual-change handler
fires on **every cursor move** (`editor_tab/callbacks.rs:33`), and the
handler allocates a `gio::File` + URI `String` per open tab per call
(`dirty_session_uris`, `workspace.rs:447`). Meanwhile
`refresh_selected_state` (`workspace/selection.rs:44`) **already** calls
`notify_dirty_state_changed()`, and the visual-change closure already queues
exactly that via `queue_refresh_selected_state` (coalesced flag + idle,
`selection.rs:14–26`). The direct call is a redundant, synchronous duplicate
on the hot path.

**Files:**
- Modify: `app/src/workspace/tabs.rs` (one line removed)
- Modify (anchors if lines shift): `app/build-aux/validation/runtime-review-v13-c.v1.json`

- [ ] **Step 1: Remove the direct call**

Restore the closure to:

```rust
tab.set_visual_change_handler(Rc::new(move || {
    if let Some(workspace) = weak.upgrade() {
        workspace.queue_refresh_selected_state();
    }
}));
```

The queued `refresh_selected_state` → `notify_dirty_state_changed` chain
covers every dirty transition; the GTK dirty-dot test in `gtk_tests_v6.rs`
pumps idles via `drain_events`/`spin_until`, so its on/off assertions must
stay green and are the regression net.

- [ ] **Step 2: Run the Gate** — expect all 403 tests green, notably the v6
dirty-dot assertions.

- [ ] **Step 3: Commit**

```bash
git add app/src/workspace/tabs.rs app/build-aux/validation/
git commit -m "Coalesce dirty-state notifications through the selected-state refresh"
```

---

### Task 2: Early-out in `set_active_uri`

**Re-verified finding:** `set_active_uri`
(`app/src/source_control/active_row.rs:4–8`) has no equality check. It is
invoked from the git-action-sync handler inside **every**
`refresh_selected_state` (`window/sidebar_wiring.rs:77`), i.e. once per
coalesced keystroke idle, and unconditionally does `borrow_mut` +
`apply_active_row` → `mark_active_row`, which walks **all** bound rows in
both Source Control views doing `remove_css_class`/`add_css_class` per row
(`row_widgets.rs:43–53`) — even when the active URI is unchanged.

**Files:**
- Modify: `app/src/source_control/active_row.rs`

- [ ] **Step 1: Add the guard**

```rust
impl SourceControlController {
    pub(crate) fn set_active_uri(&self, uri: Option<String>) {
        if self.state.borrow().active_uri == uri {
            return;
        }
        self.state.borrow_mut().active_uri = uri;
        apply_active_row(&self.state.borrow());
    }
}
```

The rebuild path (`refresh.rs::rebuild_views` → `apply_active_row`) still
re-applies marking after every snapshot rebuild, so the guard cannot cause a
stale highlight.

- [ ] **Step 2: Run the Gate** — the v9 active-row assertion
(`source_control_active_row_path_for_tests`) is the regression net.

- [ ] **Step 3: Commit**

```bash
git add app/src/source_control/active_row.rs
git commit -m "Skip active-row remarking when the active URI is unchanged"
```

---

### Task 3: Remove the codebase's only lint suppression via `GitRunOptions`

**Re-verified finding:** `app/src/git_process.rs:172` carries
`#[allow(clippy::too_many_arguments)]` — the **only** `#[allow]` in
production source (verified by grep). AGENTS.md hard limits forbid lint
suppression as a shortcut. Bundling the three run knobs into one struct drops
`run`/`spec` to 5/3 domain parameters and deletes the attribute.
Also fold in audit note 5: the detached-mutating-child timeout caveat gets a
comment.

**Files:**
- Modify: `app/src/git_process.rs` (new `GitRunOptions`; `run`, `spec`,
  `run_text` signatures; delete the `#[allow]`)
- Modify: `app/src/git_process/ops.rs`, `app/src/git_process/log.rs` (call sites)
- Test: `app/src/git_process/tests.rs`
- Modify (anchors): `runtime-review-v13-a.v1.json`, `parser-boundaries.v1.json`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy)]
  struct GitRunOptions {
      stdout_cap: usize,
      allow_failure: bool,
      kill_on_cancel: bool,
  }
  ```
  `fn run<const N: usize>(&self, args, stdin, options: GitRunOptions, cancellable, callback)`;
  same shape for the private `fn spec`. `GitSpec`'s fields are unchanged (the
  existing `detect_repo_spec_kills_on_cancel` and
  `mutating_ops_opt_out_of_cancel_kill` tests keep passing as written — the
  const names remain at the call sites as field values).

- [ ] **Step 1: Write the failing test**

In `git_process/tests.rs` (concat so the test's own literal cannot satisfy
it):

```rust
#[test]
fn git_process_carries_no_lint_suppressions() {
    let source = include_str!("../git_process.rs");
    let marker = ["#[", "allow("].concat();
    assert!(!source.contains(&marker));
}
```

Run: `cargo test git_process_carries_no_lint_suppressions` → FAIL.

- [ ] **Step 2: Implement**

1. Define `GitRunOptions` next to `GitSpec`; `run`/`spec` take
   `options: GitRunOptions` instead of the three trailing scalars and copy
   them into `GitSpec`. Delete the `#[allow(...)]` block entirely.
2. `run_text` builds `GitRunOptions { stdout_cap: 4096, allow_failure, kill_on_cancel: true }`.
3. Update every call site in `ops.rs` (8) and `log.rs` (1), e.g.:

```rust
self.run(
    [ /* args unchanged */ ],
    None,
    GitRunOptions {
        stdout_cap: STATUS_CAP,
        allow_failure: false,
        kill_on_cancel: READ_ONLY_KILL_ON_CANCEL,
    },
    cancellable,
    /* callback unchanged */
);
```

4. Extend the `MUTATING_KILL_ON_CANCEL` doc comment in `ops.rs`:

```rust
/// Mutating git children must not be `SIGKILL`ed on user cancellation; a killed
/// index writer strands .git/index.lock. Note: once cancelled, the detached
/// child also loses its 30 s timeout guard and is only reaped via wait_async —
/// acceptable because these commands normally finish in milliseconds.
```

- [ ] **Step 3: Run the Gate** — fix shifted `git_process.rs` anchors in the
two manifests.

- [ ] **Step 4: Commit**

```bash
git add app/src/git_process.rs app/src/git_process/ops.rs app/src/git_process/log.rs \
        app/src/git_process/tests.rs app/build-aux/validation/
git commit -m "Bundle git run options and drop the lint suppression"
```

---

### Task 4: Print preview gets the same Markdown confirmation as print

**Re-verified finding:** `print_document` (`app/src/document_tools.rs:115`)
asks before printing raw Markdown source from preview, but `preview_document`
(same file, line ~160) still silently shows raw source in the print preview.
Both entries must share one dispatch.

**Files:**
- Modify: `app/src/document_tools.rs`
- Modify: `app/po/io.github.cadric.Riteed.pot`, `app/po/da.po` (two new strings)
- Modify (anchors + new strings): `runtime-review.v1.json` line fixes,
  `i18n-review.v1.json` entries if the Gate demands

- [ ] **Step 1: Restructure into one guarded dispatch**

```rust
#[derive(Clone, Copy)]
enum PrintEntry {
    Print,
    Preview,
}

fn print_document(self: &Rc<Self>) {
    self.dispatch_print_entry(PrintEntry::Print);
}

fn preview_document(self: &Rc<Self>) {
    self.dispatch_print_entry(PrintEntry::Preview);
}

fn dispatch_print_entry(self: &Rc<Self>, entry: PrintEntry) {
    let Some(tab) = self.workspace.selected_tab() else {
        return;
    };
    if print_needs_markdown_confirmation(tab.is_markdown_preview_active()) {
        self.present_markdown_print_confirmation(entry);
        return;
    }
    self.run_print_entry(entry);
}

fn run_print_entry(self: &Rc<Self>, entry: PrintEntry) {
    match entry {
        PrintEntry::Print => self.start_print(),
        PrintEntry::Preview => self.start_preview(),
    }
}
```

Rename today's `preview_document` body to `fn start_preview(&self)` (it keeps
its own `selected_tab` + guards, mirroring `start_print`). The receiver
change to `self: &Rc<Self>` compiles at the `install_callbacks` call sites
unchanged (they hold an upgraded `Rc`).

- [ ] **Step 2: Parametrize the dialog**

`present_markdown_print_confirmation(entry)` reuses the existing dialog code
from `print_document`, with per-entry body and affirmative label (complete
sentences — no concatenation):

```rust
let (body, affirm) = match entry {
    PrintEntry::Print => (
        gettext("Printing the formatted preview is not supported yet. The raw Markdown source will be printed."),
        pgettext("dialog button", "Print Source"),
    ),
    PrintEntry::Preview => (
        gettext("Previewing the formatted Markdown in print is not supported yet. The raw Markdown source will be shown in the print preview."),
        pgettext("dialog button", "Show Preview"),
    ),
};
```

Heading stays `gettext("Print Markdown Source?")`. The response callback
calls `controller.run_print_entry(entry)`.

- [ ] **Step 3: Catalogs**

POT + `da.po`:
- `"Previewing the formatted Markdown in print is not supported yet. The raw Markdown source will be shown in the print preview."`
  → da: `"Forhåndsvisning af formateret Markdown i print understøttes ikke endnu. Den rå Markdown-kilde vises i printforhåndsvisningen."`
- `msgctxt "dialog button"` / `"Show Preview"` → da: `"Vis forhåndsvisning"`.

`msgfmt --check-format --check-header -o /dev/null app/po/da.po` → OK.

- [ ] **Step 4: Run the Gate**, then manual verify note: MD preview aktiv →
Print Preview → dialog vises; "Show Preview" åbner previewet.

- [ ] **Step 5: Commit**

```bash
git add app/src/document_tools.rs app/po/ app/build-aux/validation/
git commit -m "Confirm before showing raw source in Markdown print preview"
```

---

### Task 5: Go to Line in the editor (F1)

**Re-verified gap:** no goto-line action exists for the editor (grep: only
the large-file viewer has line jump). `document_statistics::present(parent,
tab)` (`document_statistics.rs:17`) is the module pattern to mirror;
`EditorTab::select_offsets` (`editor_tab/view.rs:141`) and `grab_focus`
exist; accels live in `app.rs:124+` with **no existing `<Ctrl>i` binding**
(verified). GNOME Text Editor/gedit use Ctrl+I for Go to Line.

**Files:**
- Create: `app/src/document_goto_line.rs`
- Modify: `app/src/main.rs` (register `mod document_goto_line;` next to
  `mod document_statistics;` — match however statistics is declared)
- Modify: `app/src/document_tools.rs` (new `go-to-line` action, registered
  and gated exactly like `statistics_action`, incl. `sync_actions`
  enablement)
- Modify: `app/src/app.rs` (accel `win.go-to-line` → `<Ctrl>i`); if a
  shortcuts/help-overlay surface lists editor shortcuts (check
  `app.rs`/`window_shell.rs`/`workspace_menu.rs` — all three mention overlay
  material), add the entry there too
- Modify: `app/src/window/testing.rs` (test accessors)
- Test: unit tests in `document_goto_line.rs` + assertion in a gtk exercise
- Catalogs: POT + `da.po`; `i18n-review.v1.json` entries as the Gate demands

**Interfaces:**
- Produces: `pub(crate) fn present(parent: &adw::ApplicationWindow, tab: &Rc<EditorTab>)`;
  pure `fn parse_line_number(text: &str) -> Option<u32>`;
  `pub(crate) fn go_to_line(tab: &Rc<EditorTab>, line: u32)`;
  `Window::go_to_line_for_tests(&self, line: u32) -> bool` and
  `Window::selected_cursor_line_for_tests(&self) -> i32`.

- [ ] **Step 1: Write the failing unit tests**

In the new module's test block:

```rust
#[cfg(test)]
mod tests {
    use super::parse_line_number;

    #[test]
    fn line_numbers_parse_trimmed_positive_integers() {
        assert_eq!(parse_line_number(" 42 "), Some(42));
        assert_eq!(parse_line_number("1"), Some(1));
        assert_eq!(parse_line_number("0"), None);
        assert_eq!(parse_line_number(""), None);
        assert_eq!(parse_line_number("abc"), None);
        assert_eq!(parse_line_number("-3"), None);
    }
}
```

Run: `cargo test line_numbers_parse` → FAIL to compile.

- [ ] **Step 2: Implement the module**

```rust
use std::rc::Rc;

use gettextrs::{gettext, pgettext};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use crate::editor_tab::EditorTab;

pub(crate) fn present(parent: &adw::ApplicationWindow, tab: &Rc<EditorTab>) {
    let dialog = adw::AlertDialog::new(Some(&gettext("Go to Line")), None);
    let entry = gtk4::Entry::builder()
        .input_purpose(gtk4::InputPurpose::Digits)
        .activates_default(true)
        .build();
    entry.set_placeholder_text(Some(&pgettext("go to line", "Line number")));
    dialog.set_extra_child(Some(&entry));
    dialog.add_response("cancel", &pgettext("dialog button", "Cancel"));
    dialog.add_response("go", &pgettext("dialog button", "Go"));
    dialog.set_default_response(Some("go"));
    dialog.set_close_response("cancel");
    let weak = Rc::downgrade(tab);
    let entry_for_response = entry.clone();
    dialog.connect_response(Some("go"), move |_, _| {
        let Some(tab) = weak.upgrade() else {
            return;
        };
        let Some(line) = parse_line_number(entry_for_response.text().as_str()) else {
            return;
        };
        go_to_line(&tab, line);
    });
    dialog.present(Some(parent));
}

fn parse_line_number(text: &str) -> Option<u32> {
    let line = text.trim().parse::<u32>().ok()?;
    (line > 0).then_some(line)
}

pub(crate) fn go_to_line(tab: &Rc<EditorTab>, line: u32) {
    let buffer = tab.text_buffer();
    let last_line = buffer.line_count().saturating_sub(1).max(0);
    let target = i32::try_from(line.saturating_sub(1)).map_or(last_line, |value| value.min(last_line));
    let Some(iter) = buffer.iter_at_line(target) else {
        return;
    };
    let offset = iter.offset();
    tab.select_offsets(offset, offset);
    tab.grab_focus();
}
```

(Out-of-range input clamps to the last line — matching the viewer's
forgiving behavior. Check `select_offsets`/`grab_focus` visibility; widen to
`pub(crate)` if needed.)

- [ ] **Step 3: Action + accel**

In `DocumentToolsController::new`, add `goto_line_action =
gio::SimpleAction::new("go-to-line", None)`, `parent.add_action`, connect to
`document_goto_line::present(&self.parent, &tab)` guarded by the same
`is_document() && !is_loading()` checks as statistics, and mirror the
statistics enable/disable in `sync_actions`. In `app.rs`:

```rust
app.set_accels_for_action("win.go-to-line", &["<Ctrl>i"]);
```

- [ ] **Step 4: GTK assertion + accessors**

`window/testing.rs`:

```rust
pub(crate) fn go_to_line_for_tests(&self, line: u32) -> bool {
    let Some(tab) = self.workspace.selected_tab() else {
        return false;
    };
    crate::document_goto_line::go_to_line(&tab, line);
    true
}

pub(crate) fn selected_cursor_line_for_tests(&self) -> i32 {
    self.workspace.selected_tab().map_or(-1, |tab| {
        let buffer = tab.text_buffer();
        buffer.iter_at_mark(&buffer.get_insert()).line()
    })
}
```

In an existing editor gtk exercise that opens a multi-line document — **not**
the session-restore exercise at `gtk_tests.rs:241` (Task 7 adds cursor
assertions there, and a goto here would fight them). Use an exercise that
opens a temp file via `write_temp_file` + `request_open_files` (pattern at
`gtk_tests.rs:160`), with at least 4 lines of content, and add:

```rust
assert!(window.go_to_line_for_tests(3));
assert_eq!(window.selected_cursor_line_for_tests(), 2);
assert!(window.go_to_line_for_tests(9_999));
assert!(window.selected_cursor_line_for_tests() >= 2);
```

- [ ] **Step 5: Catalogs**

POT + da.po: `"Go to Line"` → `"Gå til linje"`; ctx `go to line` /
`"Line number"` → `"Linjenummer"`; ctx `dialog button` / `"Go"` → `"Gå"`
(check whether `dialog button`/`Cancel` already exists — it does after
batch 1). i18n-review entries as the Gate demands.

- [ ] **Step 6: Run the Gate + commit**

```bash
git add app/src/document_goto_line.rs app/src/main.rs app/src/document_tools.rs \
        app/src/app.rs app/src/window/testing.rs app/src/gtk_tests*.rs \
        app/po/ app/build-aux/validation/
git commit -m "Add Go to Line for the editor"
```

---

### Task 6: Recent Commits rows copy the commit hash (F3)

**Re-verified gap:** `commit_row` (`source_control/history.rs:297–309`) sets
`set_activatable(false)` and hides the full hash in a tooltip. Minimal
valuable interaction: activating a row copies the full hash to the clipboard
with a toast. `Workspace::show_toast` exists (`workspace.rs:410`);
`SourceControlState.workspace: Weak<Workspace>` exists, and
`SourceControlHistory::new()` is called from `SourceControlController::new`
where the workspace is in scope (`source_control.rs:159`).

**Files:**
- Modify: `app/src/source_control/history.rs` (struct gains
  `workspace: Weak<Workspace>`; `new(workspace: Weak<Workspace>)`;
  `commit_row` becomes a method and wires activation)
- Modify: `app/src/source_control.rs` (call site:
  `SourceControlHistory::new(Rc::downgrade(workspace))`)
- Modify: `runtime-review-v13-a.v1.json` (new shared-state entry for the
  `workspace` weak field on the history widget; shifted history.rs anchors),
  `i18n-review.v1.json` (new toast string)
- Catalogs: POT + `da.po`
- Test: gtk_tests_v9 + accessor in `source_control/testing.rs` / `window/testing.rs`

- [ ] **Step 1: Implement**

In `history.rs` (imports: add `std::rc::Weak`, `crate::workspace::Workspace`):

```rust
pub(super) struct SourceControlHistory {
    // ...existing fields...
    workspace: Weak<Workspace>,
}
```

`new(workspace: Weak<Workspace>)` stores it. `set_commits` calls
`self.commit_row(commit)`:

```rust
fn commit_row(&self, commit: &GitCommitSummary) -> adw::ActionRow {
    let subtitle = format!(
        "{} · {} · {}",
        commit.author, commit.date, commit.short_hash
    );
    let row = adw::ActionRow::builder()
        .title(&commit.subject)
        .subtitle(&subtitle)
        .tooltip_text(&commit.full_hash)
        .use_markup(false)
        .activatable(true)
        .build();
    let hash = commit.full_hash.clone();
    let workspace = self.workspace.clone();
    row.connect_activated(move |row| {
        row.clipboard().set_text(&hash);
        if let Some(workspace) = workspace.upgrade() {
            workspace.show_toast(&gettext("Commit hash copied."));
        }
    });
    row
}
```

(The old free `fn commit_row` and its `set_activatable(false)` disappear.)
Call-site fix in `source_control.rs:159`:
`let history = SourceControlHistory::new(Rc::downgrade(workspace));`

- [ ] **Step 2: Test accessor + assertion**

`source_control/testing.rs`:

```rust
pub(crate) fn first_commit_row_activatable_for_tests(&self) -> bool {
    let state = self.state.borrow();
    state
        .history
        .first_row_for_tests()
        .is_some_and(|row| row.is_activatable())
}
```

with, in `history.rs`:

```rust
#[cfg(test)]
pub(super) fn first_row_for_tests(&self) -> Option<adw::ActionRow> {
    self.list.first_child()?.downcast::<adw::ActionRow>().ok()
}
```

Forward through `window/testing.rs`; in the v9 exercise, after history loads
commits, assert `window.source_control_first_commit_row_activatable_for_tests()`.
(Clipboard content is not asserted — clipboard reads are async and
session-dependent; the activation handler is 4 lines and covered by the
manual verification note.)

- [ ] **Step 3: Catalogs + manifests**

POT + da.po: `"Commit hash copied."` → `"Commit-hash kopieret."`.
i18n-review entry (short toast string in the Source Control context);
runtime-review-v13-a: new entry for the history widget's `workspace` weak ref
(ownership: "The history widget holds a weak workspace handle for clipboard
toasts."), plus line fixes.

- [ ] **Step 4: Run the Gate + commit**

```bash
git add app/src/source_control/history.rs app/src/source_control.rs \
        app/src/source_control/testing.rs app/src/window/testing.rs \
        app/src/gtk_tests_v9.rs app/po/ app/build-aux/validation/
git commit -m "Copy the commit hash when a Recent Commits row is activated"
```

---

### Task 7: Restore cursor positions on session restore (F2)

**Re-verified gap:** `workspace/session_state.rs` persists file URIs +
selected file only; every restored file opens at line 1. Session settings
live in `settings/window_session.rs` (140 lines) with schema keys
`session-files` (`as`) / `session-selected-file` (`s`)
(`gschema.xml:155–160`), and `gsettings-review.v1.json` carries `key` +
`trigger` entries for them (lines ~300–330). The restore flow's completion
hook is the `Ok` branch in `process_open_request`
(`workspace_open.rs:~212`), which fires **after** the (possibly chunked)
apply completes. The existing session-restore GTK exercise sits at
`gtk_tests.rs:241–255`.

**Design:** one new `as` key, entries `"<uri>|<offset>"` (character offset;
`|` cannot appear unencoded in a URI, parse with `rsplit_once('|')`, skip
malformed entries).

**Files:**
- Modify: `app/data/schemas/io.github.cadric.Riteed.gschema.xml` (new key)
- Modify: `app/src/settings/window_session.rs` (getters/setters + pure
  format/parse helpers + unit tests)
- Modify: `app/src/settings.rs` (memory-backend field for tests — mirror how
  `session_files` is stored in `MemorySettings`)
- Modify: `app/src/workspace/session_state.rs` (persist offsets alongside files)
- Modify: `app/src/workspace_open.rs` (apply stored offset in the
  session-restore `Ok` branch)
- Modify: `app/build-aux/validation/gsettings-review.v1.json` (new key entry
  with `key` + `trigger`, modeled on the `session-selected-file` entry)
- Test: unit tests + extend the `gtk_tests.rs:241` restore exercise

**Interfaces:**
- Produces (in `window_session.rs`):
  `pub fn session_cursors(&self) -> Vec<(String, i32)>`,
  `pub fn set_session_cursors(&self, cursors: &[(String, i32)])`,
  pure `fn format_cursor_entry(uri: &str, offset: i32) -> String`,
  pure `fn parse_cursor_entry(entry: &str) -> Option<(String, i32)>`.

- [ ] **Step 1: Failing unit tests**

In `window_session.rs`:

```rust
#[test]
fn cursor_entries_round_trip_and_reject_malformed() {
    assert_eq!(
        format_cursor_entry("file:///tmp/a.txt", 42),
        "file:///tmp/a.txt|42"
    );
    assert_eq!(
        parse_cursor_entry("file:///tmp/a.txt|42"),
        Some((String::from("file:///tmp/a.txt"), 42))
    );
    // URIs percent-encode '|', so the last separator always wins:
    assert_eq!(parse_cursor_entry("no-separator"), None);
    assert_eq!(parse_cursor_entry("file:///tmp/a.txt|not-a-number"), None);
    assert_eq!(parse_cursor_entry("|7"), None);
}
```

Run → FAIL to compile.

- [ ] **Step 2: Schema + settings**

`gschema.xml`, after `session-selected-file`:

```xml
<key name="session-cursors" type="as">
  <default>[]</default>
  <summary>Session Cursor Positions</summary>
  <description>Stores the last cursor offset for each restored session file as URI and offset pairs.</description>
</key>
```

`window_session.rs`: implement the four functions (backend match like
`session_files`; memory backend stores `Vec<String>` raw entries):

```rust
fn format_cursor_entry(uri: &str, offset: i32) -> String {
    format!("{uri}|{offset}")
}

fn parse_cursor_entry(entry: &str) -> Option<(String, i32)> {
    let (uri, offset) = entry.rsplit_once('|')?;
    if uri.is_empty() {
        return None;
    }
    let offset = offset.parse::<i32>().ok()?;
    Some((String::from(uri), offset.max(0)))
}
```

Verify: `glib-compile-schemas --strict --dry-run app/data/schemas`.

- [ ] **Step 3: Persist**

In `workspace/session_state.rs`, where session files are collected from tabs
(line ~41), also collect offsets:

```rust
let cursors: Vec<(String, i32)> = tabs
    .iter()
    .filter_map(|tab| {
        let uri = tab.session_uri()?;
        let buffer = tab.text_buffer();
        let offset = buffer.iter_at_mark(&buffer.get_insert()).offset();
        Some((uri, offset))
    })
    .collect();
settings.set_session_cursors(&cursors);
```

(Adapt names to the function's real locals; write in the same place
`set_session_files` is written so the two stay consistent.)

- [ ] **Step 4: Restore**

In the session-restore flow in `workspace_open.rs`: read
`settings.session_cursors()` into a `HashMap<String, i32>` when the restore
request is created, carry it on the request struct, and in the `Ok(uri)`
branch of `process_open_request` (which runs post-apply):

```rust
if source == OpenSource::SessionRestore
    && let Some(offset) = request.borrow().cursors.get(&uri).copied()
{
    let char_count = tab_for_result.text_buffer().char_count();
    let clamped = offset.clamp(0, char_count.saturating_sub(1).max(0));
    tab_for_result.select_offsets(clamped, clamped);
}
```

(Match the request struct's real shape; the selected tab's scroll follows via
`select_offsets`.)

- [ ] **Step 5: GTK assertion**

Extend the restore exercise at `gtk_tests.rs:241`: before building the
restore window, also call
`restore_settings.set_session_cursors(&[(second_uri.clone(), 5)]);` and after
`spin_until("restore session", ...)` assert
`window.selected_cursor_line_for_tests()` / a new
`selected_cursor_offset_for_tests()` equals 5 (add the offset accessor next
to the line accessor from Task 5).

- [ ] **Step 6: gsettings-review + Gate**

Add the `session-cursors` entry to `gsettings-review.v1.json` modeled on
`session-selected-file` (trigger: "Persisting the session writes cursor
offsets at the same site as the session file list, outside restore mode.").
Run the Gate + `glib-compile-schemas --strict --dry-run app/data/schemas`.

- [ ] **Step 7: Commit**

```bash
git add app/data/schemas/ app/src/settings/window_session.rs app/src/settings.rs \
        app/src/workspace/session_state.rs app/src/workspace_open.rs \
        app/src/window/testing.rs app/src/gtk_tests.rs app/build-aux/validation/
git commit -m "Restore cursor positions when a session is restored"
```

---

### Task 8: Docs, changelog, and explicit scope decisions

**Files:**
- Modify: `CHANGELOG.md` (Unreleased)
- Modify: `docs/mangler-og-bugs.md`

- [ ] **Step 1: CHANGELOG**

Under `### Added`:

```markdown
- Go to Line (Ctrl+I) for the editor.
- Session restore returns each file to its last cursor position.
- Activating a Recent Commits row copies the full commit hash.
```

Under `### Changed`:

```markdown
- Print preview now asks before showing raw Markdown source, matching print.
```

Under `### Fixed`:

```markdown
- Dirty-state and Source Control active-row bookkeeping no longer run
  redundant work on every keystroke.
```

- [ ] **Step 2: mangler-og-bugs.md**

1. Add a section documenting the **v7 GTK smoke geometry flake** (the
   "narrow compare suppresses minimap" assertion depends on the mapped
   window width vs. a 960 sp breakpoint on the live session; direction: pin/
   verify the test window's mapped width before the assertion).
2. Add a section "Bevidste git-scope-beslutninger (åbne spørgsmål)" listing:
   dirty-dot/badge-propagering til mapper, flerlinjes commit-besked,
   formateret Markdown-print (allerede tracket), branch-skift, discard af
   untracked, log-grænsen på 25 commits — each with a one-line
   accept/afvis-question so they stop being implicit.

- [ ] **Step 3: Final verification + commit**

Run the Gate **plus** `python3 -m tools.coverage_check --root app`.

```bash
git add CHANGELOG.md docs/mangler-og-bugs.md
git commit -m "Document batch 2 fixes and open scope decisions"
```

---

## Deliberately out of scope

- Commit-diff view on Recent Commits activation (needs multi-file diff
  plumbing; copy-hash is the stepping stone).
- Folder-level dirty/badge propagation, multi-line commit message, branch
  switching — awaiting the Task 8 scope decisions.
- Formatted Markdown printing (tracked separately).
- Any minimap/v7-flake code change (test-side issue, tracked in Task 8).

## Final verification (after Task 8)

```bash
(cd app && cargo fmt --all --check)
(cd app && cargo check --workspace --all-targets --all-features)
(cd app && cargo clippy --workspace --all-targets --all-features -- -D warnings)
(cd app && cargo test --workspace --all-targets --all-features)
python3 -m tools.policy_check --root app --strict
python3 -m tools.coverage_check --root app
glib-compile-schemas --strict --dry-run app/data/schemas
```
