# Audit P3 cleanup evidence

This pass starts from integrated local main `6aaffc3e`. Findings are closed
only after their implementation and validation; the overall pass is ongoing.

## RIT-GEN-020: whitespace-ignore line identity (Task 1)

Whitespace-ignore diffing now normalizes one element for each token produced by
the existing `line_slices` tokenizer and passes those elements directly to
`similar::TextDiff::diff_slices`. Each normalized element retains its original
LF, CRLF, lone-CR or absent ending. The row model and presentation continue to
use the original tokens, so no sentinel or synthetic newline changes displayed
text or file semantics.

The pre-fix RED compared `"a\n   "` with `"a\nb"`: the unterminated
whitespace-only reference token had no row. The reverse input lost the current
token, and the same omission occurred against empty input and a trailing
newline. The focused GREEN covers those cases, original presentation text,
LF/CRLF/lone CR, interior blanks, Unicode whitespace, trailing-newline
boundaries, ignore=false, and property-generated total, unique, in-range
round trips for every original token. Existing hidden-whitespace, inline,
hunk and lone-CR regressions remain in the full suite.

The `diff_compute` fuzz adapter runs both default and whitespace-ignore
computations and asserts the same total/unique row-map invariant. Its committed
midpoint-split seed contains an unterminated whitespace-only reference tail;
explicit one-input replay executes that target path. The parser-boundary
registry now records the fuzz assertion, deterministic corpus seed and property
evidence. Task 11 subsequently completed the token-reuse cleanup documented
below.

Final strict validation passed 473 library tests plus the stress-binary unit
test and UI smoke. Coverage passed at 84.6% against the unchanged 80% minimum.
The fixed-seed `cargo +nightly fuzz run` replay passed with one executed input.
The first strict attempt correctly rejected shifted runtime-review anchors;
their existing ownership evidence was re-anchored without semantic changes.
The corrected gate then found a constant-assertion Clippy violation in the new
test helper; the helper now asserts the actual optional mapping without a lint
exception. Environment-only Mesa/portal/bus diagnostics remained non-fatal in
the isolated Xvfb/D-Bus run. No Flatpak build or desktop test was performed.

## RIT-GEN-022: pending-open ownership characterization

Commit `ce26e22` holds actual workspace open completions for a close/reopen
sequence. A's cancelled completion cannot clear B's registration; B remains
attached and publishes the expected text and URI. The runtime identity fix
already existed, so this is characterization rather than a new RED/fix.
Strict validation passed 447 tests; line coverage was 83.8%.

## RIT-GEN-021: bounded worktree reads (Task 12A)

Git review worktree versions use the shared `GIT_BLOB_BYTE_LIMIT` of
1,000,001 accepted bytes. The Gio reader consumes at most the checked limit
plus one sentinel byte, with requests capped at 64 KiB. Short nonzero reads
continue; acceptance requires EOF within the limit. No metadata-size query
controls this boundary. The owned stream closes asynchronously, without the
caller cancellation token, before success/error/cancellation is delivered.
The bounded byte vector moves into UTF-8 decoding; NUL and invalid UTF-8 retain
their existing errors. An exhausted aggregate prevents the next queued read.

The 4 MiB aggregate bounds retained decoded review inputs, not process RSS.
Existing reference and final input clones remain bounded transient copies;
a read may temporarily retain Gio bytes and its bounded copied chunk.
Task 12B below bounds the remaining Git subprocess pipes. Together the two
independently tested changes close RIT-GEN-021.

The initial real-loader RED accepted six bytes with an injected five-byte
limit. Final tests prove only six bytes are consumed from a 128-byte fixture,
alongside empty/exact-limit, short-read, cancellation, close, binary, UTF-8,
and read-error cases. Aggregate exhaustion is tested through the queue helper.
A real GTK review closes A during its first chunk and reopens B; both streams
close and B publishes the complete expected text. Test setup failures and
missing callbacks fail explicitly, with private-context deadline sources.

