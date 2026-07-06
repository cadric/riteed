````text
You are acting as an external senior code auditor for Riteed, a lightweight Rust-native GNOME text editor.

This is an audit-only assignment. Do not implement fixes unless explicitly asked later. Do not rewrite architecture. Do not make broad formatting-only changes. Do not delete files. Do not run destructive commands. Do not use network access unless a specific dependency/documentation lookup is necessary and you record exactly what you used it for.

Your goal is to produce a broad, evidence-backed audit of this repository covering:
- bug hunting
- data-loss risks
- correctness issues
- async/lifecycle races
- UI/UX defects
- performance and responsiveness problems
- dead code and unused paths
- over-complexity and maintainability issues
- missing tests
- fuzz/stress gaps
- Flatpak/sandbox issues
- dependency and supply-chain concerns
- release/CI/policy enforcement gaps
- documentation drift
- small, high-leverage improvements

The final output must be a comprehensive audit report in Danish. Keep file paths, symbols, command names, Rust identifiers, and error messages in their original language.

Repository context:
- The app lives under `app/`.
- The repo also contains policy, validation tooling, stress/fuzz infrastructure, Flatpak packaging, release workflows, and previous audit artifacts.
- Treat `AGENTS.md` as the repo contract.
- Treat `.agent/CONTINUITY.md` as continuity/context only, not policy.
- Treat `policy/*.json`, `policy/README.md`, and `tools/` as part of the enforceable validation system.
- Existing audit/history docs may be stale; verify claims against current code before relying on them.

First, read these files before auditing anything else:
1. `EXTERNAL-REVIEW-HANDOFF.txt`
2. `AGENTS.md`
3. `README.md`
4. `ROADMAP.md`
5. `CHANGELOG.md`
6. `.agent/CONTINUITY.md`
7. `docs/mangler-og-bugs.md`
8. `docs/audit.md`
9. `docs/audit_report.md`
10. `policy/README.md`
11. `app/Cargo.toml`
12. `app/fuzz/Cargo.toml`
13. `.github/workflows/validate.yml`
14. `.github/workflows/publish-flatpak.yml`
15. `app/build-aux/io.github.cadric.Riteed.yml`

Important audit rule:
Do not report a finding unless you can support it with evidence. Evidence can be:
- exact file path and line references
- quoted code snippets
- command output
- a minimal reproduction
- a failing test you added locally for confirmation
- a clear control-flow/data-flow explanation grounded in current code
- a comparison between documented policy and actual enforcement

Speculative observations are allowed only in a separate section called “Ikke-verificerede observationer / kræver runtime-test”. They must not be mixed with confirmed findings.

Work method:

Phase 1 — Baseline and inventory
- Identify repo layout and major subsystems.
- Summarize the current architecture in 1–2 pages:
  - app/window/workspace/document model
  - editor tabs and close/save/open flow
  - Markdown preview/parser/rendering
  - compare/diff subsystem
  - large-file viewer
  - project tree / file browser
  - source control / Git process boundary
  - find/replace and find-in-files
  - settings/GSettings
  - Flatpak packaging
  - policy/validation tooling
  - fuzz/stress tests
- Record exact commit/source state from handoff if present.
- Record toolchain assumptions from repo files.
- Note any missing referenced files or stale handoff pointers.

Phase 2 — Run safe validation commands
Run what is available in the environment. If a command cannot run due to missing system packages, record that honestly and continue with static review.

From repo root:
```bash
python3 -m tools.policy_check --root app --strict
python3 -m tools.coverage_check --root app
scripts/dependency-preflight --root app
scripts/integration-preflight
git diff --check
````

From `app/`:

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
GTK_A11Y=none GSK_RENDERER=cairo G_DEBUG=fatal-criticals cargo test --workspace --all-targets --all-features
```

Metadata checks, if tools exist:

```bash
glib-compile-schemas --strict --dry-run app/data/schemas
msgfmt --check-format --check-header -o /dev/null app/po/*.po
desktop-file-validate app/data/io.github.cadric.Riteed.desktop
appstreamcli validate --no-net --pedantic app/data/io.github.cadric.Riteed.metainfo.xml
flatpak-builder --show-manifest app/build-aux/io.github.cadric.Riteed.yml
```

Fuzz/stress checks, if available:

```bash
cd app/fuzz
cargo fuzz list
cargo +nightly fuzz run markdown_parse -- -runs=1000
cargo +nightly fuzz run frontmatter_split -- -runs=1000
cargo +nightly fuzz run git_status_parse -- -runs=1000
cargo +nightly fuzz run diff_compute -- -runs=1000
cargo +nightly fuzz run unsupported_scanner -- -runs=1000
```

Do not treat a passing validation suite as proof that no bugs exist.

Phase 3 — Independent audit tracks
Run these as separate passes. If using subagents, assign one track per subagent, then merge and deduplicate findings.

Track A — Data loss and document lifecycle
Review:

