# Non-Blocking Large-File Apply + Long-Line Viewer Routing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Closing a tab must work instantly at any point while a large (5–25 MiB) file is opening, and files with pathologically long lines must never freeze the editor.

**Architecture:** Two independent changes. (A) Replace the synchronous `set_text(whole document)` in `apply_loaded_document` with a chunked, cancellable insertion driven by `glib::idle_add_local` with a per-tick time budget, guarded by the existing IO-generation mechanism so `cancel_io()` (already called on tab close) aborts it immediately. (B) After decode, detect lines longer than a hard cap and route such files to the existing V15 read-only large-file viewer instead of the editor, because GtkTextView/Pango cannot lay out megabyte-long lines without multi-second main-thread stalls.

**Tech Stack:** Rust 2024 (stable 1.95.x), gtk4-rs 0.11.x, libadwaita, sourceview5, glib main loop. No new dependencies, no threads, no new GSettings keys.

---

## Background: measured root cause (2026-06-10)

Instrumented GTK tests against the real code (presented window, 20 MiB files) showed:

| Phase | Measured | Verdict |
|---|---|---|
| `sourceview5::FileLoader` async read of 20 MiB | 0.25 s, **zero** main-loop stalls | not the problem |
| `GCancellable` cancel during load | **0 ms** latency | already works |
| Tab close during the read phase | 5–7 ms to detach | already works |
| `replace_buffer_text` (`set_text` of full document) | ~300 ms hard main-thread stall **per file**, stalls stack when several files open | fix = Task A |
| GtkTextView height validation after apply | ~13 s of saturated main loop per 20 MiB file (low-priority, but compounds) | mitigated by A spreading work |
| File with ~1 MiB-long lines (10 MiB total) | repeated **2.1–2.5 s** single stalls (Pango layout of one line is unpreemptible) | fix = Task B |

`on_page_detached` (`app/src/workspace_close.rs:113`) already calls `tab.cancel_io()`; the cancellation plumbing in `EditorIoState` (generation + cancellable) is reused by this plan.

## Compliance with AGENTS.md and policy pack

- **Main loop discipline** (`gtk4-rs.policy.json`, AGENTS "Keep UI work on the main loop"): chunked apply stays on the main loop with a 15 ms tick budget — no threads, no async runtime (broad async runtimes are forbidden). `glib::idle_add_local` does **not** match the policy grep-warning `\bglib::idle_add\b` (word boundary), and `timeout_add_local`/`idle_add_local_once` are already used in this codebase (`workspace_close.rs`, markdown debounce).
- **No `unsafe`/`unwrap`/`expect`/`panic!`** in runtime code: all code below avoids them.
- **Line limits (600 prod / 800 test):** new logic goes in a new file `app/src/editor_tab/apply.rs`; `runtime.rs` (522) *loses* lines via refactor; `open.rs` (468), `editor_io.rs` (446), `document_limits.rs` (530), `state.rs` (324) get small additions and must stay ≤ 600 — verify with `wc -l` before committing each task.
- **Localizable by default:** exactly one new user-visible string (`AppError::LineTooLong` body), added to POT + `po/da.po` (Task 9). No other copy changes.
- **GSettings:** none. The long-line cap is a constant like the existing `EDITOR_HARD_LIMIT_BYTES`/`EDITOR_POLICY_CEILING_BYTES`; it is not a user preference. (If the maintainer later wants it configurable, that is a separate schema + preferences + gsettings-review change — out of scope.)
- **Parser boundaries (`stress-fuzz.policy.json`):** the long-line gate extends the existing `document_file_load` boundary (post-decode limits). If `python3 -m tools.policy_check --root app --strict` reports unmatched scanner hits or stale anchors in `app/build-aux/validation/*.json`, update the affected review artifact entries (`path`/`line`/`match`/`kind` per `policy/README.md`) in Task 10 — never suppress the validator.
- **AGENTS Core Workflow 0.4:** before coding, verify current gtk-rs API signatures with context7 (`/gtk-rs/gtk4-rs`): `glib::idle_add_local`, `glib::SourceId`, `glib::ControlFlow`, `TextBufferExt::insert`, `TextView::set_editable`.

