---
created: 2026-05-25
updated: 2026-05-27
status: complete
priority: high
type: audit-report
---

# Riteed Security and Robustness Audit Report

## 1. Repo/Commit Audited

- Branch: `main`
- Commit: `6dc24fc346960b444cd4745d4554366db23ed49d`
- Date: `2026-05-25` Europe/Copenhagen
- Worktree note before this report: existing local changes were present in `README.md`, `docs/stresstest_plan.md`, `docs/audit.md`, and `docs/stress-test-plan.md`; this audit did not edit them.
- Commands/checks run during audit:
  - `rg`/`nl` inspections across `app/src`, `policy`, `tools`, `.github`, `app/build-aux`, `app/fuzz`, `stress`
  - Context7 checks for current gtk-rs and Flatpak guidance
  - delegated read-only subaudits for GTK lifecycle, Git/parser/fuzz, Flatpak/supply chain, and policy/GSettings/save-session
- Validation run after writing this report:
  - `git diff --check -- docs/audit_report.md` passed
  - `scripts/dependency-preflight --root app` passed with `[dependency-preflight] OK`
  - `python3 -m tools.coverage_check --root app` passed with line coverage `81.8%`
  - `python3 -m tools.policy_check --root app --strict` first hit `SIGSEGV` after the unit-test list while it was running concurrently with coverage; rerun alone passed with `[policy-check] OK`
  - `cargo fuzz list` passed and listed `markdown_parse`, `frontmatter_split`, `git_status_parse`, `diff_compute`, `unsupported_scanner`
  - Short fuzz smoke passed for all five targets with `cargo +nightly fuzz run <target> -- -runs=1`
- Checks not run as standalone commands:
  - `cargo test --workspace --locked`: not run separately because `policy_check --strict` ran the repo's stricter Cargo gate, including `cargo test --workspace --all-targets --all-features`
  - Full native stress suite: not run locally because it is a long GTK/Xvfb/DBus workflow; this audit inspected the scripts/workflow and found coverage gaps instead of treating the scheduled job as proof
  - ASan/Valgrind smoke scripts: not run locally because they require the heavier instrumented GUI/Flatpak smoke setup; scripts were located and considered in the CI coverage assessment

## V14.7 Remediation Status

- `RIT-AUD-010`: closed by V14.7 signing-key governance remediation. The
  publish workflow now compares the imported signing secret's public key with
  `app/build-aux/flatpak/riteed-beta-public.asc` before signing and uses the
  committed public key for generated remote metadata. The beta Flatpak README
  now documents rotation, revocation, compromise recovery, and emergency
  cutover with no `TBD` marker.
- `RIT-AUD-011`: closed by V14.7 CI supply-chain remediation. Release-critical
  GitHub Actions in publish and validation workflows are pinned to full commit
  SHAs with version comments, and cargo-installed CI tools now use exact
  versions.
- `RIT-AUD-002`: closed by V14.7 rollback remediation. The publish preflight
  queries the existing beta Pages remote, records the published version and
  OSTree commit, blocks normal publishes for versions older than the current
  beta, and exposes an explicit `emergency_rollback` path requiring a matching
  rollback ref and user-visible reason.
- `RIT-AUD-009`: closed by V14.7 local patch remediation. The `sourceview5`
  patch now carries a machine-checked manifest, official upstream `.crate`
  checksum, reviewed allowed changed files, canonical diff checksum, and
  unsafe/FFI baseline.
- `RIT-AUD-013`: closed by V14.7 session-restore remediation. Startup restore
  now ends the restore guard without persisting pruned session state to
  GSettings; failed restored files are durably pruned only on the next explicit
  session-changing action.
- `RIT-AUD-003`: closed by V14.7 open-lifecycle remediation. Detaching or
  closing a non-transferred page now cancels tab IO, invalidates stale IO
  generations, and open callbacks verify the original page still belongs to the
  same workspace tab before applying recent/session side effects.
- `RIT-AUD-004`: closed by V14.7 review-lifecycle remediation. Git review loads
  are tracked by both the review tab and source-control state, cancelled on tab
  close or project/root churn, and refused at finish if the tab is detached or
  the source generation is stale.
- `RIT-AUD-012`: closed by V14.7 minimap lifecycle remediation. Source-control
  minimap blob requests are tracked in source-control state and cancelled before
  refresh/root churn; callbacks drop stale repo generations before updating the
  tab.