* `app/src/document*.rs`
* `app/src/editor_io.rs`
* `app/src/editor_tab/open.rs`
* `app/src/editor_tab/save.rs`
* `app/src/editor_tab/state.rs`
* `app/src/editor_tab/runtime.rs`
* `app/src/workspace*.rs`
* `app/src/close_flow.rs`
* `app/src/workspace_close.rs`
* `app/src/workspace_open.rs`
* `app/src/workspace/autosave.rs`
* `app/src/editor_monitor*`

Look for:

* stale async callbacks applying to closed/reused tabs
* save-after-edit races
* dirty-state loss
* file identity confusion
* session restore overwriting state
* autosave surprises
* external modification detection gaps
* encoding/line-ending bugs
* cancellation gaps
* “close window” vs “close tab” behavior divergence
* data loss after failed save, failed reload, or failed open

Track B — Search, replace, large files, and performance
Review:

* `app/src/editor_search/**`
* `app/src/find_in_files/**`
* `app/src/large_file/**`
* `app/src/editor_tab/large_file.rs`
* `app/src/document_limits.rs`

Look for:

* UI-thread blocking
* unbounded memory use
* O(n²) behavior on large files/projects
* search count drift
* replace-all edge cases
* regex/literal/case-sensitivity bugs if applicable
* binary/invalid UTF-8 issues
* very long line behavior
* project search following symlinks unexpectedly
* directory skip bugs
* cancellation behavior when user changes tabs/projects

Track C — Markdown, parser boundaries, and untrusted input
Review:

* `app/src/markdown/**`
* `app/src/editor_tab/markdown_preview.rs`
* `app/fuzz/fuzz_targets/**`
* `app/fuzz/corpus/**`
* `policy/stress-fuzz.policy.json`
* `app/build-aux/validation/parser-boundaries.v1.json`

Look for:

* panics on malformed Markdown/YAML/frontmatter
* invalid UTF-8 or lossy boundary bugs
* raw HTML/image handling mismatches
* preview/source state divergence
* print/export behavior gaps
* parser normalization bugs
* fuzz target coverage gaps
* stale corpus or missing regression seeds
* patched `pulldown-cmark` drift or risk

Track D — Compare/diff/source control/Git
Review:

* `app/src/editor_tab/compare/**`
* `app/src/window_compare*.rs`
* `app/src/source_control/**`
* `app/src/git_process/**`
* `app/src/git_status.rs`
* `app/build-aux/git/**`

Look for:

* path escaping/display bugs
* raw bytes vs displayed path confusion
* stale Git status after root change
* stage/unstage/discard safety
* destructive action confirmation gaps
* index.lock handling gaps
* Git subprocess timeout/cancellation bugs
* repo root confusion
* symlink/path traversal surprises
* too-large status behavior
* duplicated tabs or state mismatch between editor/compare/source-control
* diff rendering correctness issues
* unified vs split view inconsistency

Track E — GTK/libadwaita UI lifecycle, accessibility, and UX
Review:

* `app/src/window*.rs`
* `app/src/app*.rs`
* `app/src/dialog*.rs`
* `app/src/sidebar_host.rs`
* `app/data/ui/*.ui`
* `app/data/ui/*.css`
* `app/data/schemas/*.xml`
* `app/po/*.po`
* `app/data/io.github.cadric.Riteed.metainfo.xml`
* `app/data/io.github.cadric.Riteed.desktop`

Look for:

* widget lifetime/callback bugs
* stale weak refs
* action state drift
* shortcuts not wired
* modal/dialog lifecycle problems
* gettext bypass
* missing translator comments where needed
* GSettings schema drift
* accessibility label gaps
* HIG/libadwaita violations
* inconsistent active-file/dirty-state indicators
* CSS that fights Adwaita or theme changes

Track F — Security, sandbox, release, and supply chain
Review:

* `app/build-aux/io.github.cadric.Riteed.yml`
* `app/build-aux/cargo/cargo-sources.json`
* `app/build-aux/cargo-patches/**`
* `app/Cargo.lock`
* `app/fuzz/Cargo.lock`
* `.github/workflows/*.yml`
* `.github/dependabot.yml`
* `tools/checks/dependency_preflight.py`
* `tools/checks/release*.py`
* `policy/release.policy.json`
* `policy/flatpak-metadata.policy.json`
* `policy/rust.policy.json`

Look for:

* broad Flatpak permissions
* host filesystem assumptions
* portal bypasses
* bundled Git trust boundary bugs
* floating refs or unpinned actions
* release signing/rollback gaps
* artifact poisoning between jobs
* stale generated cargo sources
* lockfile drift
* patched crate manifest drift
* duplicate dependency versions
* unsafe/FFI boundary risk
* CI checks that can be bypassed
* policy intent that is not enforced

Track G — Dead code, maintainability, and simplification
Review all `app/src/**/*.rs` plus tests.

Look for:

* unused functions/types/modules
* duplicated helper logic
* old versioned test files that can be merged
* unreachable branches
* state structs carrying unused fields
* too-large modules near policy limits
* excessive cloning or string allocation
* avoidable coupling between window/workspace/editor/source-control
* code that exists only to satisfy stale tests
* test-only helpers accidentally in production paths

Use commands such as:

```bash
rg "TODO|FIXME|HACK|XXX|unwrap|expect|panic!|todo!|unimplemented!|dbg!|unsafe" app/src tools policy .github
rg "allow\\(|allow\\[" app/src app/Cargo.toml
rg "clone\\(|to_string\\(|to_owned\\(" app/src
rg "spawn|Command|Subprocess|filesystem|read_to_string|metadata\\(" app/src tools
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Do not assume every `clone()` is bad. Only report it when you can explain impact and a concrete safer/simpler alternative.

Finding format:
Every confirmed finding must use this exact structure:

```markdown
### RIT-GEN-XXX — Short title

- Severity: Critical | High | Medium | Low | Info
- Confidence: Confirmed | High | Medium
- Category: correctness | data-loss | performance | security | sandbox | tests | dead-code | maintainability | UX | docs | policy
- Status: confirmed finding | improvement | dead-code candidate | policy gap
- Files:
  - `path/to/file.rs:line-line`
- Evidence:
  - Quote the relevant code or command output.
  - Explain the control flow or data flow.
- Why this matters:
  - User impact / developer impact / release impact.
- Reproduction:
  - Exact steps, command, test case, or minimal scenario.
  - If runtime reproduction was not possible, explain why and give the static proof.
- Suggested fix:
  - Smallest safe fix.
  - Mention any policy/docs/tests that must be updated.
- Suggested regression test:
  - Unit/integration/GTK/fuzz/stress/policy test to prevent recurrence.
- Risk of fix:
  - Low | Medium | High, with reason.
```

Severity guide:

* Critical: credible data loss, sandbox escape, release signing compromise, arbitrary command execution, or severe security issue.
* High: likely user-visible correctness bug, serious stale async lifecycle bug, large data corruption risk, or meaningful supply-chain/release weakness.
* Medium: real bug with bounded impact, performance cliff, missing enforcement, or important test gap.
* Low: minor UX, maintainability, docs drift, small inefficiency.
* Info: cleanups, dead-code candidates, observations without immediate user impact.

Report requirements:
The final report must contain:

1. Executive summary

   * 5–10 bullets.
   * Top risks first.
   * Number of findings by severity and category.

2. Scope and method

   * Files/subsystems reviewed.
   * Commands run.
   * Commands not run and why.
   * Environment limitations.

3. Architecture summary

   * Short explanation of how the app is structured.
   * Note the highest-risk subsystem boundaries.

4. Coverage matrix
   Table with columns:

   * Area
   * Files reviewed
   * Checks run
   * Result
   * Confidence

5. Findings overview table
   Columns:

   * ID
   * Severity
   * Category
   * Title
   * Primary file
   * Confidence
   * Fix size estimate

6. Detailed findings
   Use the exact finding format above.

7. Dead code and simplification inventory
   Separate confirmed dead code from candidates.
   Do not recommend deletion unless usage was checked.

8. Performance opportunities
   Include only opportunities with code evidence or plausible measured impact.
   Prefer small, safe changes.

9. Test/fuzz/stress gaps
   Include concrete proposed tests and target files.
   Prioritize tests for data-loss, parser, Git status, save/open lifecycle, and large-file behavior.

10. Policy and validation gaps
    Separate:

* policy intent exists but enforcement missing
* enforcement exists but bypassable
* enforcement exists and appears adequate

11. Documentation drift
    Include stale docs, missing docs, and handoff inconsistencies.

12. Remediation plan
    Split into:

* Must fix before next public beta
* Should fix soon
* Opportunistic cleanup
* Needs manual UX/runtime verification

13. Appendix

* command log
* grep queries used
* files not reviewed
* assumptions
* false positives dismissed

Quality bar:

* Be adversarial but fair.
* Prefer 10 real findings over 50 speculative ones.
* Do not pad the report.
* Do not praise the repo unless it explains risk or scope.
* Do not claim a command passed unless you actually ran it.
* Do not claim a file was reviewed unless you read it.
* If you find no issue in an area, say what you checked and why confidence is limited.
* When in doubt, downgrade confidence instead of overstating.

Final deliverables:

1. `AUDIT_REPORT.md` content in Danish.
2. A concise `findings.json` representation with:

   * id
   * severity
   * confidence
   * category
   * title
   * files
   * summary
   * reproduction
   * suggested_fix
   * suggested_test
3. A short “next prompt” for a follow-up fixing agent, but do not implement fixes.

````

```text
Use multiple independent audit passes before writing the final report.

Suggested subagent split:
1. Data-loss/open/save/session lifecycle
2. GTK/UI/action/dialog lifecycle
3. Markdown/parser/fuzz/large-file handling
4. Git/source-control/compare/diff
5. Flatpak/release/supply-chain/policy enforcement
6. Dead-code/performance/maintainability

Each subagent must return:
- reviewed files
- confirmed findings only
- suspected-but-unconfirmed issues separately
- commands run
- confidence level
- top 3 recommended fixes

The coordinator must deduplicate findings, verify evidence, assign severities consistently, and produce one final Danish report. Do not let subagents directly edit files.
````

