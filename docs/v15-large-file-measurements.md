# V15 Large-File Measurements

Created: 2026-05-31

## Current Cap Decision

V15 starts with `EDITOR_HARD_LIMIT_BYTES` equal to the existing 25 MiB
snapshot-safe cap. The 500 MiB roadmap value is a policy ceiling for tiering
and warning copy, not a shipped promise that `GtkTextBuffer` editing is safe at
that size.

No larger edit cap is enabled until a generated-fixture measurement run shows:

- editor load and manual save preserve sourceview5 encoding, BOM, newline, and
  stale-save behavior;
- no UI heartbeat gap above 500 ms during load/save setup or completion;
- load and save duration are documented for 25, 75, 250, and 500 MiB generated
  files where the machine can support them;
- RSS hard-fails at `min(3 GiB, 6 * file_size + 500 MiB)`;
- any RSS above 3x file size is called out and may lower the shipped cap;
- large manual save does not leak the edit lock on success, failure, cancel,
  stale retry, generation mismatch, or tab mode switch.

Generated fixtures must stay in temporary directories and must not be committed.

## Initial Evidence

The initial implementation keeps editor-mode expansion conditional. Large-file
viewing uses async Gio paged reads and a separate read-only viewer surface, so
files above the measured edit cap remain inspectable without loading the whole
file into a `GtkTextBuffer`.