- `RIT-AUD-005`: closed by V14.7 save-concurrency remediation. Saves capture a
  dirty-generation guard; small saves use a scratch-buffer snapshot, large
  manual saves temporarily lock editing, and successful saves preserve dirty
  state if the buffer or document identity changed while the save was running.
- `RIT-AUD-006`: closed by V14.7 Git timeout remediation. Git subprocesses now
  install a wall-clock timeout, cancel the operation through Gio, force-exit the
  process after a grace window, and schedule an async wait to reap cancelled
  subprocesses.
- `RIT-AUD-007`: closed at the app boundary by V14.7 Git status cap
  remediation. `parse_status` caps entries at 10,000, marks oversized snapshots
  as `too_large`, skips attr refresh for those snapshots, and the UI shows a
  degraded "too many changes" state without rebuilding the changed-file model.
- `RIT-AUD-016`: closed at the app boundary by V14.7 path-display remediation.
  Git path labels and tooltips escape C0, DEL, Unicode bidi controls, and
  backslashes while preserving raw bytes for Git identity and row actions.
- `RIT-AUD-008` and `RIT-AUD-015`: closed by V14.7 stress/fuzz remediation.
  The parser-boundary registry now maps required boundaries to fuzz, unit,
  integration, and stress evidence; Git status fuzz seeds use valid
  NUL-delimited porcelain v2 data; stress scripts declare real fixtures,
  actions, assertions, and artifact directories; and the stress runner consumes
  generated corpus/repo inputs instead of unrelated temp fixtures.
- Validation-tooling stability follow-up: the original concurrent
  `policy_check`/`coverage_check` `SIGSEGV` was not left as a sequencing
  convention. V14.7 added a shared validation command lock and gives
  `cargo-llvm-cov` an isolated target directory through
  `CARGO_LLVM_COV_TARGET_DIR`, so concurrent invocations serialize the
  GTK/Cargo command phase instead of racing on build state.
- `RIT-AUD-017`: closed by V14.7 repository-governance remediation on
  2026-05-27. After repository-owner approval, `Protect main` and
  `Protect version tags` were enabled and tightened through the GitHub ruleset
  API, PR #8 merged through the reviewed pull-request bypass path, and the
  live `ruleset-governance` job passed against the final ruleset and rollback
  environment state. Evidence: PR #8
  `https://github.com/cadric/riteed/pull/8`, merge event
  `https://api.github.com/repos/cadric/riteed/issues/events/26035812509`,
  governance run `https://github.com/cadric/riteed/actions/runs/26538566076`,
  `Protect main` payload hash
  `0fe8ecbd07e197ebb9c285c61dfa1db62e549732a78ae7071628402d452f3645`,
  `Protect version tags` payload hash
  `b73aaf895e3ce07c107e41ef7a60d821a3c022ca0e7530e3e2afade59bdf584c`,
  rollback environment hash
  `19b0f3a3cb987f0924cc7bd54c41df34141e4977f259c5ac093ea033ad9db70b`,
  and CodeQL snapshot `docs/evidence/v147-codeql-alerts-20260527.json`.

## 2. Threat Model

This audit treats Riteed as a local GNOME editor that may open attacker-controlled files, Markdown documents, Git repositories, filenames, and project trees inside the Flatpak sandbox after user selection. It also treats release infrastructure as security-critical because the beta Flatpak remote can ship signed updates to users.

The main threats considered are:

- malicious Markdown, text, frontmatter, and diff content intended to crash, hang, or mislead the UI
- malicious Git repositories and filenames, including non-UTF-8 and control-character paths
- compromised or mutable release infrastructure, workflow inputs, tags, and dependencies
- drift in local patched dependencies, especially generated FFI/unsafe binding surfaces
- policy gaps where documented intent is not backed by deterministic validation

The audit did not attempt a complete review of D-Bus surface, keyboard shortcut hijacking, undo/redo state corruption, or every GNOME platform API boundary. Those should be considered explicit non-scope items for this report, not positive assurance.

## 3. Executive Summary