Strict validation passed 456 tests (454 library, one stress-binary unit and
one UI smoke); line coverage was 84.6%, above the unchanged 80% minimum.
Independent review found no outstanding issues after two test-harness fixes.
Logs are under `/tmp/riteed-p3-validation-hGkt8u/`, with prefixes
`task12a-final-strict-r2` and `task12a-final-coverage`.

## RIT-GEN-021: bounded Git pipes (Task 12B)

The shared Git runner no longer uses `communicate_async`. It starts stdin,
stdout and stderr concurrently: stdin is written and closed, both output
streams loop across short reads, and the direct child is awaited separately.
The wait and all stream closes are uncancelled. Caller cancellation is only a
signal to the supervisor; a private cleanup token can stop pending reads or
writes after cleanup is requested and the direct child has actually reaped.
This also terminates a deadline when a descendant inherited the pipe file
descriptors. A normal post-reap drain is not cancelled.

Stdout retains at most its configured cap plus one sentinel byte; stderr uses
the existing 64 KiB cap plus one sentinel. Once a mutation overflows, later
chunks use fixed requests no larger than 64 KiB and are discarded while the
live writer remains supervised. The existing stdout profiles remain unchanged:
1,000,001 accepted bytes for Git review blobs and 4 MiB for status. Exact-cap
output remains accepted. First terminal reason wins, and delivery occurs once
only after every owned pipe closes and the direct child is reaped.

The test-only peak counter is a conservative logical-byte upper bound. It adds
both retained accumulators and both outstanding read request budgets, including
the current Gio chunk while it overlaps an accumulator copy. It does not
measure `Vec` capacity, allocator overhead or process RSS. The general
two-stream bound is `stdout cap + 1 + stderr cap + 1 + 2 * 64 KiB`; owned input,
Gio objects and allocator capacity remain bounded transients outside that
logical metric. The original full-buffer RED retained 262,144 output bytes for
a 32-byte cap, exceeding even the corrected 131,105-byte conservative test
bound.

Focused coverage includes blob/status/stderr overflow, exact-cap success,
stdout/stderr output before a 256 KiB stdin read, mutation overflow completing
naturally during held grace, already-cancelled input, first-reason races, both
I/O/reap completion orders, and inherited descendant pipe cleanup. The
descendant timeout was a WIP P2 contract-preservation regression found RED
while replacing communication ownership, not a released defect.

Final isolated validation passed `git-status-stress`, the strict app gate with
465 library tests plus the stress-binary unit test and UI smoke, and coverage
at 84.7% (minimum 80%). Independent rereview found no outstanding issue after
the descendant-fixture teardown was made fail-safe. Logs are under
`/tmp/riteed-p3-validation-hGkt8u/` as
`task12b-final-git-status-stress-r3.log`, `task12b-final-strict-r4.log`, and
`task12b-final-coverage-r2.log`.

## RIT-GEN-028: one release commit identity (Task 7)

Manual release preflight resolves the requested tag once as a peeled, exact
40-character commit SHA. Cargo and AppStream metadata are read from that Git
object, the required-check collector queries that SHA, and rollback records the
same candidate commit while retaining the validated tag as human-readable
provenance. The preflight exports exactly one version, release tag and commit;
the build job consumes the commit output in its sole checkout and immediately
checks actual `HEAD` equality before any step, job or workflow scope can expose
the signing secret.

The offline release checker owns this identity chain structurally. It rejects
rebindings, duplicate or incorrect output exports and job mappings, worktree
metadata reads, conditional or alternate checkouts, conditional/custom-shell
or fallback HEAD checks, and signing-secret exposure through environment,
commands or action inputs before the verifier. Exact SHA and SemVer guard bodies
must belong to their active shell control blocks; copied text in heredocs,
functions or substitutions cannot satisfy the guard.

