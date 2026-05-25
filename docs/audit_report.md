---
created: 2026-05-25
updated: 2026-05-25
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
- The highest release risk is governance: the signed Flatpak publish workflow can run without requiring the separate validation workflow, older valid tags can overwrite the beta remote, and existing GitHub rulesets are disabled.
- The highest app-runtime risks are stale async ownership paths: file open and Git review loading can continue after tab detach/close and then mutate retained, off-screen state.
- Save uses the live `TextBuffer` during asynchronous save and clears dirty state unconditionally on success, so edits made during the save window are not snapshotted defensively.
- Fuzzing and stress infrastructure exists and is useful, but it does not fully exercise the real GTK/SourceView/Git subprocess/UI boundaries named by the stress scripts.
- The bundled local `sourceview5` patch is a real FFI/supply-chain boundary, but policy and dependency preflight do not machine-check its upstream diff, unsafe/FFI surface, or drift.
- Git subprocess handling avoids shell injection and preserves raw non-UTF-8 paths in parser state, but large status sets and long-running Git commands still have resource-boundary gaps.
- Several policy intentions are prose-only or regex-only: parser fuzz requirements, GSettings user-action rules, release signing gates, local patches, and review artifact semantics.

## 4. Findings Table

| ID | Severity | Area | File/line | Finding | Evidence | Impact | Repro/trigger | Recommendation | Confidence |
|----|----------|------|-----------|---------|----------|--------|---------------|----------------|------------|
| RIT-AUD-001 | High | Release signing | `.github/workflows/publish-flatpak.yml:35`, `.github/workflows/validate.yml:14`, GitHub ruleset API | Signed Flatpak publish is not gated by the repo validation workflow. | Publish validates tag/version/main ancestry, then imports signing secrets and builds a signed repo; dependency preflight, policy, coverage, native tests, stress, and Flatpak tests live in the separate `Validate` workflow. `RIT-AUD-017` confirms no active GitHub ruleset mitigation. | A `v*` tag on an ancestor of `main` can publish a signed beta update even if validation failed or never ran for that commit. | Push or manually run a matching tag that has not completed `Validate`. | Make publish require the validation workflow/check suite for the exact tag commit before accessing `flatpak-beta-signing`, or rerun `dependency-preflight`, `policy_check --strict`, coverage, and Flatpak smoke in publish before importing the key. | High |
| RIT-AUD-002 | High | Release rollback | `.github/workflows/publish-flatpak.yml:3`, `.github/workflows/publish-flatpak.yml:157`, `.github/workflows/publish-flatpak.yml:175`, GitHub ruleset API | Older valid tags/manual reruns can overwrite the beta remote. | Workflow supports `workflow_dispatch`; preflight only checks tag/version/main ancestry; build deletes `site` and writes a fresh `site/flatpak/repo`. `RIT-AUD-017` confirms disabled tag rulesets, so there is no active tag-rule mitigation. | Accidental rollback or abused manual rerun can publish a signed downgrade to current beta users. | Rerun publish for an older valid tag after a newer beta exists. | Add a monotonic version/ref check against the currently published Pages remote and require an explicit emergency rollback path with separate approval. | High |
| RIT-AUD-017 | High | GitHub governance | GitHub ruleset API, branch protection API | Existing GitHub repository rulesets are present but disabled. | Read-only GitHub checks found `Protect main` and `Protect version tags` with `enforcement: disabled`; classic `main` branch protection returned `Branch not protected`. | Release findings have no active branch/tag ruleset mitigation today. Disabled draft rulesets are either misconfiguration or intentional bypass and should not be treated as protection. | `gh api repos/cadric/riteed/rulesets`; `gh api repos/cadric/riteed/branches/main/protection`. | Enable the rulesets after validating required checks and bypass policy, or document the intentional disabled state with typed, time-bounded planned remediation. | High |
| RIT-AUD-003 | Medium | GTK async lifecycle | `app/src/workspace_close.rs:97`, `app/src/editor_tab/runtime.rs:15`, `app/src/workspace_open.rs:189`, `app/src/editor_tab/open.rs:205` | Closing/detaching a loading tab does not cancel its open IO. | `on_page_detached` clears zoom/monitor but does not call `cancel_io`; open callbacks hold a strong `tab_for_result`; `load_file` applies loaded document after weak upgrade and generation check. | Closed tabs can be retained until load finishes, update buffers/state off-screen, and trigger recent/session callbacks after user intent moved on. | Open a slow/large portal-backed file and close the tab before SourceView load completes. | On page detach/close, cancel tab IO and verify tab membership/page identity before applying open callbacks or session/recent side effects. | High |
| RIT-AUD-004 | Medium | Git review lifecycle | `app/src/source_control/review_loader.rs:40`, `app/src/source_control/review_loader.rs:62`, `app/src/source_control/review_loader.rs:175` | Git review loading retains and updates closed review tabs. | `ReviewLoad` owns a strong `Rc<EditorTab>` and private `gio::Cancellable`; `finish` always calls `populate_review_session_with_spec`. | Slow multi-file reviews keep detached tabs/widgets alive and can populate off-screen buffers after close or project switch. | Start a large Git review, then close the generated review tab before blob/worktree reads complete. | Store review load ownership/cancellable in tab or source-control state, cancel on close/project switch, and guard `finish` with tab membership plus generation. | High |
| RIT-AUD-005 | Medium; High if confirmed | Save/concurrency | `app/src/editor_tab/save.rs:171`, `app/src/editor_io.rs:191`, `app/src/editor_tab/runtime.rs:423`, `app/src/editor_tab/runtime.rs:280` | Async save uses the live buffer and clears dirty state unconditionally on success. | `save_to_path` passes `self.text_buffer` into `FileSaver`; `set_loading` does not make the buffer read-only; successful save calls `set_modified(false)`. | Edits made after save start can be written unintentionally or marked clean without a snapshot/dirty-generation check. This is the most plausible local data-loss class found. | First V14.7 save/session action: reproduce with a slow save and typing during save before choosing the fix shape. | Snapshot text/format at save start or record a dirty generation and only clear modified if the buffer generation has not advanced; if repro confirms data loss, bump severity to High and consider temporarily disabling editing for manual save until a precise generation guard lands. | Medium |
| RIT-AUD-006 | Medium | Git resource boundary | `app/src/git_process.rs:248`, `app/src/git_process/ops.rs:15`, `app/src/git_process/ops.rs:123` | Git subprocesses have output caps but no wall-clock timeout. | `communicate_async` is cancellable, but no timeout source is attached; status is capped at 4 MiB and blob reads at 1,000,001 bytes. | Malicious or pathological repos can keep `/app/bin/git` work alive until user navigation cancels it; some callers, such as minimap blob reads, have no owner-held cancellable. | Repo with slow filters disabled? large index/object access, blocked filesystem, or many repeated refresh-triggered blob reads. | Add per-operation timeouts with Gio cancellables and explicit process cleanup; make every caller's cancellable owner-visible. | Medium |
| RIT-AUD-007 | Medium | Git UI scale | `app/src/git_status.rs:262`, `app/src/source_control/list_view.rs:66`, `app/src/source_control/tree_model.rs:31`, `app/src/source_control/refresh.rs:139` | Git status has a byte cap but no entry-count/UI-work cap. | Parser pushes every record under the 4 MiB stdout cap; list/tree views rebuild all entries; `too_large` exists in `GitStatusSnapshot` but is never set by `parse_status`. | Hundreds of thousands of short paths can stay under byte cap while forcing high allocations, attr checks, sorting, and GTK model rebuilds. | Create a repo with many short untracked files and refresh Source Control. | Add max-entry and max-attr-path caps, set `too_large`, and show a degraded "too many changes" UI state instead of rebuilding everything. | High |
| RIT-AUD-008 | Medium | Stress/fuzz coverage | `app/src/bin/riteed_stress.rs:37`, `.github/workflows/validate.yml:214`, `.github/workflows/validate.yml:221` | Scheduled/manual stress does not drive the real boundaries named by its scripts. | CI generates Markdown corpus and Git stress repos, but the stress binary opens fresh temp folders/files; compare script does not start compare, markdown script does not toggle preview, git-status script does not open the generated pathological repos. | Stress jobs can pass while non-UTF-8 paths, conflicts, submodules, compare rendering, Markdown preview rendering, and save/search flows are barely exercised. | Run generated stress repos, then run `RITEED_STRESS_SCRIPT=../stress/scripts/git-status-stress.json`; the app opens a temp stress folder instead. | Wire stress scripts to generated corpus/repos and assert UI state transitions for preview, compare, Git status, save, and search. | High |
| RIT-AUD-009 | Medium | Supply chain / FFI | `app/Cargo.toml:37`, `app/fuzz/Cargo.toml:16`, `app/build-aux/dependencies/sourceview5.md:7`, `tools/checks/dependency_preflight.py:356` | Local `sourceview5` patch is outside machine supply-chain checks. | The app and fuzz crate patch `sourceview5` to a local path; dependency preflight derives cargo-source expectations only from registry packages with `source` and `checksum`; the unsafe/FFI baseline command summed to 1329 text matches. | Patch drift can pass policy and dependency preflight without a machine-checked upstream diff, reviewed unsafe/FFI inventory, or patch manifest. | Edit code under `app/build-aux/cargo-patches/sourceview5/src`; current dependency preflight does not tie it to upstream `sourceview5 = 0.11.0`. | Add a patch manifest containing upstream crate checksum, allowed changed files, diff checksum, unsafe/FFI baseline, and review evidence; enforce it in dependency preflight. | High |
| RIT-AUD-010 | Medium | Release key governance | `.github/workflows/publish-flatpak.yml:148`, `.github/workflows/publish-flatpak.yml:155`, `app/build-aux/flatpak/README.md:27` | Signing key is self-consistent but not pinned to the committed public key; rotation/revocation is TBD. | Workflow checks secret key fingerprint equals `FLATPAK_GPG_KEY_ID`, then exports that same imported key into generated remote metadata; README lists key rotation as `TBD`. | A secret/environment misconfiguration can publish a remote signed by a different key without failing against the repo's committed public key contract; compromise recovery has no documented trust path. | Change signing environment secrets to a different key with matching secret ID. | Compare exported secret public key/fingerprint against `app/build-aux/flatpak/riteed-beta-public.asc` and add rotation, revocation, and emergency cutover procedure. | High |
| RIT-AUD-011 | Medium | CI supply chain | `.github/workflows/validate.yml:17`, `.github/workflows/validate.yml:78`, `.github/workflows/publish-flatpak.yml:107`, `.github/workflows/publish-flatpak.yml:211` | CI/release actions and tool installers are mutable inputs. | Workflows use major action tags and install Rust tooling via `curl ... sh` plus unversioned `cargo install cargo-llvm-cov --locked`. | Validation and release behavior can change without a repo diff. Release job has least permissions, but still trusts mutable action tags before signing. | Upstream action major tag or installer output changes. | Pin release-critical actions by commit SHA and pin cargo-installed tool versions; consider applying the same to validation jobs that gate releases. | Medium |
| RIT-AUD-012 | Low/Medium | Source control minimap | `app/src/source_control/minimap.rs:130`, `app/src/source_control/minimap.rs:141` | Minimap blob reads are stale-guarded but not owner-cancellable. | `load_reference_blob` creates a local cancellable, but tab/source state does not store it; callback uses weak state/tab and snapshot checks. | UI writes are mostly blocked, but repeated refreshes, tab closes, or project switches can leave unnecessary Git blob reads running. | Trigger repeated source-control refreshes for a modified clean file. | Store and cancel minimap blob requests per tab/source generation. | Medium |
| RIT-AUD-013 | Low/Medium | GSettings/session | `policy/gsettings.policy.json:49`, `app/src/workspace_open.rs:278`, `app/src/workspace/session_state.rs:30` | Session restore can write GSettings during startup failure handling. | Policy forbids startup writes; restore flips `restoring_session` false and immediately persists session state, pruning failed restored files. | Startup can mutate durable session state without explicit user action; this is probably intentional pruning, but conflicts with the strict policy wording. | Start with stale/missing session files. | Either defer session pruning until after startup/user-visible state settles and document it as explicit recovery behavior, or add a narrow enforced policy exception. | Medium |
| RIT-AUD-014 | High | Policy enforcement | `tools/validation_tooling.py:15`, `tools/validation_tooling.py:150` | Policy scanning skips any path component named `target`. | `iter_files` excludes every file whose path parts contain `target`, not only build-output roots. | Real source under paths such as `src/target/foo.rs` would bypass forbidden patterns, line limits, GSettings, runtime, and review-artifact checks. This is a V14.6 blocker because new policy gates would inherit the blind spot. | Add `app/src/target/example.rs` with a forbidden pattern; scoped iteration skips it. | Skip only known build-output directories rooted at repo/app target paths, not arbitrary path components, with a regression test before adding new policy files. | High |
| RIT-AUD-015 | Low/Medium | Policy enforcement | `policy/rust.policy.json:265`, `tools/policy_check.py`, `app/fuzz/fuzz_targets/git_status_parse.rs:5` | Parser fuzz/property requirement is policy intent, not a complete machine gate. | Policy requires property/fuzz tests for parsers and untrusted inputs; existing fuzz targets are real, but the validator does not prove new parsers have matching fuzz, and the Git status seed is newline-delimited rather than valid `-z` porcelain. | Future parser/trust-boundary additions can pass policy without fuzz; current corpus underrepresents valid Git `-z` records. | Add a new parser module without fuzz target; policy still focuses on generic tests/coverage. | Add a parser-boundary registry or review artifact enforced by policy check; seed Git status fuzz with valid NUL-delimited porcelain v2 records including controls/non-UTF-8/unmerged paths. | High |
| RIT-AUD-016 | Low | UI integrity | `app/src/git_status.rs:98`, `app/src/source_control/list_view.rs:176`, `app/src/source_control/tree_model.rs:140` | Valid UTF-8 Git paths with control characters are displayed unescaped. | `GitPath::from_bytes` stores valid UTF-8 directly; list/tree labels and tooltips use display text. | Paths containing newline/tab/control characters can distort Source Control rows/tooltips and make file identity misleading. This is not command injection. | Create a Git path containing newline or tab. | Render display names with escaped controls or visible replacement glyphs while keeping raw bytes for Git identity. | Medium |