- Runtime `app/src` has no verified `unsafe` blocks, FFI declarations, `static mut`, or runtime shell execution outside the reviewed Gio subprocess Git boundary.
- The Flatpak manifest is narrow: only Wayland and fallback X11 sockets, no broad filesystem, network, D-Bus, device, or home permissions.
- V14.7 closes the highest release governance risks by gating signing on exact-commit validation, blocking silent beta rollback, pinning release-critical CI inputs, pinning the signing key to the committed public key, and enabling the reviewed branch/tag GitHub rulesets.
- V14.7 closes the app-runtime async ownership risks by cancelling stale open, review, minimap, and Git work and by guarding completion paths with tab/source/document generations.
- Save completion now preserves dirty state when the buffer or document identity changed while a save was in flight; small saves use a snapshot path and large manual saves temporarily lock editing.
- Stress/fuzz assurance now uses a parser-boundary registry, valid Git status `-z` seeds, schema-backed stress scripts, and generated corpus/repo consumption.
- The bundled local `sourceview5` patch now has machine-checked upstream checksum, reviewed changed files, canonical diff checksum, and unsafe/FFI baseline enforcement.
- Git subprocess handling now combines shell-free Gio subprocess execution with owner cancellables, wall-clock timeouts, process cleanup, status entry caps, and escaped display names for unsafe path text.
- Remaining policy improvement ideas are hardening backlog, not active V14.7 audit gaps.

## 4. Findings Table

| ID | Severity | Area | Status | Closure evidence | Residual rule |
|----|----------|------|--------|------------------|---------------|
| RIT-AUD-001 | High | Release signing | Closed by V14.7. | Publish gates signing-secret import on exact tag-commit validation through the GitHub check-runs API, and the release validator enforces the gate. | Keep required contexts aligned with release policy before signing. |
| RIT-AUD-002 | High | Release rollback | Closed by V14.7. | Publish compares the candidate beta version and OSTree commit with the Pages remote and requires explicit emergency rollback inputs for non-monotonic targets. | Normal publishes must remain monotonic. |
| RIT-AUD-003 | Medium | GTK async lifecycle | Closed by V14.7. | Tab close/detach now cancels tab IO and open callbacks check page membership and document generation before applying side effects. | Keep close/detach paths wired to `EditorTab::cancel_io`. |
| RIT-AUD-004 | Medium | Git review lifecycle | Closed by V14.7. | Git review loads are owner-cancellable and finish only when the review tab and source generation still match. | Keep review loads tied to tab/source ownership. |
| RIT-AUD-005 | Medium | Save/concurrency | Closed by V14.7. | Save completion is dirty-generation and document-identity guarded; small saves use a scratch snapshot and large manual saves temporarily lock editing. | Never clear modified state after a stale save completion. |
| RIT-AUD-006 | Medium | Git resource boundary | Closed by V14.7. | `/app/bin/git` subprocesses have owner cancellables, wall-clock timeout, grace-window force-exit, and async reap cleanup. | Keep new Git callers on the typed subprocess boundary. |
| RIT-AUD-007 | Medium | Git UI scale | Closed by V14.7. | Git status parsing caps entries, marks oversized snapshots as `too_large`, skips attr refresh for those snapshots, and shows a degraded UI state. | Keep policy-owned status caps enforced. |
| RIT-AUD-008 | Medium | Stress/fuzz coverage | Closed by V14.7. | Stress scripts now declare real fixtures, actions, assertions, artifact directories, and consume generated corpus/repo inputs. | Preserve boundary fidelity in new stress scripts. |
| RIT-AUD-009 | Medium | Supply chain / FFI | Closed by V14.7. | The local `sourceview5` patch now has upstream crate checksum, reviewed changed files, canonical diff checksum, and unsafe/FFI baseline validation. | Patch drift must update the manifest and evidence. |
| RIT-AUD-010 | Medium | Release key governance | Closed by V14.7. | Publish compares the imported signing public key with the committed beta public key and docs cover rotation, revocation, compromise recovery, and emergency cutover. | Never sign with unpinned key material. |
| RIT-AUD-011 | Medium | CI supply chain | Closed by V14.7. | Release-critical GitHub Actions are pinned to full commit SHAs and cargo-installed CI tools use exact versions. | Keep action updates reviewable and SHA-pinned. |
| RIT-AUD-012 | Low/Medium | Source control minimap | Closed by V14.7. | Minimap blob reads are tracked per source generation and cancelled before refresh/root churn. | Keep minimap work owner-cancellable. |
| RIT-AUD-013 | Low/Medium | GSettings/session | Closed by V14.7. | Session restore ends the startup guard without durable pruning; missing files are pruned only after a later explicit session-changing action. | Avoid startup-time session writes. |
| RIT-AUD-014 | High | Policy enforcement | Closed by V14.6, out of V14.7 scope. | Validator iteration skips only known build-output roots while scanning legitimate source paths named `target`. | Keep path-skip exceptions root-scoped. |
| RIT-AUD-015 | Low/Medium | Policy enforcement | Closed by V14.7. | Parser-boundary registry enforcement is bidirectional and Git status fuzz seeds use valid NUL-delimited porcelain v2 records. | New parser boundaries need registry evidence. |
| RIT-AUD-016 | Low | UI integrity | Closed by V14.7. | Git path labels/tooltips escape C0, DEL, Unicode bidi controls, and backslashes while preserving raw bytes for Git identity. | Keep display text separate from raw Git identity. |
| RIT-AUD-017 | High | GitHub governance | Closed by V14.7 on 2026-05-27. | PR #8 merged after all required checks and CodeQL passed; live `ruleset-governance` passed with active `Protect main`, active `Protect version tags`, exact status checks, signed commits, reviewed `pull_request` bypass only on `Protect main`, no tag bypass actors, and reviewed rollback environment. | Re-run `tools.ruleset_governance_check` after every ruleset change. |