Tests execute the extracted workflow commands in temporary Git repositories.
They prove bad tagged metadata cannot be masked by a good worktree, good tagged
metadata is not rejected by a bad worktree, annotated tags are peeled, moving a
tag after preflight cannot change the SHA checkout, and the extracted HEAD
verifier accepts the selected commit but rejects a deliberate mismatch. The
focused release matrix and final 287-test tooling suite passed with one
intentional live-token skip. The 40 policy unit tests and strict policy-pack
gate passed. Final app strict passed 465 library tests plus the stress unit and
UI smoke; coverage passed at 84.7% against the unchanged 80% minimum.
Independent rereview found no outstanding issue after the active-owner guard
checks were made location-bound. Logs use the `task7-final-*` prefix under
`/tmp/riteed-p3-validation-hGkt8u/`. No live tag, release, secret or GitHub
setting was changed.

## RIT-GEN-038: truthful governance evidence (Task 8)

The local workflow candidate separates the PR-required, tokenless
`governance-static` check from protected-main `governance-live`. Static code
checks out the exact event SHA, proves `HEAD`, and runs only the offline release
contract. Live code is admitted only for main push, schedule and manual runs,
uses the main-only governance environment, proves event/ref/repository/HEAD,
and exposes its environment secret only to one decisive live-check step.

Publish preflight collects complete check-run, workflow-run and job evidence
from policy-derived same-origin GitHub API endpoints. The decision requires the
newest matching live check, exact candidate and job SHA, exact repository/head
repository, policy-owned workflow and event, exact check/job URL mapping, and
one completed-success decisive step. Aggregate workflow failure does not erase
a valid job result, while skipped/neutral/missing steps, wrong producers,
newer failures, malformed IDs, incomplete pagination and foreign URLs fail
closed. Live governance also checks the exact main-only environment policy and
proves the credential name is absent at repository scope and present once at
environment scope.

### Remote closure, 2026-09-06

The owner-approved activation moved the credential to the main-only
`ruleset-governance-live` environment, removed the repository copy, and changed
exactly one required context in ruleset `16713108` from
`ruleset-governance` to `governance-static`. Reviewed before, after and inverse
payloads prove that all other protections and contexts were preserved. All
eight owner-side read-permission probes passed before secret migration.

PR #38 passed its six required contexts and CodeQL, then merged normally as
signed main commit `28d754729ae575e0078804e379bb29e1110785e0`. Main Validate
`34043264885`, CodeQL `34043264871`, exact-SHA static and live jobs, the live
decisive step, and the release evidence collector/checker all succeeded.
Dependabot PR #39 then passed all six required contexts and CodeQL on rebased
head `a207805fca54738cfcab46ed072807cfa9daabe8`; its real synthetic checkout and
static identity assertion succeeded while live governance correctly skipped.
PR #39 remains unmerged. No release, tag, signing operation or dependency merge
was performed. This evidence closes `RIT-GEN-038`; its typed remediation is
removed while all governance enforcement remains unchanged.

### Historical pre-activation status

The remaining Task 8 text records the local implementation and pending remote
state before activation. Its open-state statements are historical and are
superseded by the closure evidence above.

The old remote layout remains active, so `POLICY-RIT-GEN-038` stays typed and
open. No environment, secret, ruleset, workflow run or other GitHub state was
changed by this local implementation. On 2026-09-06 the repository owner
approved the bounded activation, but the exact before/after/inverse context
payload and post-merge main/PR evidence remain pending; see
`docs/github-ruleset-governance.md`.

Final local validation passed 328 policy/tooling tests with one intentional
live-token skip, 42 focused policy unit tests, strict policy-pack validation,
and the app strict gate with 465 library tests plus the stress unit and UI
smoke. Coverage was 84.6% against the unchanged 80% minimum. Independent
review found no remaining material issue after JSON `null` and potentially
credential-bearing transport errors were made explicit, redacted failures.
Logs are under `/tmp/riteed-p3-validation-hGkt8u/` as
`task8b-final-tools.log`, `task8b-final-policy-unittest.log`,
`task8b-final-policy-pack.log`, `task8b-final-app-strict.log`, and
`task8b-final-coverage.log`.