## 5. Deep Dives

### GTK/GObject lifecycle

The dominant lifecycle risk is async ownership rather than raw ref-cycles. Open flows and Git review flows both have weak workspace/state guards, but they also retain strong tab references long enough to mutate detached state. `workspace_close.rs:97-115` clears zoom/monitor on detach but not IO, while `EditorTab::cancel_io` exists at `runtime.rs:15-31`. Git review loading is stronger evidence: `ReviewLoad` owns `Rc<EditorTab>` and a private cancellable that no tab close path can cancel.

Search and compare have a mix of better patterns. Compare stores style handler IDs and disconnects in `Drop`; search style handlers are connected to the global `StyleManager` and only weakly guard callbacks, so closed windows can leave app-lifetime closures behind. I classify that as low hardening, not a primary data-risk.

TextBuffer visual layers are mostly careful. Normal load disables undo while replacing text. Git review generated content uses the main editor buffer, which starts undo-enabled; review rendering calls `set_text`/`set_modified(false)` rather than the normal load helper. That should be tightened so generated read-only review buffers cannot accumulate undo history.

### Git boundary

The Git boundary is well designed against shell injection: production uses `/app/bin/git`, argv vectors through Gio subprocess, `cwd="/"`, a fixed environment (`LC_ALL=C`, no prompts, no pager/editor/external diff), and raw NUL-delimited stdin where needed. Non-UTF-8 paths are preserved in `GitPath` and mutating row actions reject them.