## 5. Deep Dives

### GTK/GObject lifecycle

V14.7 moved the high-value async paths onto owner-cancellable flows. Closing or detaching a page cancels tab IO, open callbacks verify page membership and document generation, Git review loads are cancelled on tab close or source churn, and minimap blob loads are tied to source-control generation.

Search and compare keep their existing weak-guarded callback model. Those paths remain lower-risk hardening candidates, not active V14.7 findings.

Generated review buffers now follow the guarded review-load lifecycle rather than allowing detached review tabs to be populated after user intent moved on.

### Git boundary

The Git boundary is well designed against shell injection: production uses `/app/bin/git`, argv vectors through Gio subprocess, `cwd="/"`, a fixed environment (`LC_ALL=C`, no prompts, no pager/editor/external diff), and raw NUL-delimited stdin where needed. Non-UTF-8 paths are preserved in `GitPath` and mutating row actions reject them.

The V14.7 closeout adds the missing resource and UI integrity boundaries. `status --porcelain=v2 -z --untracked-files=all` now has a policy-owned entry cap, oversized snapshots set `too_large`, attr refresh is skipped for degraded snapshots, and the Source Control UI renders the too-many-changes state instead of rebuilding every row.

Git command execution is cancellable and time-limited, with force-exit and async reap cleanup after timeout. Host/prod drift still exists because many behavior tests use `/usr/bin/git`, but the Flatpak smoke path and generated stress repos cover the packaged `/app/bin/git` boundary.

### Parser/Fuzz/Stress coverage

The repo has fuzzing and property coverage for Markdown parse, frontmatter split, Git status parse, diff compute, and unsupported scanner. V14.7 adds a bidirectional parser-boundary registry so new parser/trust-boundary sites need mapped evidence, and the Git status corpus now seeds valid NUL-delimited porcelain v2 records.

Stress scripts now declare the flow, description, expected failure mode, fixtures, actions, assertions, and artifact directory they need. The stress runner consumes generated Markdown corpora and Git repos and asserts preview, compare, source-control, save, search, and artifact states so named scripts exercise the boundaries they advertise.

### Flatpak sandbox

Manifest finish-args are narrow and appropriate for a graphical GNOME editor:

- `--socket=wayland` is required for the GTK/libadwaita UI.
- `--socket=fallback-x11` is a compatibility fallback with higher display-server blast radius than Wayland, but it is the only broad-ish permission found.
- No `--filesystem=host`, `home`, `xdg-*`, `--share=network`, `--device=*`, `--talk-name`, or `--own-name` finish-args were present.

File access is portal-oriented from the UI, but once a user grants access to a project/repo path, `/app/bin/git` intentionally operates on that granted filesystem view. That is consistent with the app model; the robustness work is around Git command limits and path handling, not sandbox expansion.

Bundled Git is pinned to Git `2.54.0`, fetched from kernel.org with sha256 and verified against signed `sha256sums.asc` using a committed kernel.org autosigner key. Git is built without network-facing helpers and stripped down before install. This is a positive control.

### Supply chain/dependency pipeline

Dependency preflight is strong for the gtk-rs stack pins, safe/sys version pairing, fuzz lock sync, and Flatpak cargo-source checksums. V14.7 extends the release policy checker to cover local patched crate drift. The local `sourceview5` patch now carries an upstream `.crate` anchor, the official checksum, reviewed changed files, a canonical diff checksum, and an unsafe/FFI baseline so future drift has a machine-checked comparison anchor.