**Out of scope (deliberate):** differentiated viewer copy for long-line files (the read-only viewer's existing "Editing Is Not Available" alert mentions the size limit; wording follow-up is a separate copy/i18n change). Any change to tier thresholds or the 25 MiB editor cap.

## File map

| File | Action | Responsibility |
|---|---|---|
| `app/src/editor_tab/apply.rs` | **create** | chunked apply: constants, chunk boundary helper, `apply_loaded_document_async`, tick/finish logic |
| `app/src/editor_tab.rs` | modify | register `mod apply;`; `is_dirty()` loading guard |
| `app/src/editor_tab/state.rs` | modify | `EditorIoState.pending_apply: Option<glib::SourceId>` + take helper + unit test |
| `app/src/editor_tab/runtime.rs` | modify | split `apply_loaded_document` into `begin_loaded_document_state` / `finish_loaded_document_presentation`; remove the old monolith; `cancel_io`/`start_io_request` also drop pending apply |
| `app/src/editor_tab/open.rs` | modify | 3 call sites switch to async apply; handle `LoadFailure::LineTooLong` (viewer route / reload error) |
| `app/src/editor_tab/save.rs` | modify | refuse save while loading |
| `app/src/editor_io.rs` | modify | `LoadFailure::LineTooLong` variant + post-decode line scan |
| `app/src/document_limits.rs` | modify | `EDITOR_MAX_LINE_BYTES` + `text_supports_editor_line_lengths` + tests |
| `app/src/error.rs` | modify | `AppError::LineTooLong` + localized copy |
| `app/src/window/testing.rs` | modify | test helpers: close selected page, selected char count |
| `app/src/gtk_tests_boundaries.rs` | modify | GTK tests: integrity, close-during-apply, long-line viewer routing |
| `app/po/io.github.cadric.Riteed.pot`, `app/po/da.po` | modify | one new msgid |
| `CHANGELOG.md` | modify | Unreleased entry |
| `app/build-aux/validation/*.json` | modify if flagged | anchored review entries per validator output |

Run all `cargo`/test commands from `app/`.

---

### Task 1: Pending-apply tracking in `EditorIoState`

**Files:**
- Modify: `app/src/editor_tab/state.rs` (struct at ~line 114, tests at bottom)

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `state.rs`:

```rust
    #[test]
    fn editor_io_take_pending_apply_clears_stored_source() {
        let mut state = EditorIoState::default();
        assert!(state.take_pending_apply().is_none());

        let source = gtk4::glib::timeout_add_local_once(
            std::time::Duration::from_secs(3600),
            || {},
        );
        state.pending_apply = Some(source);
        assert!(state.take_pending_apply().is_some());
        assert!(state.take_pending_apply().is_none());
    }
```

Note: `timeout_add_local_once` requires a main context owned by this thread; this works in the plain `cargo test` harness because glib creates/acquires the default context lazily on first use. If it errors under test, replace the source creation with `glib::idle_add_local_once(|| {})`. The returned `SourceId` from `take_pending_apply` must be dropped without `.remove()` in this test (dropping a `SourceId` does not remove the source; the 1-hour timer simply never fires in the test process).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib editor_io_take_pending_apply -- --nocapture`
Expected: FAIL — `no field pending_apply` / `no method take_pending_apply`.

- [ ] **Step 3: Implement** — in `state.rs`, extend `EditorIoState` and add the helper:

```rust
#[derive(Default)]
pub(super) struct EditorIoState {
    pub(super) generation: u64,
    pub(super) cancellable: Option<gio::Cancellable>,
    pub(super) candidate_encodings: Option<SList<sourceview5::Encoding>>,
    pub(super) loading: bool,
    pub(super) external_reload_in_progress: bool,
    pub(super) pending_apply: Option<gtk4::glib::SourceId>,
}
```

```rust
    pub(super) fn take_pending_apply(&mut self) -> Option<gtk4::glib::SourceId> {
        self.pending_apply.take()
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib editor_io_take_pending_apply`
Expected: PASS. Also run `cargo test --lib state` to confirm no existing state test broke.

- [ ] **Step 5: Commit**

```bash
git add app/src/editor_tab/state.rs
git commit -m "Add pending-apply source tracking to editor IO state"
```

---

### Task 2: Chunk boundary helper and new `apply` module

**Files:**
- Create: `app/src/editor_tab/apply.rs`
- Modify: `app/src/editor_tab.rs` (add `mod apply;` next to `mod open;` at ~line 19)

- [ ] **Step 1: Create the module with constants, helper, and failing tests**

`app/src/editor_tab/apply.rs`:

```rust
use std::time::Duration;

/// Documents at or below this size keep today's synchronous apply.
pub(super) const CHUNKED_APPLY_MIN_BYTES: usize = 1024 * 1024;
/// Upper bound for a single buffer insertion.
const CHUNK_TARGET_BYTES: usize = 2 * 1024 * 1024;
/// Budget for one idle tick before yielding back to the main loop.
const CHUNK_TICK_BUDGET: Duration = Duration::from_millis(15);

/// Largest `end > start` with `end <= start + target`, `end <= text.len()`,
/// and `end` on a `char` boundary. Returns `start` only when already at the end.
fn chunk_end_at_char_boundary(text: &str, start: usize, target: usize) -> usize {
    let mut end = start.saturating_add(target).min(text.len());
    while end > start && !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    end
}

#[cfg(test)]
mod tests {
    use super::chunk_end_at_char_boundary;

    #[test]
    fn chunk_end_respects_target_and_length() {
        assert_eq!(chunk_end_at_char_boundary("abcdef", 0, 4), 4);
        assert_eq!(chunk_end_at_char_boundary("abcdef", 4, 4), 6);
        assert_eq!(chunk_end_at_char_boundary("abcdef", 6, 4), 6);
    }

    #[test]
    fn chunk_end_backs_up_to_char_boundary() {
        // 'é' is two bytes (0xC3 0xA9); target 3 lands mid-char.
        let text = "abé";
        assert_eq!(chunk_end_at_char_boundary(text, 0, 3), 2);
        assert_eq!(chunk_end_at_char_boundary(text, 2, 3), 4);
    }

    #[test]
    fn chunk_end_handles_multibyte_run() {
        let text = "ééé";
        assert_eq!(chunk_end_at_char_boundary(text, 0, 1), 0);
        assert_eq!(chunk_end_at_char_boundary(text, 0, 2), 2);
    }
}
```

In `app/src/editor_tab.rs`, after `mod open;` add:

```rust
mod apply;
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib chunk_end`
Expected: PASS (3 tests). `cargo check` may warn about unused constants — that is fine only transiently; Task 3 consumes them within the same PR, but each commit must build clean under `-D warnings`, so add `#[allow(...)]`? **No** — lint suppression is forbidden. Instead: keep Step 1 and Task 3 in one commit if `cargo clippy --workspace --all-targets --all-features -- -D warnings` flags dead code. Check first; `pub(super)` consts referenced by tests usually pass. If clippy fails on unused items, squash this task's commit into Task 3's commit.

- [ ] **Step 3: Commit**

```bash
git add app/src/editor_tab/apply.rs app/src/editor_tab.rs
git commit -m "Add chunked-apply module with char-boundary chunk helper"
```

---

### Task 3: Split `apply_loaded_document` into begin/finish halves

Pure refactor (behavior-preserving) so the async path can reuse both halves.

**Files:**
- Modify: `app/src/editor_tab/runtime.rs` (`apply_loaded_document` at ~line 245)

- [ ] **Step 1: Refactor** — replace `apply_loaded_document` with three methods. `begin_loaded_document_state` takes `&LoadedDocument` (cheap clones; the 20 MB `text` is **not** cloned):

```rust
    pub(super) fn apply_loaded_document(
        self: &Rc<Self>,
        document: LoadedDocument,
        snapshot: Option<&ReloadSnapshot>,
    ) {
        self.begin_loaded_document_state(&document);
        self.replace_buffer_text(&document.text, document.format.implicit_trailing_newline());
        self.finish_loaded_document_presentation(&document, snapshot);
    }

    pub(super) fn begin_loaded_document_state(self: &Rc<Self>, document: &LoadedDocument) {
        self.exit_markdown_preview();
        self.exit_compare();
        self.clear_large_file_surface();
        self.content.set_visible(true);
        let loaded_size = loaded_document_gate_size(document.disk_size, document.text.len());
        {
            let mut state = self.state.borrow_mut();
            state.large_file.surface = DocumentSurface::Editor;
            state.large_file.file_size = Some(loaded_size);
            state.document.document = DocumentState::from_loaded_with_display_path(
                document.path.clone(),
                document.display_path.clone(),
                document.format.clone(),
            );
            state.document.saved_format = document.format.clone();
            state.document.source_file = Some(document.source_file.clone());
            state.external.pending = PendingExternalState::Idle;
            state.external.writability = Writability::Unknown;
            state.autosave.paused_message = None;
            state.ui.external_prompt_active = false;
            state.ui.visible_banner = crate::editor_tab::VisibleBannerState::None;
            state.document.content_type = None;
            state.document.language_id = None;
            state.ui.suppress_changes = true;
        }
    }

    pub(super) fn finish_loaded_document_presentation(
        self: &Rc<Self>,
        document: &LoadedDocument,
        snapshot: Option<&ReloadSnapshot>,
    ) {
        if let Some(snapshot) = snapshot {
            snapshot.apply(&self.text_buffer);
        }
        self.text_buffer.set_modified(false);
        self.state.borrow_mut().ui.suppress_changes = false;
        self.apply_minimap_visibility();
        self.sync_markdown_preview_availability();
        if !self.editor_heavy_features_enabled() {
            self.clear_source_control_minimap_diff();
        }
        self.set_attention(false);
        self.set_banner_revealed(false);
        if let Some(file) = self.saved_file() {
            self.refresh_writability_for_file(&file);
        }
        self.resolve_display_path_for_access_path(&document.path);
    }
```

Important deltas vs. the old body: the old `apply_loaded_document` consumed `document.path` / `display_path` / `source_file` by move and kept a `loaded_access_path` copy; the refactor clones these (PathBuf/Option<PathBuf>/glib handle — cheap) and uses `&document.path` at the end. Delete the now-unused `loaded_access_path` binding.

- [ ] **Step 2: Verify behavior is unchanged**

Run: `cargo check --workspace --all-targets --all-features` then `cargo test --lib gtk_surfaces_and_editor_flow_work -- --nocapture` (needs a display; this is the repo's single GTK test entry).
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add app/src/editor_tab/runtime.rs
git commit -m "Split loaded-document apply into reusable begin/finish halves"
```

---

### Task 4: Chunked `apply_loaded_document_async` + cancellation wiring

**Files:**
- Modify: `app/src/editor_tab/apply.rs` (main logic)
- Modify: `app/src/editor_tab/runtime.rs` (`cancel_io` ~line 18, `start_io_request` ~line 412; delete the sync `apply_loaded_document` once unreferenced)
- Modify: `app/src/editor_tab/open.rs` (3 call sites: load ~line 233, reload ~line 104, reopen ~line 324)
- Modify: `app/src/editor_tab.rs` (`is_dirty` ~line 268)
- Modify: `app/src/editor_tab/save.rs` (`request_save` ~line 50)

- [ ] **Step 1: Implement the async apply in `apply.rs`** (add below the helper; keep the existing tests module last):

```rust
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gtk4::glib;
use gtk4::prelude::*;
use sourceview5::prelude::*;

use crate::editor_io::LoadedDocument;
use crate::editor_tab::EditorTab;
use crate::editor_view::ReloadSnapshot;

struct PendingApply {
    document: LoadedDocument,
    snapshot: Option<ReloadSnapshot>,
    offset: usize,
    restore_editable: bool,
    restore_undo: bool,
}

impl EditorTab {
    /// Applies a loaded document without blocking the main loop.
    ///
    /// Documents up to [`CHUNKED_APPLY_MIN_BYTES`] apply synchronously
    /// (identical to the previous behavior). Larger documents are inserted
    /// in budgeted idle ticks; `cancel_io()` or a newer IO request aborts
    /// the insertion between ticks, so tab close stays instant.
    pub(super) fn apply_loaded_document_async(
        self: &Rc<Self>,
        document: LoadedDocument,
        snapshot: Option<ReloadSnapshot>,
        on_applied: Rc<dyn Fn(&Rc<Self>)>,
    ) {
        self.begin_loaded_document_state(&document);
        let restore_undo = self.text_buffer.enables_undo();
        self.text_buffer.set_enable_undo(false);
        self.text_buffer
            .set_implicit_trailing_newline(document.format.implicit_trailing_newline());

        if document.text.len() <= CHUNKED_APPLY_MIN_BYTES {
            self.text_buffer.set_text(&document.text);
            self.text_buffer.set_enable_undo(restore_undo);
            self.finish_loaded_document_presentation(&document, snapshot.as_ref());
            on_applied(self);
            return;
        }

        self.text_buffer.set_text("");
        let restore_editable = self.text_view.is_editable();
        self.text_view.set_editable(false);
        let generation = self.state.borrow().io.generation;
        let pending = Rc::new(RefCell::new(PendingApply {
            document,
            snapshot,
            offset: 0,
            restore_editable,
            restore_undo,
        }));
        let weak = Rc::downgrade(self);
        let source = glib::idle_add_local(move || {
            let Some(tab) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if tab.state.borrow().io.generation != generation {
                // A cancel or newer request owns the tab now; drop quietly.
                tab.state.borrow_mut().io.pending_apply = None;
                return glib::ControlFlow::Break;
            }
            let tick_started = Instant::now();
            loop {
                if insert_next_chunk(&tab, &pending) {
                    tab.state.borrow_mut().io.pending_apply = None;
                    finish_chunked_apply(&tab, &pending, &on_applied);
                    return glib::ControlFlow::Break;
                }
                if tick_started.elapsed() >= CHUNK_TICK_BUDGET {
                    return glib::ControlFlow::Continue;
                }
            }
        });
        self.state.borrow_mut().io.pending_apply = Some(source);
    }
}

/// Inserts the next chunk at the buffer end. Returns `true` when done.
fn insert_next_chunk(tab: &Rc<EditorTab>, pending: &Rc<RefCell<PendingApply>>) -> bool {
    let (start, end, done) = {
        let pending = pending.borrow();
        let text = &pending.document.text;
        let end = chunk_end_at_char_boundary(text, pending.offset, CHUNK_TARGET_BYTES);
        (pending.offset, end, end >= text.len())
    };
    if end > start {
        let slice_owner = pending.borrow();
        let mut iter = tab.text_buffer.end_iter();
        tab.text_buffer
            .insert(&mut iter, &slice_owner.document.text[start..end]);
    }
    pending.borrow_mut().offset = end;
    done
}

fn finish_chunked_apply(
    tab: &Rc<EditorTab>,
    pending: &Rc<RefCell<PendingApply>>,
    on_applied: &Rc<dyn Fn(&Rc<EditorTab>)>,
) {
    let pending = pending.borrow();
    tab.text_view.set_editable(pending.restore_editable);
    tab.text_buffer.set_enable_undo(pending.restore_undo);
    tab.finish_loaded_document_presentation(&pending.document, pending.snapshot.as_ref());
    on_applied(tab);
}
```

Re-entrancy rule: never hold a `state.borrow_mut()` across `text_buffer.insert(...)` — the buffer's `connect_changed` handler does `state.borrow()` (it early-returns on `suppress_changes`, which is active throughout the apply). The code above respects this.

- [ ] **Step 2: Drop pending applies on cancel and on new requests** — in `runtime.rs`:

`cancel_io` (~line 18) — add pending-apply takedown before the cancellable handling:

```rust
    pub fn cancel_io(&self) {
        let (cancellable, pending_apply) = {
            let mut state = self.state.borrow_mut();
            let pending_apply = state.io.take_pending_apply();
            (state.io.cancel_request(), pending_apply)
        };
        if let Some(source) = pending_apply {
            source.remove();
        }
        if let Some(cancellable) = cancellable {
            cancellable.cancel();
        }
        // ... rest unchanged (writability cancel, large_file cancel, review cancel)
    }
```

`start_io_request` (~line 412):

```rust
    pub(super) fn start_io_request(
        &self,
        candidate_encodings: Option<SList<sourceview5::Encoding>>,
    ) -> (u64, gio::Cancellable) {
        let (result, pending_apply) = {
            let mut state = self.state.borrow_mut();
            let pending_apply = state.io.take_pending_apply();
            (state.io.start_request(candidate_encodings), pending_apply)
        };
        if let Some(source) = pending_apply {
            source.remove();
        }
        result
    }
```

`SourceId::remove()` must only run on a still-live source; both paths above hold the only `SourceId`, and the tick clears `pending_apply` before its own `Break`, so a stored id always refers to a live source. The generation-mismatch branch inside the tick sets `pending_apply = None` *without* `.remove()` (the source is removed by returning `Break`).

- [ ] **Step 3: Switch the three `open.rs` call sites.** Pattern — everything that previously ran after `apply_loaded_document` moves into the `on_applied` closure:

Load path (success arm of `load_file_with_candidates`, ~line 224):

```rust
                        Ok(document) => {
                            if tab.dirty_generation() != start_dirty_generation {
                                tab.set_loading(false);
                                tab.sync_presentation();
                                callback(Err(AppError::Cancelled));
                                return;
                            }
                            let monitored_file = gio::File::for_path(&document.path);
                            let uri = document.uri.clone();
                            let callback = callback.clone();
                            tab.apply_loaded_document_async(
                                document,
                                None,
                                Rc::new(move |tab| {
                                    tab.swap_monitor(&monitored_file);
                                    tab.refresh_language_for_file(&monitored_file);
                                    tab.set_loading(false);
                                    tab.sync_presentation();
                                    tab.grab_focus();
                                    callback(Ok(uri.clone()));
                                }),
                            );
                        }
```

Reload path (success arm in `reload_from_disk`, ~line 97) — `snapshot` moves into the call (change the earlier `let snapshot = ReloadSnapshot::capture(...)` usage accordingly; the closure for `apply` no longer needs it):

```rust
                            Ok(document) => {
                                if !tab.can_apply_reload(cause, &expected_uri, &should_apply) {
                                    tab.finish_reload(false);
                                    callback(Ok(ReloadResult::Deferred));
                                    return;
                                }
                                let monitored_file = gio::File::for_path(&document.path);
                                let callback = callback.clone();
                                tab.apply_loaded_document_async(
                                    document,
                                    Some(snapshot.clone()),
                                    Rc::new(move |tab| {
                                        tab.swap_monitor(&monitored_file);
                                        tab.refresh_language_for_file(&monitored_file);
                                        tab.finish_reload(true);
                                        callback(Ok(ReloadResult::Applied));
                                    }),
                                );
                            }
```

If `ReloadSnapshot` does not implement `Clone`, derive it (check `app/src/editor_view.rs`); it is a small cursor/scroll capture.

Reopen-with-encoding path (success arm in `reopen_with_encoding`, ~line 314): same pattern — guard checks stay before the apply call; `swap_monitor` + `refresh_language_for_file` + `set_loading(false)` + `sync_presentation()` + `callback(Ok(()))` move into `on_applied`.

After all three sites compile, grep `apply_loaded_document\b` — if only the thin sync wrapper in `runtime.rs` remains unreferenced, delete it (`replace_buffer_text` stays if still referenced, otherwise delete it too).

- [ ] **Step 4: Close/save semantics during loading.**

`app/src/editor_tab.rs` `is_dirty` (~line 268) — a buffer being filled by the app is not user-dirty; without this, mid-apply close would show a bogus "unsaved changes" dialog:

```rust
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        if !self.is_document() {
            return false;
        }
        let state = self.state.borrow();
        if state.io.loading {
            return false;
        }
        state.is_dirty(self.text_buffer.is_modified())
    }
```

`app/src/editor_tab/save.rs` `request_save` (~line 50), right after the `is_document` guard — a partially applied buffer must never be saved:

```rust
        if self.is_loading() {
            callback(SaveResult::CancelledByUser);
            return;
        }
```

- [ ] **Step 5: Build and run the GTK suite**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings` then `cargo test --lib gtk_surfaces_and_editor_flow_work -- --nocapture`
Expected: PASS. Pay attention to existing reload/encoding tests — they now exercise the async path (small fixtures take the synchronous fast path, so existing timing-sensitive tests should be unaffected).

- [ ] **Step 6: Commit**

```bash
git add app/src/editor_tab/apply.rs app/src/editor_tab/runtime.rs app/src/editor_tab/open.rs app/src/editor_tab.rs app/src/editor_tab/save.rs
git commit -m "Apply loaded documents in cancellable budgeted chunks"
```

---

### Task 5: GTK tests for chunked apply

**Files:**
- Modify: `app/src/window/testing.rs` (add helpers near `tab_count_for_tests`)
- Modify: `app/src/gtk_tests_boundaries.rs` (extend `exercise_boundary_smokes`)

- [ ] **Step 1: Add window test helpers** in `window/testing.rs`:

```rust
    pub(crate) fn close_selected_page_for_tests(&self) -> bool {
        let Some(tab) = self.workspace.selected_tab() else {
            return false;
        };
        let Some(page) = tab.page() else {
            return false;
        };
        self.workspace.tab_view.close_page(&page);
        true
    }

    pub(crate) fn selected_char_count_for_tests(&self) -> i32 {
        self.workspace
            .selected_tab()
            .map_or(0, |tab| tab.text_buffer().char_count())
    }
```

(`selected_loading_for_tests` already exists at ~line 323 — do not duplicate it.)

- [ ] **Step 2: Add the failing GTK tests** — in `gtk_tests_boundaries.rs`, add two functions and call them at the end of `exercise_boundary_smokes` (line 18). Use the file's existing imports (`build_window`, `spin_until`, `drain_events`, `write_temp_file` come from `crate::gtk_tests`; add a local repeat helper if none exists in this file):

```rust
fn repeated_contents(seed: &[u8], target_len: usize) -> Vec<u8> {
    let mut contents = Vec::with_capacity(target_len + seed.len());
    while contents.len() < target_len {
        contents.extend_from_slice(seed);
    }
    contents
}

fn exercise_chunked_apply_completes_with_full_content(test_app: &adw::Application) {
    let window = crate::gtk_tests::build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("keep this tab");
    let contents = repeated_contents(b"chunked-apply line\n", 3 * 1024 * 1024);
    let expected_chars = i32::try_from(contents.len()).unwrap_or(i32::MAX);
    let path = crate::gtk_tests::write_temp_file("riteed-chunked-full.txt", &contents);
    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    crate::gtk_tests::spin_until("chunked apply fills the buffer", || {
        window.tab_count_for_tests() == 2
            && !window.selected_loading_for_tests()
            && window.selected_char_count_for_tests() == expected_chars
    });
    assert!(!window.selected_dirty_for_tests());
    let text = window.selected_text_for_tests();
    assert!(text.starts_with("chunked-apply line"));
    assert!(text.ends_with("chunked-apply line\n") || text.ends_with("chunked-apply line"));
    let _removed = std::fs::remove_file(path);
}

fn exercise_chunked_apply_close_during_apply(test_app: &adw::Application) {
    let window = crate::gtk_tests::build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    window.set_selected_text_for_tests("keep this tab");
    let contents = repeated_contents(b"chunked-close line\n", 8 * 1024 * 1024);
    let path = crate::gtk_tests::write_temp_file("riteed-chunked-close.txt", &contents);
    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    crate::gtk_tests::spin_until("chunked apply is in progress", || {
        window.tab_count_for_tests() == 2
            && window.selected_loading_for_tests()
            && window.selected_char_count_for_tests() > 0
    });
    // Close mid-apply: must detach without prompting and without finishing the fill.
    assert!(window.close_selected_page_for_tests());
    crate::gtk_tests::spin_until("loading tab closes promptly", || {
        window.tab_count_for_tests() == 1
    });
    crate::gtk_tests::drain_events(8);
    assert_eq!(window.selected_text_for_tests(), "keep this tab");
    let _removed = std::fs::remove_file(path);
}
```

Notes for the executor: the “in progress” predicate (loading **and** char_count > 0) only holds while the chunked fill is running — if the test ever flakes because apply finishes too fast on CI, raise the fixture to 16 MiB rather than asserting on wall-clock timing. `expected_chars` equals byte length because the seed is pure ASCII; the trailing-newline assertion is tolerant because a single trailing `\n` becomes implicit (`from_disk_text`). If `gtk_tests_boundaries.rs` already imports `adw`/`gio`/`OpenSource`, reuse those imports; otherwise mirror the import style of `gtk_tests_tabs.rs`. Check the test-file line count stays ≤ 800 (`wc -l app/src/gtk_tests_boundaries.rs`).

- [ ] **Step 3: Run the suite**

Run: `cargo test --lib gtk_surfaces_and_editor_flow_work -- --nocapture`
Expected: PASS, including the two new exercises.

- [ ] **Step 4: Commit**

```bash
git add app/src/window/testing.rs app/src/gtk_tests_boundaries.rs
git commit -m "Cover chunked apply integrity and mid-apply tab close"
```

---

### Task 6: Long-line capability gate in `document_limits`

**Files:**
- Modify: `app/src/document_limits.rs` (constants ~line 5, tests module ~line 300)

- [ ] **Step 1: Write the failing tests** — add to the tests module:

```rust
    #[test]
    fn long_line_gate_accepts_normal_lines() {
        assert!(text_supports_editor_line_lengths("short\nlines\nonly\n"));
        assert!(text_supports_editor_line_lengths(""));
    }

    #[test]
    fn long_line_gate_rejects_oversized_line() {
        let long_line = "x".repeat(EDITOR_MAX_LINE_BYTES + 1);
        assert!(!text_supports_editor_line_lengths(&long_line));
        let mixed = format!("ok\n{long_line}\nok\n");
        assert!(!text_supports_editor_line_lengths(&mixed));
    }

    #[test]
    fn long_line_gate_boundary_is_inclusive() {
        let at_cap = "x".repeat(EDITOR_MAX_LINE_BYTES);
        assert!(text_supports_editor_line_lengths(&at_cap));
    }
```

Add `EDITOR_MAX_LINE_BYTES` and `text_supports_editor_line_lengths` to the `use super::{...}` list in the tests module.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib long_line_gate`
Expected: FAIL — unresolved names.

- [ ] **Step 3: Implement** — next to the other editor caps (~line 10):

```rust
/// Longest line (in bytes) the editor surface accepts. Pango must lay out a
/// line as one unit, so a single multi-megabyte line stalls the main thread
/// for seconds regardless of total file size; such files open in the
/// read-only large-file viewer instead.
pub(crate) const EDITOR_MAX_LINE_BYTES: usize = 64 * 1024;

#[must_use]
pub(crate) fn text_supports_editor_line_lengths(text: &str) -> bool {
    text.as_bytes()
        .split(|byte| *byte == b'\n')
        .all(|line| line.len() <= EDITOR_MAX_LINE_BYTES)
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib long_line_gate` — PASS. Check `wc -l app/src/document_limits.rs` ≤ 600.

- [ ] **Step 5: Commit**

```bash
git add app/src/document_limits.rs
git commit -m "Add editor long-line capability gate"
```

---

### Task 7: `LoadFailure::LineTooLong` + `AppError::LineTooLong`

**Files:**
- Modify: `app/src/editor_io.rs` (enum ~line 61, completion ~line 197, tests)
- Modify: `app/src/error.rs` (variant ~line 14, title ~line 32, body ~line 58)

- [ ] **Step 1: Write the failing unit test** — in `editor_io.rs` tests:

```rust
    #[test]
    fn long_line_failures_carry_path_and_size() {
        let failure = LoadFailure::LineTooLong {
            path: std::path::PathBuf::from("/tmp/example.txt"),
            size: 7,
        };
        assert!(matches!(
            failure,
            LoadFailure::LineTooLong { size: 7, .. }
        ));
    }
```

(Compile-level test; the real coverage is the GTK routing test in Task 8.)

- [ ] **Step 2: Implement.** `editor_io.rs` enum:

```rust
#[derive(Clone, Debug)]
pub enum LoadFailure {
    DecodeFailed(PathBuf),
    TooBig(PathBuf),
    LineTooLong { path: PathBuf, size: u64 },
    Failed(AppError),
}
```

In `TextLoadRequest::start`'s `Ok(())` arm, after the hard-cap check (~line 206):

```rust
                    if !crate::document_limits::text_supports_editor_line_lengths(
                        &loaded_format.text,
                    ) {
                        callback(Err(LoadFailure::LineTooLong {
                            path: path.clone(),
                            size: disk_size.unwrap_or_else(|| {
                                crate::large_file::usize_to_u64(loaded_format.text.len())
                            }),
                        }));
                        return;
                    }
```

`error.rs`: add `LineTooLong(PathBuf)` to `AppError`; title arm joins the existing open-failure group (`gettext("Unable to Open the File")`, ~line 32); body arm:

```rust
            Self::LineTooLong(_) => {
                gettext("The file contains lines that are too long to edit safely.")
            }
```

Match the surrounding arms' exact structure (some include the path in the body — mirror `FileTooBig`'s arm shape). The compiler will now flag every non-exhaustive `LoadFailure` match (`open.rs` ×3, `map_load_failure_to_app_error`) — Task 8 fills those in; to keep this commit green, add the `map_load_failure_to_app_error` arm here:

```rust
        LoadFailure::LineTooLong { path, .. } => AppError::LineTooLong(path),
```

and temporary identical arms in `open.rs` matches that Task 8 immediately replaces is **not** allowed (no dead intermediate states) — instead do Task 7 and Task 8 as one commit if the build cannot be kept green otherwise. Prefer: implement Task 7 + the three `open.rs` arms (Task 8 Step 1) together, commit once.

- [ ] **Step 3: Run** `cargo check --workspace --all-targets --all-features` — expect missing-arm errors listing exactly the `open.rs` sites → proceed straight into Task 8 Step 1, then test + commit there.

---

### Task 8: Route long-line files to the read-only viewer

**Files:**
- Modify: `app/src/editor_tab/open.rs` (3 match sites + import `OpenDecision`)
- Modify: `app/src/gtk_tests_boundaries.rs` (routing test)

- [ ] **Step 1: Handle the new failure in all three `open.rs` matches.**

Add to the imports: `use crate::document_limits::{OpenDecision, OpenFileSupport};` (extend the existing line 9 import).

In `load_file_with_candidates` (the initial-open path) — long lines reroute to the viewer; `edit_allowed: false` is deliberate, otherwise "Edit Anyway" would re-enter the editor path and loop:

```rust
                        Err(LoadFailure::LineTooLong { path: _, size }) => {
                            tab.set_loading(false);
                            tab.sync_presentation();
                            let thresholds = tab.settings.large_file_thresholds();
                            let tier = crate::document_limits::tier_for_size_with_thresholds(
                                size,
                                &thresholds,
                            );
                            tab.open_viewer_for_large_file(
                                &parent,
                                &opened_file,
                                size,
                                OpenDecision::Viewer {
                                    tier,
                                    edit_allowed: false,
                                },
                                callback.clone(),
                            );
                        }
```

(`tier_for_size_with_thresholds` is `pub(crate)` in `document_limits.rs`; verify and widen from private if needed.)

In `reload_from_disk` (a reload cannot switch surfaces mid-flight; fail like `TooBig`):

```rust
                            Err(LoadFailure::LineTooLong { path, .. }) => {
                                tab.finish_reload(false);
                                callback(Err(AppError::LineTooLong(path)));
                            }
```

In `reopen_with_encoding`:

```rust
                            Err(LoadFailure::LineTooLong { path, .. }) => {
                                tab.set_loading(false);
                                tab.sync_presentation();
                                callback(Err(AppError::LineTooLong(path)));
                            }
```

- [ ] **Step 2: Add the GTK routing test** — in `gtk_tests_boundaries.rs`, plus a call from `exercise_boundary_smokes`:

```rust
fn exercise_long_line_file_routes_to_viewer(test_app: &adw::Application) {
    let window = crate::gtk_tests::build_window(test_app);
    assert!(window.is_some());
    let Some(window) = window else {
        return;
    };
    window.ensure_default_tab();
    let mut contents = vec![b'y'; 128 * 1024];
    contents.push(b'\n');
    let path = crate::gtk_tests::write_temp_file("riteed-long-line.txt", &contents);
    window.request_open_files(vec![gio::File::for_path(&path)], OpenSource::AppOpen);
    crate::gtk_tests::spin_until("long-line file opens in the viewer", || {
        window.selected_large_file_surface_for_tests() == Some(String::from("viewer"))
    });
    let _removed = std::fs::remove_file(path);
}
```

(Check the exact return type of `selected_large_file_surface_for_tests` in `window/testing_large_file.rs` — `gtk_tests_tabs.rs:131` compares it to `Some("viewer")`, so mirror that comparison style.)

- [ ] **Step 3: Run everything**

Run: `cargo test --lib long_line` then `cargo test --lib gtk_surfaces_and_editor_flow_work -- --nocapture`
Expected: PASS.

- [ ] **Step 4: Commit (combined Task 7+8)**

```bash
git add app/src/editor_io.rs app/src/error.rs app/src/editor_tab/open.rs app/src/document_limits.rs app/src/gtk_tests_boundaries.rs
git commit -m "Route files with oversized lines to the read-only viewer"
```

---

### Task 9: Localization for the new error body

**Files:**
- Modify: `app/po/io.github.cadric.Riteed.pot`
- Modify: `app/po/da.po`

- [ ] **Step 1: Add the msgid to the POT** following the file's existing entry format (source-reference comment with the real `error.rs` line number):

```
#: src/error.rs:NN
msgid "The file contains lines that are too long to edit safely."
msgstr ""
```

- [ ] **Step 2: Add the Danish translation to `da.po`:**

```
#: src/error.rs:NN
msgid "The file contains lines that are too long to edit safely."
msgstr "Filen indeholder linjer, der er for lange til at kunne redigeres sikkert."
```

- [ ] **Step 3: Validate**

Run: `msgfmt --check-format --check-header -o /dev/null app/po/da.po`
Expected: exit 0, no output.

- [ ] **Step 4: Commit**

```bash
git add app/po/io.github.cadric.Riteed.pot app/po/da.po
git commit -m "Localize long-line open failure copy"
```

---

### Task 10: Validation gates, review artifacts, changelog

**Files:**
- Modify: `CHANGELOG.md` (Unreleased → Added/Fixed)
- Modify (only if the validator demands it): `app/build-aux/validation/runtime-review*.json`, `parser-boundaries.v1.json`, `i18n-review.v1.json`

- [ ] **Step 1: Run the full gate set** (from repo root unless noted):

```bash
cd app && cargo fmt --all --check
cd app && cargo clippy --workspace --all-targets --all-features -- -D warnings
cd app && cargo test --workspace --all-targets --all-features
python3 -m tools.policy_check --root app --strict
python3 -m tools.coverage_check --root app
```

Expected: all green. Treat every warning as a failure (AGENTS Guardrails).

- [ ] **Step 2: If `policy_check` reports review findings**, fix them per `policy/README.md`:
  - *Unmatched scanner hit* (e.g., a new reviewable pattern on a line you added): add an anchored entry `{path, line, match, kind, ...}` to the matching `app/build-aux/validation/*.json` artifact, copying the schema of neighboring entries.
  - *Stale anchor* (a `match` no longer on its `line` because your edit shifted lines in `editor_io.rs`/`open.rs`/`runtime.rs`): update the `line` (and `match` if the text changed) in the existing entry — V15 entries anchor heavily into these files.
  - For the `document_file_load` parser boundary in `parser-boundaries.v1.json`: extend `real_input_shape` to mention the post-decode line-length gate, and refresh `last_reviewed` to today's date. Re-run `policy_check` until clean. Never relax a rule.

- [ ] **Step 3: Changelog** — under `## Unreleased` in `CHANGELOG.md` (Keep a Changelog style, mirror existing bullet voice):

```markdown
### Fixed
- Large files (up to the 25 MiB editor cap) now stream into the editor in
  budgeted main-loop chunks, so closing a tab while a document is loading
  takes effect immediately instead of waiting several seconds.

### Changed
- Files containing lines longer than 64 KiB now open in the read-only
  large-file viewer regardless of file size, because single oversized lines
  stall GTK text layout for seconds.
```

- [ ] **Step 4: Final verification then commit**

Re-run the Step 1 gate set; everything green, then:

```bash
git add CHANGELOG.md app/build-aux/validation
git commit -m "Record non-blocking apply and long-line routing in changelog and review artifacts"
```

---

## Self-review checklist (for the executing agent, after Task 10)

1. Open a ~20 MiB file manually (`app/scripts/dev-run`), click the tab close button while the spinner shows → tab closes instantly, no dialog.
2. Open four ~20 MiB files, close them all mid-load → window stays responsive throughout.
3. Open a minified single-line JSON > 64 KiB → opens in the read-only viewer.
4. Reload-from-disk and reopen-with-encoding on a > 1 MiB file still restore cursor/scroll (snapshot path) and end with `is_dirty() == false`.
5. Undo after a chunked open does nothing (no undo step from the fill), typing afterwards undoes normally.
6. `wc -l` every touched production file ≤ 600, test files ≤ 800.