The remaining Git risks are resource and UI integrity boundaries. `status --porcelain=v2 -z --untracked-files=all` has a byte cap but not an entry cap. `too_large` exists in the snapshot type but is not populated by `parse_status`; oversized command output becomes a generic refresh error, while many short entries can still pass and overload GTK model rebuilds. Git command execution is cancellable but not time-limited, and some callers do not retain cancellables for owner-driven cleanup.

Host/prod drift is real but partially mitigated. Tests use `/usr/bin/git`, while Flatpak production uses `/app/bin/git`; CI has a Flatpak smoke for `/app/bin/git --version` and porcelain output from generated repos, but most behavior tests still exercise host Git and pure parsers.

### Parser/Fuzz/Stress coverage

The repo does have fuzzing and property coverage: fuzz targets exist for Markdown parse, frontmatter split, Git status parse, diff compute, and unsupported scanner; scheduled stress runs those targets for 30 minutes when available. The concern is boundary fidelity, not absence of fuzz.

Pure parser fuzzing uses bytes transformed into strings for Markdown/frontmatter/diff, while real app input goes through SourceView file loading, candidate encodings, size limits, GTK buffers, preview rendering, tags, and save/reload flows. Git status fuzz targets parser bytes, but current seed material is newline-delimited, so valid `-z` porcelain shape depends on mutations.

