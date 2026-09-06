# What the policy gate proves

`scripts/policy-check --root app --strict` runs the app gate. The wrapper can
be called from another working directory; an explicit root must identify the
actual target directory and never falls back to the current project.

A successful result means the configured automatic checks passed. It does
not certify every architectural or human-review requirement in `policy/`.
The source inventory printed by the gate counts files by validation owner;
it is not a test-coverage percentage.

## Enforcement map

| Requirement | Evidence and enforcement |
| --- | --- |
| Rust compilation, lints and tests | Configured Cargo workspace/all-targets/all-features commands really execute. |
| Rust family and required toolchain components | Read from `rust.policy.json`; stable full versions must match the configured family. |
| File length | `line_limit_globs`, test globs and reviewed waiver caps drive the line checker. Function-length design limits remain review requirements; Clippy has its own independent lint rules. |
| Runtime source rules | Policy regex checks plus lexical comment/literal masking and brace scopes for runtime review and async blocking calls. This is not macro expansion, type resolution or call-graph analysis. |
| Runtime evidence | Source-linked, typed, nonempty review entries. Ownership arguments, native-only guarantees and absence of reference cycles still require human assessment. |
| UI XML localization, surface discovery and icon naming | Structural XML parsing with source lines, including multiline attributes/content. Ambiguous same-line review sites and malformed XML fail. |
| Programmatically constructed UI and HIG | Rust source rules, widget tests and human review; XML discovery does not inventory Rust-created widgets. Menu review counts describe reviewed menus, not an independently computed Rust menu model. |
| Gettext | Extraction/POT equality, catalog format validation and source-linked context review. Extraction cannot prove that every arbitrary Rust string intended for display uses gettext. |
| Dependencies | Exact stack/lock synchronization and complete accounting of generated Cargo source entries. Every archive/checksum must belong to the lockfile; only the reviewed vendor config is additionally allowed. |
| Release check status | `tools.release_check_runs` executes the exact-commit, app-identity, newest-run, completed-success decision. Incomplete pagination, invalid IDs and malformed input fail closed. Offline tests exercise actual decisions. |
| Release workflow wiring | Exact helper invocation before signing, tag ancestry against fetched `origin/main`, AppStream top-release comparison, private-key import cleanup, GitHub-hosted signing runner, exact build checkout ref, approved governance condition, supported shell/working directory, and an unconditional success dependency chain for every signing job. Folded YAML scalars, decoy owners and conditional/error-tolerant paths are rejected. General workflow shell remains reviewed code. |
| Live repository governance | Separate token-scoped governance job. Offline policy validation cannot certify current remote settings. |
| App line coverage | Separate `tools.coverage_check` invocation; the percentage applies to app coverage, not Python validator tests. |
| Validator implementation | Root unittest discovery in CI plus negative fixtures. `--policy-pack-check` itself checks policy bundle integrity and scoped line limits; it is not a replacement for those tests. |

## Source accounting

`validation-tooling.policy.json:source_scope` assigns every discovered Rust
source exactly one category. Missing, overlapping and empty categories fail.
New untracked source files are included. Known build output is excluded.
Claims of source-pattern, runtime, line-limit and patch ownership are checked
against their corresponding configured scopes.

- App source: Cargo checks, source patterns, runtime review, line limits and
  the separate coverage gate.
- Integration tests/examples/benches: Cargo and applicable source patterns,
  line limits and coverage. Runtime/i18n scanners do not implicitly cover
  all these paths.
- `build.rs`: Cargo, applicable forbidden source patterns and line limits.
- Fuzz targets: separate stress/fuzz registry and execution plus dependency
  preflight. The ordinary app test command does not execute a full fuzz run.
- Each explicitly listed local dependency patch: release integrity and
  unsafe/FFI baseline checks plus dependency synchronization. Application
  no-unsafe rules are not applied indiscriminately to upstream binding code.

The category checker records validation ownership. It does not execute the
separate coverage or scheduled fuzz job itself. Domain policy rules and
manual design requirements remain binding even when no automatic verifier
can prove them.

## Regression evidence

Dedicated suites cover the 2026-09-05 audit failures:
`test_policy_audit`, `test_cargo_source_inventory`,
`test_release_gate_hardening`, `test_rust_scanner_hardening`,
`test_ui_xml_hardening` and `test_source_scope` under `tools/tests/`.
They use temporary local fixtures and require no network access. The release
guard suite also executes only the extracted ancestry commands against a local
temporary Git remote and the extracted AppStream Python block against temporary
metadata; mutated workflow shell is never executed. Run them together with
existing validator tests:

```sh
python3 -m unittest discover -s tools/tests -v
python3 -m tools.policy_check --policy-pack-check --strict
scripts/policy-check --root app --strict
python3 -m tools.coverage_check --root app
```

Release API input follows the [GitHub check-runs API](https://docs.github.com/en/rest/checks/runs#list-check-runs-for-a-git-reference):
fetch all pages with `filter=all`, then evaluate required checks for the exact
commit. Highest check-run ID selects the newest created run; queued/in-progress
reruns invalidate earlier successes. If the result changes during pagination,
validation fails and must be rerun.

The P2 release guard checks prove that the active preflight and signing jobs
retain their reviewed guard structure. `POLICY-RIT-GEN-028` records the
remaining SHA-binding gap: AppStream metadata still comes from the working
checkout, and the build checkout still resolves the validated tag ref. P3 Task
7 must read metadata from the validated commit SHA, checkout that exact SHA,
and add negative fixtures for both bindings before the debt entry is removed.
