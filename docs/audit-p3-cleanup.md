# Audit P3 cleanup evidence

This pass starts from integrated local main `6aaffc3e`. Findings are closed
only after their implementation and validation; the overall pass is ongoing.

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

## Validation environment

The existing Solarized chrome test assumes ordinary contrast. The desktop's
high-contrast setting correctly disables custom chrome CSS and exposed that
assumption. Runs use private Xvfb/D-Bus, isolated XDG config/data directories,
`GSETTINGS_BACKEND=memory`, `G_DEBUG=fatal-criticals`, Cairo/X11 and one Rust
test thread. No desktop preferences were changed. These runs do not claim
manual high-contrast UX or installed-Flatpak verification.