The named stress scripts currently under-exercise their advertised surfaces. The scheduled workflow generates Markdown corpus and Git repos, then runs stress scripts, but the stress binary opens temp files/folders and does not toggle Markdown preview, start compare, open generated Git repos, perform save/search, or assert resulting UI states. This can create false assurance around GTK criticals and boundary handling.

### Flatpak sandbox

Manifest finish-args are narrow and appropriate for a graphical GNOME editor:

- `--socket=wayland` is required for the GTK/libadwaita UI.
- `--socket=fallback-x11` is a compatibility fallback with higher display-server blast radius than Wayland, but it is the only broad-ish permission found.
- No `--filesystem=host`, `home`, `xdg-*`, `--share=network`, `--device=*`, `--talk-name`, or `--own-name` finish-args were present.

File access is portal-oriented from the UI, but once a user grants access to a project/repo path, `/app/bin/git` intentionally operates on that granted filesystem view. That is consistent with the app model; the robustness work is around Git command limits and path handling, not sandbox expansion.

Bundled Git is pinned to Git `2.54.0`, fetched from kernel.org with sha256 and verified against signed `sha256sums.asc` using a committed kernel.org autosigner key. Git is built without network-facing helpers and stripped down before install. This is a positive control.

### Supply chain/dependency pipeline