PR #38 CodeQL high alert 185 subsequently traced the configured `live_secret`
identifier into repository-present and environment-missing diagnostics. The
source is a policy identifier, not a PAT value, and no credential-value access
was demonstrated. The follow-up nevertheless minimizes diagnostic data: both
checks retain their decisions but report only the failing scope. A synthetic
identifier regression was RED against the PR head and now proves the helper
errors and actual CLI stdout do not repeat that identifier. Remote CodeQL rerun
evidence remains pending.

## RIT-GEN-023: rootless project sidebar state (Task 13)

The `project-sidebar-visible` change-state handler previously committed the
requested boolean before borrowing project state and checking for an active
root. A no-root true request therefore left the Gio action at true even though
the sidebar stayed closed. The project-search action also entered project
search unconditionally.

The handler now acquires the mutable state borrow first. A missing root resets
the action to false and returns; a valid root delegates directly to
`sidebar_state::set_sidebar_visibility`, which remains the single owner of the
action state, animation, callback and GSettings persistence. Project search
returns before either sidebar or search activation when no root exists.

The existing failed-root GTK restore scenario now fails setup explicitly,
reads the actual Gio action state rather than the action-enabled tuple, drives
the real change-state handler, and proves `win.find-in-files` is enabled before
invoking it. It asserts false action state and closed search after both paths,
no additional settings writes, and retention of the remembered true
preference. Existing valid-root toggle and restore coverage remains in the
same full GTK flow.

The pre-fix focused RED exited 101: the direct no-root toggle produced
`Some(true)` instead of `Some(false)`. With the minimal reorder and search
guard, the same complete GTK flow passed. Existing runtime-review anchors for
`window_project.rs` precede the edited handler and remain exact; no ownership
or justification metadata changed.

## RIT-GEN-026: steady-state editor presentation (Task 3)

GTK documents `TextBuffer::changed` as a content-change signal and
`TextBuffer::modified-changed` as firing when the modified bit flips. Riteed's
two callbacks both rebuilt tab presentation, so every edit after the document
was already dirty repeated title, tooltip, indicator and visual-state work.

The content-change callback now retains dirty-generation accounting, Markdown
preview scheduling and Source Control minimap stale checks, but leaves title
presentation to the existing modified-state transition callback. Save, open,
compare and other explicit presentation call sites remain unchanged.

The real GTK regression opens a named file and fails setup if its window or tab
is missing. It first establishes the legitimate clean-to-dirty transition and
observes the dirty tab indicator, then resets a per-tab `cfg(test)` counter
before a six-edit steady-state burst. The pre-fix RED counted six presentation
rebuilds; the focused GREEN counted zero. A real save writes the burst text,
clears the dirty indicator, and retains the file title, proving that removing
the content-change call did not remove dirty or save presentation updates.

The Task 9 opportunistic rename was not taken: `gtk_tests_v13.rs` remains a
small feature-level GTK suite, and no existing feature-named status-flow file
would accept this one flow without unrelated module/test movement.

## RIT-GEN-025: native encoding dialog shell (Task 5)

The encoding chooser previously set `AdwDialog:title` but placed its content
directly in the dialog. The title was therefore assistive metadata rather than
a visible header, and the dialog had no native header close control.

The chooser now passes its existing title through `build_dialog_shell`, reuses
the shell's content box, and keeps the existing 420-pixel width and
`follows-content-size=true` behavior. No gettext string changed or was added.
Current libadwaita documentation confirms that an `AdwHeaderBar` inside an
`AdwDialog` adapts its decoration layout to a close-only control, and that an
`AdwWindowTitle` is the supported custom title widget.

The dialog-lifecycle GTK flow now fails explicitly if its window fixture cannot
be built. In each of the existing ten encoding rounds it traverses the actual
presented widget tree and requires a visible `AdwWindowTitle` containing the
fixture title, plus a visible button-role control containing the
`window-close-symbolic` image. The definitive pre-fix RED observed both states
as `(false, false)` instead of `(true, true)`. Focused GREEN passed all rounds,
and the existing encoding leak canary continued to clear after every real
dialog close.