Baseline command: `rg -c '\bunsafe\b|\bextern\s+"C"|\btransmute\b' app/build-aux/cargo-patches/sourceview5/src -g '*.rs'`, then sum the per-file counts.

Flatpak YAML source hash enforcement is mostly adequate for the current manifest shape, but the parser is line/count based. A future complex YAML representation could satisfy global URL/hash counts without proving per-source pairing. This is low severity because current manifest lines are simple and pinned.

### GPG/OSTree/GitHub Pages signing

The release workflow uses a protected signing environment, imports the private key into a temporary `GNUPGHOME`, presets the passphrase, signs Flatpak commits and summary metadata, checks for symlinks/hardlinks, and deploys Pages with minimal `pages: write`/`id-token: write` permissions. Secrets are scoped to the build step.

V14.7 added the missing controls: pre-sign validation gating, monotonic remote update checks, explicit emergency rollback inputs, key pinning to the committed public key, and documented rotation, revocation, compromise recovery, and emergency cutover procedures.

Read-only GitHub checks originally found two repository rulesets, `Protect main` and `Protect version tags`, with `enforcement: disabled`; classic `main` branch protection also returned `Branch not protected`. V14.7 enabled both repository rulesets through the GitHub ruleset API after repository-owner approval, recorded before/after evidence in `docs/github-ruleset-governance.md`, and added live release-policy verification that requires active rulesets, strict exact required checks, reviewed pull-request bypass only on `Protect main`, no tag bypass actors, and the reviewed rollback environment. PR #8 closed the live enforcement loop on 2026-05-27 with all required validation contexts green; the scheduled stress job remains outside the release-critical required context list and is tracked separately from `RIT-AUD-017`.

### Policy intent vs enforcement

| Policy intent | Stated in | Current enforcement | Gap | Recommendation |
|---------------|-----------|---------------------|-----|----------------|
| Runtime Rust has no unsafe/FFI. | `AGENTS.md`, `policy/rust.policy.json:158`, `policy/validation-tooling.policy.json:289` | Scans app source and release-policy validation checks local release-critical patches. | Local patched dependency drift is now manifest-backed. | Keep patch manifest evidence in sync with source changes. |
| Parser/untrusted inputs require property or fuzz tests. | `policy/rust.policy.json:265`, `policy/stress-fuzz.policy.json` | Coverage/tests run, fuzz targets exist, and parser-boundary registry entries are enforced bidirectionally. | New boundaries can only pass with registry evidence or reviewed exception. | Keep registry entries anchored to source markers and real input shapes. |
| GSettings writes only explicit user actions, not startup. | `policy/gsettings.policy.json:49` | Regex scans actual `.set_*` lines and local text windows. | Wrapper calls from startup/session restore are call-site blind. | Track settings wrapper calls or require typed write APIs with context labels. |
| GSettings schema artifacts must not be committed. | `policy/gsettings.policy.json:80` | Schema compile command exists. | No explicit scan for `gschemas.compiled` found in inspected checker. | Add committed-artifact check. |
| Review artifacts prove reviewed runtime/GSettings/UI exceptions. | `policy/README.md`, `tools/scanners/sites.py:141` | Line number plus substring match. | Semantic edits can retain the substring and pass. | Store stable code hashes or richer structured evidence. |
| Flatpak downloaded sources are hash-pinned. | `policy/flatpak-metadata.policy.json` | Current manifest is checked by count/regex parser. | YAML pairing can be bypassed by future complex syntax. | Use a real YAML parser or flatpak-builder manifest introspection. |
| Release signing is safe by workflow policy. | `.github/workflows/publish-flatpak.yml`, docs, `policy/release.policy.json` | Exact-commit validation gate, monotonic remote update, public-key pin, rollback mode, action pinning, and ruleset verification are enforced. | Release governance now depends on keeping remote rulesets active. | Keep ruleset rollback approval-controlled. |
| Validators cover scoped source paths. | `policy/*.json`, `tools/validation_tooling.py:150` | `iter_files` recursively scans while excluding known build-output roots. | V14.6 closed the arbitrary `target` path-component skip. | Keep root-scoped build-output exclusions narrow. |
| Coverage gate proves current worktree coverage. | `tools/coverage_check.py` | Runs `cargo llvm-cov` by default. | `--json-summary` accepts stale/unrelated summaries without provenance. | Embed command/revision provenance or limit documented use. |
| GSettings migrations stay valid. | schema policy and `scripts/verify_schema_migration.sh` | CI compiles schemas. | Migration smoke is partial and not in CI. | Add migration smoke to validation and expand key coverage. |