Dependency preflight is strong for the gtk-rs stack pins, safe/sys version pairing, fuzz lock sync, and Flatpak cargo-source checksums. It does not cover local patched crate drift. The local `sourceview5` patch is trusted as repo source, but it includes generated unsafe/FFI code and app-specific async/candidate-encoding modifications. A text baseline over the patched source found 1329 matches for `unsafe`, `extern "C"`, or `transmute`; V14.7 should commit that baseline as an enforceable manifest field so future drift has a comparison anchor. Current documentation explains why the patch exists; machine validation should also enforce what changed and how it stays in sync with upstream.

Baseline command: `rg -c '\bunsafe\b|\bextern\s+"C"|\btransmute\b' app/build-aux/cargo-patches/sourceview5/src -g '*.rs'`, then sum the per-file counts.

Flatpak YAML source hash enforcement is mostly adequate for the current manifest shape, but the parser is line/count based. A future complex YAML representation could satisfy global URL/hash counts without proving per-source pairing. This is low severity because current manifest lines are simple and pinned.

### GPG/OSTree/GitHub Pages signing

The release workflow uses a protected signing environment, imports the private key into a temporary `GNUPGHOME`, presets the passphrase, signs Flatpak commits and summary metadata, checks for symlinks/hardlinks, and deploys Pages with minimal `pages: write`/`id-token: write` permissions. Secrets are scoped to the build step.

