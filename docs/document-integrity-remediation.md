---
created: 2026-09-05
updated: 2026-09-05
status: current
priority: high
type: implementation-record
---

# Document integrity remediation

Scope: F-01, F-04 and F-06 from the September 5 audit, based on
`1c7b81c7bc462acf29ca92f8394b1ecff5603049` plus the existing local policy
changes. The implementation preserves editing while asynchronous I/O waits.
It cancels an obsolete load or close instead of discarding accepted edits.

## Behavior and ownership

| Finding | Corrected boundary | Result |
| --- | --- | --- |
| F-01 | `workspace_close::on_close_page`, `advance_close_flow` | Workspace borrows end before reentrant GTK callbacks or advancing the window-close queue. Saving one or several documents during window close completes without the observed borrow panic. |
| F-04 | `editor_tab/read_guard.rs`, `editor_tab/open.rs`, `workspace_open.rs` | A read captures document generation, URI, page and attachment before metadata work or after explicit discard consent. Only a still-current I/O request whose document guard matches may apply loaded text. |
| F-06 | `workspace_close::handle_close_save_result`, `close_flow::CloseCoordinator` | The save callback must belong to the current close operation and attached tab. It may advance only when that tab is clean and its URI matches the completed save. |

The final window-close check revisits all attached tabs. Earlier save results
do not authorize discarding later edits. Explicit discard consent records the
tab, URI and dirty generation; it does not authorize discarding a newer
revision. Detaching a queued target cancels the outstanding window-close flow.
Old dialog and save callbacks cannot advance a replacement close operation.
Rejected or cancelled tab-close requests are completed with GTK's rejection
result so the user can try again.

Load conflicts preserve text, dirty state, format, document identity and real
undo/redo history. An allocated tab is retained after an edit followed by undo,
even when the buffer has become empty again. Manual operations show one
localized cancellation notice. An automatic reload interrupted by editing
keeps the external-change banner and requires a conscious action before trying
again; repeated file notifications do not cause another automatic reload or
confirmation dialog. The existing brief input lock during chunked buffer
application remains in place.

Saving can successfully write an earlier snapshot while newer edits remain in
the buffer. The save notice now states that distinction. Save-and-close cancels
closing in this case, including Save As, and leaves the newer text editable.
Autosave can subsequently save those edits; it does not silently resume the
cancelled close operation.

Pending opens reserve an otherwise reusable untitled tab. Registry cleanup
matches URI, tab and request token; lookup only returns attached pages. This
implements the overlapping acquisition/ownership changes described in P2
Task 3 (RIT-GEN-011) and P3 Task 2 (RIT-GEN-022). Other P2/P3 tasks remain
separate work. No claim is made that either complete plan has been executed.

A related pre-existing lifecycle defect was also reproduced during review:
a save superseding an active reload could fail and leave the reload busy flag
set forever. I/O replacement now assigns reload ownership to the new request;
stale callbacks still cannot clear a newer request's state. The regression
requires a real failed write followed by a successful explicit reload.

## Regression protection

The new cases run inside the existing single-thread GTK test
`gtk_tests::gtk_surfaces_and_editor_flow_work`, with individual flow markers:

- `gtk_tests_document_close.rs`: window save with one/multiple dirty tabs;
  newer edits during tab, window and other-tabs close with autosave on/off;
  final window recheck after an earlier save, discard or initially clean tab;
  cancellation, write failure and retry; Save As; a rejected sibling close;
  actual undo/redo after the close conflict.
- `gtk_tests_document_close_lifecycle.rs`: late dialog/save callbacks against
  a replacement coordinator and actual GTK transfer of a current/future
  member of the window-close queue.
- `gtk_tests_document_reads.rs` and its fixture: initial metadata reads,
  manual and automatic reload, encoding selection, decode retry, edit/undo
  races, allocated-tab redo, concurrent distinct opens, close/reopen, transfer,
  successful reload and failed-save/reload recovery. Real Gio and GtkSourceView
  I/O is used against synthetic files.
- Find in Files and Source Control unit tests verify the error-routing
  predicates for the typed load conflict. Source review verifies the separate
  Source Control busy-state cleanup; these tests do not measure that cleanup.
- `gtk_tests_pending_open.rs` deterministically calls the actual registry
  cleanup with an old request's identity while a successor is pending. It
  checks detached lookup, same-tab token ownership and attached pending-target
  dedupe.
  This complements the real close/reopen I/O test; it does not force the order
  of full Gio completions. Restoring URI-only cleanup in the isolated copy
  provides a separate regression-sensitivity check.

P3 follow-up holds the actual workspace open-completion callbacks in a
test-only, URI-scoped queue. It closes A, opens attached B for the same file,
delivers A's cancelled completion while B remains registered, and then
delivers B's successful completion and checks its text and URI. This is
characterization of the existing URI/tab/request-token implementation, not a
new runtime fix or a new failing-first claim.