### Save/session/concurrency

V14.7 closes the save-concurrency concern. Save start records the dirty generation and document identity; successful completion only clears modified state when those identities still match. Small saves use a scratch-buffer snapshot, and large manual saves temporarily lock editing.

Session restore intentionally suppresses persistence while `restoring_session` is true. V14.7 keeps that startup guarantee through failure handling: `finish_restore` now ends the restore guard without calling durable session persistence. Missing restored files are pruned from durable settings only when a later explicit session-changing action invokes the normal persistence path.

Same-file-in-two-windows is not fully audited by a global ownership registry. Within one workspace, `find_tab_by_file` deduplicates only that workspace's tab list. SourceView stale-save detection mitigates some cross-window overwrite cases when the same file's modification state changes on disk, but no explicit app-wide lock/owner strategy was found.

### GSettings

Schemas and wrappers are generally typed, and schema compilation is part of CI. The policy gap is semantic enforcement: regex hits on `.set_value` over-review non-GSettings calls while missing wrapper call-site context. Session/recent persistence can silently drop writes on `try_borrow_mut` contention, and migration smoke coverage is not part of CI.

Simple grep-based review found no obvious GSettings secret storage. A stronger claim would require schema-by-schema semantic review, so the main audited concern remains policy consistency and recovery behavior rather than confirmed secret exposure.

### CI fatal-criticals/ASan/Valgrind coverage

Native CI runs GTK tests under `G_DEBUG=fatal-criticals`, which is a strong baseline. V14.7 improves flow coverage by making the stress scripts exercise preview, compare, Git, review, save, search, corpus, and artifact paths. Existing ASan/Valgrind scripts were noted in repo docs, but this audit did not run them.

The audit inspected workflow YAML but did not separately verify runner integrity beyond the jobs' visible use of GitHub-hosted Ubuntu runners. If self-hosted runners are added later, ephemerality, isolation, and attestation should become release-policy requirements.

One original `policy_check --strict` run hit `SIGSEGV` while running concurrently with coverage, then passed alone on rerun. V14.7 added a shared validation command lock and an isolated cargo-llvm-cov target directory so the GTK/Cargo command phase serializes instead of racing on build state.

## 6. Positive Assurance

- `app/src/lib.rs` forbids unsafe code at crate level, and direct grep of `app/src` did not find runtime unsafe blocks, C FFI, `static mut`, `transmute`, or unsafe attributes.
- The Git subprocess boundary uses Gio subprocess argv vectors, not a shell, and sets a restrictive Git environment.
- Non-UTF-8 Git paths are preserved as raw bytes in parser state and blocked for mutating row actions.
- Flatpak finish-args are narrow and avoid broad filesystem, network, D-Bus, and device permissions.
- Bundled Git source is pinned by sha256 and additionally checked against signed kernel.org checksums during Flatpak build.
- Fuzz, proptest, parser-boundary registry, and stress-script evidence are present for parser and trust-boundary layers.
- The workflow uploads stress/fuzz artifacts on failure, which helps preserve reproducer evidence.
- The publish workflow uses temporary `GNUPGHOME`, kills the GPG agent on exit, and checks Pages artifacts for symlinks/hardlinks.

## 7. Top 10 Next Actions

1. Keep `RIT-AUD-017` closed: preserve the active `Protect main` and `Protect version tags` rulesets, keep live governance green, and use the documented rollback only after renewed release-governance review.
2. Keep release signing controls aligned with policy: exact-commit validation, monotonic publish checks, key pinning, rollback approval, and action SHA pinning must remain enforced.
3. Keep the `sourceview5` patch manifest synchronized whenever the local patch, upstream crate, or unsafe/FFI baseline changes.
4. Keep parser-boundary registry entries and stress scripts in sync with new parser, preview, compare, Git, save, and search boundaries.
5. Add future GSettings semantic hardening so wrapper calls carry explicit user-action/startup context instead of relying only on scanner locality.
6. Add future schema migration smoke coverage to CI if migration behavior expands.
7. Consider richer review-artifact anchoring, such as code hashes, for high-risk runtime and settings exceptions.
8. Keep Flatpak manifest hash-pairing checks narrow or replace the remaining regex/count logic with structured manifest introspection if the manifest shape becomes more complex.
9. Keep Git status caps and subprocess timeout thresholds policy-owned when they change.
10. Keep the shared validation command lock in place when adding new policy or coverage commands.