Removing the direct dialog/content builders shifted two existing runtime-review
anchors. Only their line numbers were updated; their matches, ownership and
justifications are unchanged. There were no encoding-dialog i18n-review anchors
to move.

## RIT-GEN-027: shared streaming-search accumulator (Task 6)

Large-file search previously passed owned carry and match vectors into every
recursive async window step. The callback cloned the carry before extending it
and cloned every match collected so far before scanning the next 256 KiB
window, making accumulated-offset copying grow with both chunk count and match
count.

Each search now creates one `Rc<RefCell<SearchState>>` containing the bounded
cross-window carry and match offsets. Processing takes the carry out for the
combined scan, keeps `current_start` equal to the old carry length, derives the
same saturating base offset, and either stores the bounded suffix for the next
window or moves the terminal match vector into `SearchOutcome`. The scoped
mutable borrow ends before invoking the caller or recursively dispatching the
next Gio read, so a reentrant completion cannot observe an active borrow.

The exact 10,000-match cap and `reached_cap` result, cancellation checks,
retained stream ownership, scanned-window offset and cross-chunk stitching are
unchanged. Task 6 retained `scanned_bytes` for the Task 10 cleanup documented
below. This was a characterization refactor rather than a defect repair: the
same seven focused
search tests passed before and after, including match-cap, cancellation,
retained-stream, byte-offset and cross-chunk cases; no artificial RED was
introduced. The existing parser-boundary mapping and test targets remain
exact, and its review date plus the new shared-state runtime anchor were
refreshed. The source shift also reanchors the existing test callback result
cell without changing its ownership or justification.

## RIT-GEN-031 and RIT-GEN-036: review counts and diff token reuse (Task 11)

The compare byte cap still runs before any original line-token vector is
allocated. For accepted byte sizes, the reference and current texts are each
tokenized exactly once through the existing `line_slices` helper. Their vector
lengths drive the unchanged 20,000-line and 10,000,000-line-product limits,
and the same vectors then feed whitespace normalization and `build_row_model`.
Normal comparison still performs one `TextDiff::from_lines`; no second splitter
or changed threshold was introduced.

A thread-local test counter measured four original-tokenization calls for an
accepted pair before the refactor and guards two afterward, while the existing
line-diff counter continues to guard one expensive diff. Byte-limit rejection
guards zero tokenization calls. Existing minus-one, exact and plus-one tests
cover byte, line and product limits, and Task 1's deterministic/property tests
retain original row identity for LF, CRLF, lone CR, blank, empty, trailing-line
and Unicode-whitespace cases in both compare modes. This is a performance
characterization refactor, so no artificial RED was introduced.

The Source Control review boundary already rendered `%d addition` versus
`%d additions` and `%d removal` versus `%d removals`, but the generic
`count_text` parameters hid both pairs from extraction. The pre-fix focused
extractor probe found zero of the two required pairs. Both literal pairs now
remain at their `ngettext` call sites, while a formatting-only helper replaces
`%d` after plural selection. Selection still saturates counts to `u32::MAX`,
but substitution displays the complete `usize` value.

The checked-in POT and Danish PO add exactly those two existing plural pairs.
Danish uses `%d tilføjelse`/`%d tilføjelser` and
`%d fjernelse`/`%d fjernelser`, preserving the placeholder. No gettext keyword,
extractor wrapper or unrelated count idiom changed. The parser registry now
records the tokenization guard, and runtime/i18n review artifacts own the new
thread-local counter and two short plural call sites.

## RIT-GEN-033, RIT-GEN-034 and RIT-GEN-037: dead surfaces (Task 10)