The missing controls are pre-sign validation gating, monotonic remote updates, key pinning to the committed public key, and incident procedures. The README explicitly says key rotation is TBD. For a beta remote this may be acceptable temporarily, but it should be treated as release-blocking governance before wider distribution.

Read-only GitHub checks found two repository rulesets, `Protect main` and `Protect version tags`, but both are `enforcement: disabled`; classic `main` branch protection also returned `Branch not protected`. That means RIT-AUD-001 and RIT-AUD-002 currently have no active branch/tag ruleset mitigation. The disabled rulesets should be enabled after required-check and bypass review, or documented as an intentional disabled state with typed remediation.

### Policy intent vs enforcement

| Policy intent | Stated in | Current enforcement | Gap | Recommendation |
|---------------|-----------|---------------------|-----|----------------|
| Runtime Rust has no unsafe/FFI. | `AGENTS.md`, `policy/rust.policy.json:158`, `policy/validation-tooling.policy.json:289` | Scans `src/**/*.rs`, `crates/**/*.rs`, tests/benches/examples. | Local patched dependency under `build-aux/cargo-patches/sourceview5` is not covered. | Add local-patch unsafe/diff review enforcement. |
| Parser/untrusted inputs require property or fuzz tests. | `policy/rust.policy.json:265` | Coverage/tests run; fuzz targets exist. | No machine mapping from parser boundary to fuzz/property target. | Add parser-boundary registry enforced by policy. |
| GSettings writes only explicit user actions, not startup. | `policy/gsettings.policy.json:49` | Regex scans actual `.set_*` lines and local text windows. | Wrapper calls from startup/session restore are call-site blind. | Track settings wrapper calls or require typed write APIs with context labels. |
| GSettings schema artifacts must not be committed. | `policy/gsettings.policy.json:80` | Schema compile command exists. | No explicit scan for `gschemas.compiled` found in inspected checker. | Add committed-artifact check. |
| Review artifacts prove reviewed runtime/GSettings/UI exceptions. | `policy/README.md`, `tools/scanners/sites.py:141` | Line number plus substring match. | Semantic edits can retain the substring and pass. | Store stable code hashes or richer structured evidence. |
| Flatpak downloaded sources are hash-pinned. | `policy/flatpak-metadata.policy.json` | Current manifest is checked by count/regex parser. | YAML pairing can be bypassed by future complex syntax. | Use a real YAML parser or flatpak-builder manifest introspection. |
| Release signing is safe by workflow policy. | `.github/workflows/publish-flatpak.yml`, docs | Tag/version/main ancestry and protected environment. | No required validation status, monotonic version check, key pin to committed public key, or rollback policy. | Add release policy/checker and workflow gates. |
| Validators cover scoped source paths. | `policy/*.json`, `tools/validation_tooling.py:150` | `iter_files` recursively scans while excluding common dirs. | Any path component named `target` is skipped. | Exclude only known build-output roots. |
| Coverage gate proves current worktree coverage. | `tools/coverage_check.py` | Runs `cargo llvm-cov` by default. | `--json-summary` accepts stale/unrelated summaries without provenance. | Embed command/revision provenance or limit documented use. |
| GSettings migrations stay valid. | schema policy and `scripts/verify_schema_migration.sh` | CI compiles schemas. | Migration smoke is partial and not in CI. | Add migration smoke to validation and expand key coverage. |

### Save/session/concurrency

The save path is the main concurrency concern. SourceView `FileSaver` is handed the live buffer, and the UI remains editable while page loading is set. On success the code clears modified state without checking whether buffer content changed since save start. This should be fixed with a snapshot or generation guard.

Session restore intentionally suppresses persistence while `restoring_session` is true, but `finish_restore` flips the flag and immediately persists. If some restored files failed, that prunes durable session state during startup. This may be a desirable recovery behavior, but the strict GSettings policy says startup writes are forbidden and the checker cannot reason about this call path.

