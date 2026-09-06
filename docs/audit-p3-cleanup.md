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
Git subprocess pipes still buffer complete output until Task 12B is done.
RIT-GEN-021 is therefore not yet fully closed.

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

## Validation environment

The existing Solarized chrome test assumes ordinary contrast. The desktop's
high-contrast setting correctly disables custom chrome CSS and exposed that
assumption. Runs use private Xvfb/D-Bus, isolated XDG config/data directories,
`GSETTINGS_BACKEND=memory`, `G_DEBUG=fatal-criticals`, Cairo/X11 and one Rust
test thread. No desktop preferences were changed. These runs do not claim
manual high-contrast UX or installed-Flatpak verification.