Fresh word-boundary searches immediately before deletion found only the
definitions of `current_line_ending_mode`, `current_review_file`,
`set_writability_for_tests` and `Document::set_saved`. Those four unused
getters/wrappers were removed while preserving the live
`set_current_line_ending_mode`, `set_saved_with_display_path`, writability
accessor and review-open-target behavior. The audit's `is_acknowledged`
classification was incorrect: the document-read GTK regression calls it, so it
remains unchanged.

The first compiler pass exposed two direct dependent dead surfaces. Removing
the empty pre-run read left the private `RiteedApp.state` field unread. The
owned `adw::Application` already owns startup, action, activate and open signal
handlers with strong clones of the same state, so removing the field does not
shorten state lifetime. Removing `current_review_file` likewise left
`ReviewSession::current_file_for_line` with no caller. Fresh searches confirmed
both facts before the field and helper were removed; no dead-code suppression
was added.

`SearchOutcome::scanned_bytes` was written by search completion but read only
by one test assertion. The field, outcome initializers and assertion were
removed; the scan still computes `next_offset` for the same retained-stream
window progression. Match offsets, cap reporting, cancellation and cross-chunk
behavior remain covered by the existing tests.

The remaining safe RIT-GEN-037 cleanup removes the empty window-vector clone
performed before `Application::run`, when activation cannot yet have created a
window, and drops the only crate-wide reference to the unregistered
`win.focus-project-sidebar` accelerator. The CSS file already uses modern
custom properties throughout, so its lone `alpha(@accent_bg_color, 0.16)` use
now reads `alpha(var(--accent-bg-color), 0.16)`. The optional maximized-state
enhancement remains deferred because it is not a dead-code correction.

RIT-GEN-035 remains deferred as approved. `ExternalFileEvent::Moved` has real
runtime and test references, while deciding whether to wire production monitor
events into that path or delete the behavior requires a separate behavioral
decision. The batch-2-owned allow annotation was not touched. This task is
deletion characterization, so no artificial RED was introduced. Exact runtime
review anchors shifted by the deletions and were reanchored without changing
their ownership or justification.

## RIT-GEN-029: test-only settings backend (Task 9)

Fresh source inspection found no non-test or stress caller of
`AppSettings::new_for_tests`: direct callers live in `cfg(test)` GTK/settings
modules or explicit test branches, and `src/bin` has no caller. The complete
Memory family is therefore now gated with `cfg(test)` rather than broadening
the predicate to stress.

The gate covers the `Rc`/`Mutex` imports, Memory enum variant and structs,
constructor, helper imports and functions, subscription noop, write logger and
every backend site across the 14 settings files. There are 63
`SettingsBackend::Memory` sites in total: one constructor and 62 match arms.
The non-test `record_memory_write` noop was removed because its type and only
possible callers are now test-only. Production has no fallback arm: GSettings
is the only compiled backend.

This is a pure compile-surface cleanup, not a behavior repair, so no artificial
RED was introduced. Unit and GTK tests still compile the same Memory backend.
The stress-feature and no-default-feature checks instead prove that release
configurations compile without it. The `dialogs::lifecycle` module already has
module-level `cfg(test)` in `dialogs.rs`; adding redundant annotations to its
three functions would not narrow the binary further and was intentionally
avoided. Existing GSettings review entries were reanchored to their shifted
production write lines without changing keys, triggers or ownership.

RIT-GEN-030 required no opportunistic rename in this pass. No version-named GTK
test file was touched for RIT-GEN-029, and moving one would add unrelated churn;
the earlier Task 3 decision to retain the focused v13 file remains unchanged.

## Validation environment

The existing Solarized chrome test assumes ordinary contrast. The desktop's
high-contrast setting correctly disables custom chrome CSS and exposed that
assumption. Runs use private Xvfb/D-Bus, isolated XDG config/data directories,
`GSETTINGS_BACKEND=memory`, `G_DEBUG=fatal-criticals`, Cairo/X11 and one Rust
test thread. No desktop preferences were changed. These runs do not claim
manual high-contrast UX or installed-Flatpak verification.