Same-file-in-two-windows is not fully audited by a global ownership registry. Within one workspace, `find_tab_by_file` deduplicates only that workspace's tab list. SourceView stale-save detection mitigates some cross-window overwrite cases when the same file's modification state changes on disk, but no explicit app-wide lock/owner strategy was found.

### GSettings

Schemas and wrappers are generally typed, and schema compilation is part of CI. The policy gap is semantic enforcement: regex hits on `.set_value` over-review non-GSettings calls while missing wrapper call-site context. Session/recent persistence can silently drop writes on `try_borrow_mut` contention, and migration smoke coverage is not part of CI.

Simple grep-based review found no obvious GSettings secret storage. A stronger claim would require schema-by-schema semantic review, so the main audited concern remains policy consistency and recovery behavior rather than confirmed secret exposure.

### CI fatal-criticals/ASan/Valgrind coverage

Native CI runs GTK tests under `G_DEBUG=fatal-criticals`, which is a strong baseline. The caveat is flow coverage: the scheduled stress suite does not exercise key preview/compare/Git/review/save UI transitions, so GTK criticals in those flows may remain invisible. Existing ASan/Valgrind scripts were noted in repo docs, but this audit did not run them.

The audit inspected workflow YAML but did not separately verify runner integrity beyond the jobs' visible use of GitHub-hosted Ubuntu runners. If self-hosted runners are added later, ephemerality, isolation, and attestation should become release-policy requirements.

One `policy_check --strict` run hit `SIGSEGV` while running concurrently with coverage, then passed alone on rerun. That crash was not interpreted as an app finding, but it should be tracked as validation-tooling reliability work.

## 6. Positive Assurance

- `app/src/lib.rs` forbids unsafe code at crate level, and direct grep of `app/src` did not find runtime unsafe blocks, C FFI, `static mut`, `transmute`, or unsafe attributes.
- The Git subprocess boundary uses Gio subprocess argv vectors, not a shell, and sets a restrictive Git environment.
- Non-UTF-8 Git paths are preserved as raw bytes in parser state and blocked for mutating row actions.
- Flatpak finish-args are narrow and avoid broad filesystem, network, D-Bus, and device permissions.
- Bundled Git source is pinned by sha256 and additionally checked against signed kernel.org checksums during Flatpak build.
- Fuzz and proptest infrastructure is present and active for parser layers; the finding is about coverage gaps, not absence.
- The workflow uploads stress/fuzz artifacts on failure, which helps preserve reproducer evidence.
- The publish workflow uses temporary `GNUPGHOME`, kills the GPG agent on exit, and checks Pages artifacts for symlinks/hardlinks.

## 7. Top 10 Next Actions

1. Fix `RIT-AUD-014`: make policy scanning skip only known build-output roots, not arbitrary `target` path components, and add regression coverage before adding V14.6 policies.
2. Reproduce `RIT-AUD-005` first in V14.7 save/session work; if typing during slow save confirms data loss, bump it to High and choose snapshot/generation-guard behavior before coding the fix.
3. Resolve `RIT-AUD-017`: enable `Protect main` and `Protect version tags` after required-check/bypass review, or document the disabled rulesets as intentional with typed, time-bounded remediation.
4. Gate `publish-flatpak.yml` on the exact commit's validation result, or rerun the release-critical validation suite before importing signing secrets.
5. Add monotonic version/ref checks and an explicit rollback approval path for the beta Flatpak remote.
6. Add a machine-enforced manifest for the local `sourceview5` patch: upstream checksum, allowed changed files, diff checksum, unsafe/FFI baseline, and review evidence.
7. Cancel tab IO on close/detach and require tab membership/page identity before applying open callbacks.
8. Make Git review loads owner-cancellable and guarded by tab/source generation at `finish`.
9. Add Git status entry-count/work caps and surface a degraded too-large state instead of rebuilding all GTK rows.
10. Wire stress scripts to real generated corpora/repos and assert preview, compare, Git status, save, and search UI states.