Only chooser selections and selected lifecycle boundaries are injected by
tests. The save dialog continuation normalizes the selected path and performs
the actual snapshot write. Test setup failures are assertions. All GTK runs
use `G_DEBUG=fatal-criticals`; the validation environment supplies a real
private Xvfb display rather than accepting a skipped display-dependent test.

## Validation and evidence

All final gates completed with exit code 0 against the same application
sources and validation configuration:

| Command and working directory | Observed result | Elapsed |
| --- | --- | --- |
| Root: `python3 -m tools.policy_check --root app --strict` | Format, check, Clippy with `-D warnings`, metadata/i18n checks and 413 tests passed (411 library, 1 stress-binary unit, 1 UI smoke). | 111.264 s |
| `app/`: `cargo test --workspace --all-targets --no-default-features --locked` | 412 tests passed; the stress-only binary is excluded. `default = []`, so this is also the default feature configuration. | 87.597 s |
| Root: `python3 -m tools.coverage_check --root app` | 82.8% line coverage, above the unchanged 80.0% gate. 414 tests passed, including the coverage-only main test. | 128.406 s |
| `app/`: `cargo build --release --no-default-features --locked --offline` | Optimized native release build passed. | 82.096 s |

No tests were ignored or filtered in these final full-suite runs. The GTK
flow start/end markers confirm execution of the new cases. The metadata-only
checks are not claims of a working host document portal or installed Flatpak.

The implementation was built and tested in an isolated source copy with
Bubblewrap, no network, an empty home, memory GSettings, private D-Bus and
Xvfb (24-bit). It used openSUSE Tumbleweed 20260901, Rust/Cargo 1.95.0,
cargo-llvm-cov 0.8.5, native GTK 4.22.4, libadwaita 1.9.3, GtkSourceView 5.20.0
and Gio 2.88.3, two build jobs, offline dependency caches and synthetic data.
The original workspace and its existing local edits were hashed before work;
source promotion checks those hashes again. Missing DRI3 acceleration, FUSE
document-portal mounting and PipeWire in the private environment produced
service warnings; the app tests still ran with fatal GTK/GLib criticals.

The evidence directory records commands, working directories, environment,
exit codes, elapsed times and relevant logs. Initial regressions observed the
F-01 SIGABRT, F-04 overwritten edit, F-06 detached dirty tab, stale queued-tab
close and stuck reload after a failed save. Subsequent successful runs are
identified separately; a log filename alone is not a pass result.

A scripted GTK visual check of the isolated close-fix copy captured the
save/close conflict with retained newer text, dirty indicators and one readable
cancellation toast. This is not
a manual Wayland, accessibility or installed-Flatpak acceptance test. The
initial validation did not install a Flatpak; the later user-requested build
is recorded below. No release, dependency update or policy relaxation is part
of this change. Large-file parser fuzzing and full Git stress campaigns were
not repeated for this document-lifecycle change.

Runtime review anchors, parser-boundary coverage evidence, the POT template,
Danish translations, `CHANGELOG.md` and continuity are updated with the code.

Local command logs, source hashes, regression evidence, review notes and the
screenshot are retained in the git-ignored directory
`docs/superpowers/audits/2026-09-05-document-integrity-remediation/`.
The earlier audit records remain separate and unchanged.

## Local Flatpak follow-up

At the user's request, `scripts/local-flatpak-build` subsequently completed
from `main` with the uncommitted fixes and existing local policy work. The two
branches listed by `git branch --no-merged main` were checked with `git cherry`:
both contained patch-equivalent changes already in `main`. No missing feature
branch had to be integrated.

The installed user Flatpak is now `io.github.cadric.Riteed` 0.3.8,
`x86_64/master`, on GNOME Platform 50. Its Flatpak commit is
`a0521eaf5eb4da2e300baa50985e9e7598fba0effa97e1668039b0e709657448`.
The completed build and installed `/app/bin/riteed` have identical SHA-256
`a7e2ef8078cd84342d5fd6a8e3efbd9e619088a241697ff13919f09c9327f714`.
Both new document-integrity notice strings are present in that binary, and
the Locale extension was updated in the same build.

The 33 task-related app input files still match the tested source hashes.
Flatpak-builder had already removed its temporary module directory when the
additional per-file copy comparison was attempted; no successful comparison
of that deleted directory is claimed. The build log and installed-binary
comparison are retained as `flatpak-local-build.log` and
`flatpak-install-verification.json` in the local evidence directory.

The package was prepared before committing for the user's desktop acceptance
test. Manual Flatpak/portal/Wayland behavior was not verified by this build run.
