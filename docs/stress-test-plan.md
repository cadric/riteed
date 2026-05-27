# Riteed Stress-Test Infrastructure

## Context

Riteed har god funktionel test-dækning (`gtk_tests_v4–v14`, `gtk_tests_markdown`, `ui_smoke.rs`, pure-Rust parser tests) men ingen stress-test infrastruktur: ingen boundary-tests omkring de caps appen selv annoncerer, ingen fuzz/property-dækning af parsers, ingen UI-flow-runner, ingen nightly performance/memory job. Samtidig **intenderer** `policy/rust.policy.json:261` (`property_or_fuzz_tests_required_for_parsers_and_untrusted_inputs: true`) at parsers har fuzz/property coverage, selvom `tools/policy_check.py` ikke håndhæver det maskinelt i dag.

Denne plan etablerer et lagdelt stress-setup med skarp opdeling:

**First wave (V15-pre baseline, ~1 uge, én PR)**:
1. Interne boundary unit/GTK tests pr. cap
2. `G_DEBUG=fatal-criticals` i CI efter audit
3. Proptest tier (CI-blocking, ingen workspace-ændring)

**Post-baseline (hver fase = separat PR)**:
4. Cargo-fuzz workspace-spike (først), derefter implementering
5. Separat `riteed-stress` dev binary (ikke i app-runtime)
6. Native vs Flatpak CI split
7. Manual Valgrind/ASan
8. Nightly CI orchestrering

**Ikke en milestone** (ingen V-nummer). Test-infrastruktur der bør være på plads **før V15** (large files); first wave unblocker V15-design.

## Boundary caps — pinnet eksakt adfærd

Verificeret fra koden (inclusive vs exclusive):

| Cap | Værdi | Operator | Sted |
|-----|-------|----------|------|
| `OPEN_FILE_LIMIT_BYTES` | 25 MiB | inclusive (≤) — at cap **succeeds** | `app/src/document_limits.rs:5,43` |
| `SEARCH_CHAR_LIMIT` | 2_000_000 | inclusive (≤) — at cap **succeeds** | `app/src/document_limits.rs:6` |
| `MAX_COMPARE_BYTES` | 1_000_000 | exclusive (`>`) — at cap **succeeds**, cap+1 falls back to too_large | `app/src/editor_tab/compare/diff.rs:10,122` |
| `MAX_COMPARE_LINES` | 20_000 | exclusive (`>`) | `app/src/editor_tab/compare/diff.rs:11` |
| `MAX_COMPARE_LINE_PRODUCT` | 10_000_000 | exclusive (`>`) | `app/src/editor_tab/compare/diff.rs:12` |
| `MARKDOWN_PREVIEW_MAX_BYTES` | 1_000_000 | exclusive (`>`) — at cap **renders**, cap+1 → `render_large_document_fallback` | `app/src/editor_tab/view.rs:14,257` |

Boundary tests asserter eksakt: `cap-1` succeeds, `cap` succeeds, `cap+1` fails/fallbacks.

## Stress-områder dækket

- File load/save: encoding edge cases, line endings, NUL bytes, BOM, invalid UTF-8, 25 MiB boundary
- Buffer/rendering: meget lange linjer, mange linjer, emoji+combining, RTL/LTR mix, hurtig insert/delete, undo/redo, scroll cycling, zoom
- Search: 2M char boundary, regex-lignende patterns, unicode, replace-all på mange matches
- Compare/diff: caps respect, asymmetric diffs, presentation buffer integrity
- Markdown preview: stor frontmatter, defekt YAML, dybe lister, mange images, raw HTML, store code blocks, CRLF
- Folder sidebar / project tree: 10k+ filer, dybe mapper, refresh under churn, permission-denied, symlinks, lange/non-UTF-8 filnavne
- Git/source control: 5k untracked/modified, conflict, detached HEAD, manglende identity, huge status, non-UTF-8 paths, index.lock
- **Window/session multiplication**: `HANDLES_OPEN` med mange filer (`app.rs:332`), secondary windows
- **Project tree monitor under churn**: `project_tree_monitor.rs:66` `g_warning!` site under rapid create/rename/delete
- **GSettings write storms**: 13 settings-moduler binder keys; rapid preference toggling
- **Compare review session reveal**: `review_session_reveal.rs` navigation across mange hunks
- **Sourceview language detection**: mange ext, store pasted content
- **Dialog modal stack**: nested modals (mangler fra `gtk_tests_dialog_lifecycle.rs`)

---

## First wave (én PR, ~1 uge)

### Phase 1: Interne boundary tests + corpus generator

**Mål**: pin hver cap eksakt; deterministisk corpus. Hold almindelig `cargo test` let.

**Layering** (rettet fra forrige plan-udkast):

Pure cap-tests skal ligge **i modulet hvor cap'en bor**, ikke i et separat GTK-test-modul. Private helpers som `file_size_supports_open` og `char_count_supports_search` i `document_limits.rs` er allerede private og kan **ikke** ses udefra — fix er at lægge tests i samme modul, ikke at exporte helpers.

- **Pure cap tests** (hurtige, ingen GTK): `#[cfg(test)] mod tests` i modulet selv. Test caller den private helper direkte med byte-/char-counts, ikke faktiske buffers.
  - `document_limits.rs::tests` — `file_size_supports_open` + `char_count_supports_search` cap tests (size-tal, ikke ægte filer)
  - `editor_tab/compare/diff.rs::tests` — `compute_diff_with_options` cap tests med konstruerede inputs (eksisterende test-modul udvides)
  - `editor_tab/view.rs::tests` — Markdown preview fallback predicate test. Tilføj en lille private helper `markdown_preview_uses_fallback(len: usize) -> bool` der wrapper `len > MARKDOWN_PREVIEW_MAX_BYTES`-checket fra `view.rs:257`, og test den lokalt i samme module
- **GTK boundary smoke** (1-2 tests max, faktisk store buffers): `app/src/gtk_tests_boundaries.rs` med kun "open 25 MiB" + "search 2M chars" end-to-end, der beviser flows ikke blot returner-tal stemmer. Disse to tests bruger eksisterende helpers.

**Tung-test-disciplin**: pure cap tests kører på pure-Rust niveau (millisekunder). Kun 1-2 GTK tests åbner faktisk en 25 MiB buffer. Almindelig `cargo test` må ikke blive målbart langsommere af denne fase.

**Nye filer**:
- `app/src/gtk_tests_boundaries.rs` — kun de 1-2 end-to-end GTK smokes
- `stress/make_corpus.py` — Python corpus generator (committed)
- `stress/corpus/seeds/` — committed small-fil seeds (<100 KB total) brugt af GTK smokes
- `stress/corpus/.gitkeep` — directory placeholder
- `.gitignore` udvidet med `stress/corpus/*` undtagen `seeds/` og `.gitkeep`

**Modificerede filer**:
- `app/src/lib.rs` — tilføj `#[cfg(test)] mod gtk_tests_boundaries;`
- `app/src/document_limits.rs` — udvid `#[cfg(test)] mod tests` med pure cap tests
- `app/src/editor_tab/compare/diff.rs` — udvid eksisterende `#[cfg(test)] mod tests` med cap tests
- `app/src/editor_tab/view.rs` — tilføj private `markdown_preview_uses_fallback(len: usize) -> bool` helper + `#[cfg(test)] mod tests` for cap-predicate

**Boundary tests per cap**:
- Pure tier: `_at_minus_one_returns_ok`, `_at_exact_returns_per_contract`, `_at_plus_one_returns_too_big_error`
- GTK tier: én smoke pr. de to mest end-to-end-kritiske caps (open file, search)

**Reuse**: `init_gtk_for_tests`, `build_window`, `spin_until`, `write_temp_file` — kun i de 2 GTK smokes.

### Phase 2: `G_DEBUG=fatal-criticals` i CI (efter audit)

**Mål**: GTK criticals fail tests i stedet for at blive silently logged.

**Pre-flight audit** (kritisk):
- `rg -n "g_critical!|g_warning!" app/src` — kortlæg alle call sites
- Verificér `lib.rs:160` (resource registration critical) ikke trigges i tests
- Verificér `project_tree_monitor.rs:66` warning ikke promoveres (fatal-criticals fatalizer kun criticals, ikke warnings)
- Fix eller `g_log_set_handler`-undertryk eventuelle ægte test-side criticals **før** flag flips

**Modificeret fil**:
- `.github/workflows/validate.yml:29-32` — tilføj `G_DEBUG: fatal-criticals` til env block (naturligt sted ved siden af eksisterende `GSK_RENDERER`, `GTK_A11Y`, `NO_AT_BRIDGE`, `RUST_TEST_THREADS`)

**Rollout**: `fatal-criticals` først; opgradér til `fatal-warnings` som separat follow-up.

**Verifikation**: lokal/CI dry-run på temporary branch — indfør deliberat GTK critical i test path, bekræft CI fejler, revert lokalt. **Ikke** en commit-step.

### Phase 3a: Proptest tier (CI-blocking, ingen workspace-ændring)

**Mål**: lukker policy-intention `policy/rust.policy.json:261`. Ingen build-arkitektur-ændring.

**Vigtigt**: dette er policy-**intention**, ikke maskin-håndhævet gate. `tools/policy_check.py` håndhæver den ikke i dag. Phase 3a lukker intentionen ved at levere coverage; en separat opgave (Post-baseline) kan tilføje håndhævelse i `policy_check.py`.

**Modificerede filer**:
- `app/Cargo.toml` — tilføj `proptest` som `[dev-dependencies]` (ikke `[dependencies]` — ingen production impact)
- Hver parser's `tests` modul (`markdown::parser_tests`, `markdown::frontmatter::tests`, `markdown::unsupported::tests`, `git_status::tests`, `editor_tab::compare::diff::tests`) udvides med proptest-baserede tests

**Nye proptest-tests** (én pr. parser):
- `markdown::parser_tests::proptest_parse_document_no_panic`
- `markdown::frontmatter::tests::proptest_split_terminates`
- `markdown::unsupported::tests::proptest_diagnostics_terminate`
- `git_status::tests::proptest_porcelain_v2_robust`
- `editor_tab::compare::diff::tests::proptest_compute_diff_respects_caps`

Hver test asserter: ingen panic, ingen unbounded loop, output respekterer caps.

**Tight proptest config** (kritisk for CI-velocity og CI-stabilitet):
- `ProptestConfig::with_cases(64)` per test (ikke default 256) — hurtigere PR-CI
- `failure_persistence: FileFailurePersistence::SourceParallel(".proptest-regressions")` — committed regression seeds for repro
- `.proptest-regressions/` mappes per parser. **Initial state committed som tomt directory med `.gitkeep`** plus en kort `app/src/markdown/.proptest-regressions/README.md` (eller tilsvarende per modul) der dokumenterer at filer dér er auto-genererede regression-seeds fra historiske failures. Det forhindrer at proptest committer skæve auto-genererede filer uden kontekst.
- **Input-size disciplin pr. strategy** (kritisk for markdown/diff der ellers kan generere kæmpe inputs):
  - Markdown body: `prop::collection::vec(any::<u8>(), 0..4096)` — max 4 KB
  - Frontmatter input: max 1 KB
  - Git status output: max 16 KB
  - Diff inputs: hvert side max 8 KB lines × 256 chars
  - Tilføj eksplicit `prop_assume!` for inputs der overskrider stress-relevante bounds — sparer CI fra arbitrære megabyte-inputs

**Cargo.lock + Flatpak cargo sources**: proptest som dev-dependency opdaterer `Cargo.lock`. Hvis Flatpak manifestet bruger en lockfile-baseret cargo-sources-fil (`app/build-aux/cargo/cargo-sources.json` eller tilsvarende), skal den enten regenereres eller bekræftes uændret. Verifikation:
- `cd app && cargo update --workspace --dry-run` for at se omfanget af lockfile-ændringer
- `flatpak-builder --show-manifest app/build-aux/io.github.cadric.Riteed.yml` for at bekræfte manifest stadig renderer
- Lokal Flatpak build hvis cargo-sources fil ændrer sig: `flatpak-builder --user --install --force-clean app/build-dir app/build-aux/io.github.cadric.Riteed.yml`

---

## Post-baseline phases (hver = separat PR)

### Phase 3b: Cargo-fuzz design spike (FIRST), derefter implementation

**Spike-mål** (leverer rapport, ingen kode):

Undersøg at workspace-konvertering ikke bryder stable PR gates. Konkrete fund som spike skal levere:

1. **Kan `app/fuzz/` ekskluderes fra default `--workspace` operations?** Verificér:
   - `cargo check --workspace --all-targets --all-features` (per `policy/validation-tooling.policy.json:142`) ikke bygger `app/fuzz/`
   - `tools/coverage_check.py:79` workspace llvm-cov ikke ser `app/fuzz/`
   - Mulighed: `[workspace] default-members = ["app"]` ekskluderer `app/fuzz/` fra default scope; `--workspace` overrider dog typisk default-members
   - Alternativ: `app/fuzz/` som **separat workspace** (eget `[workspace]` root) ikke linked til `app/`
2. **Kan `app/fuzz/` pinne nightly rust uden at påvirke `app/`?** Verificér `rust-toolchain.toml` scope (per-crate vs per-workspace)
3. **Kan libFuzzer-sys bygges i Flatpak runtime?** Eller skal cargo-fuzz være host-only?

**Spike-output**: kort markdown-rapport i `.agent/design-spikes/cargo-fuzz-workspace.md` med konkret konklusion + anbefalet konfiguration. **Ingen** kode i denne spike.

**Hard rule**: cargo-fuzz implementation må ikke starte før spike har bevist at stable PR gates ikke poisones. Hvis spike konkluderer at det ikke kan ekskluderes pålideligt, drop cargo-fuzz; proptest-only accepteres som final state.

**Implementation** (efter spike godkendt, separat PR).

Baseret på spike-konklusion. Hvis spike viser separat workspace er bedste valg:
- `app/fuzz/Cargo.toml` med egen `[workspace]` root (ikke linked til `app/`)
- `app/fuzz/rust-toolchain.toml` pinner nightly
- `app/fuzz/fuzz_targets/`: `markdown_parse.rs`, `frontmatter_split.rs`, `git_status_parse.rs`, `diff_compute.rs`, `unsupported_scanner.rs`
- `app/fuzz/.gitignore` — ignore `target/`, `corpus/`, `artifacts/`
- `app/fuzz/corpus/<target>/` — deterministiske seed inputs committed for repro
- Eksklusion fra stable gates verificeret i CI-config

### Phase 4: Separat `riteed-stress` dev binary

**Mål**: stress runner som **separat binary**, ikke runtime path i `riteed`-appen.

**Tekniske krav** (rettet fra original plan):
- IKKE en `cfg(debug_assertions)`-gated module i `app/src/`. Det er stadig en hidden runtime path i selve appen via env var.
- Separat binary target i `app/Cargo.toml` `[[bin]]` med `required-features = ["stress"]`
- Feature `stress` defineret i `app/Cargo.toml` `[features]` — ikke i `default = [...]`, ikke aktiveret i Flatpak manifest build
- Binary'en bygges kun med `cargo build --bin riteed-stress --features stress`
- Aldrig i Flatpak release artifact

**Nye filer**:
- `app/src/bin/riteed_stress.rs` — separat binary main
- `stress/scripts/open-save-search.json`
- `stress/scripts/compare-roundtrip.json`
- `stress/scripts/markdown-stress.json`
- `stress/scripts/git-status-stress.json`

**Modificerede filer**:
- `app/Cargo.toml` — tilføj `[features] stress = []` og `[[bin]] name = "riteed-stress" path = "src/bin/riteed_stress.rs" required-features = ["stress"]`

**Helper-sharing design** (vigtigt — løser cfg(test) blocker):

Stress-binary'en kan ikke se `#[cfg(test)] pub(crate)` helpers. To muligheder:
1. **Stress-binary initialiserer GTK selv** og bruger public GApplication API til at åbne filer + drive actions. Den ER appen, behøver ikke test-helpers. Enkleste vej.
2. **Feature-gated test-support API**: ændr `init_gtk_for_tests`/`spin_until` til `#[cfg(any(test, feature = "stress"))] pub(crate)`. Mere genbrug men cementerer "stress" som compile-time flag der berører hovedcrate.

Anbefaling: **#1** — stress-binary er et separat program der driver appen via public API. Holder hovedcrate ren.

### Phase 5: Native vs Flatpak CI split

**Mål**: fang `/usr/bin/git` (test) vs `/app/bin/git` (Flatpak prod) divergens.

**Nye filer**:
- `stress/git-repos/make_repos.sh` — genererer stress-repos: `many-untracked`, `many-modified`, `conflicted`, `non-utf8-paths`, `huge-status`, `submodule-and-symlink`, `missing-identity`, `index-lock-present`

**Modificerede filer**:
- `.github/workflows/validate.yml` — split test job:
  - `native-tests` (eksisterende, dækker `/usr/bin/git` via cfg(test) i `app/src/git_process/support.rs:1-4`)
  - `flatpak-tests` (ny, builder Flatpak, runner `riteed-stress` mod `/app/bin/git`)
- Optional: `app/build-aux/io.github.cadric.Riteed.yml` — `run-tests: true` block med `test-commands` for in-manifest stress smoke

### Phase 6: Manual Valgrind/ASan (nightly, ikke per-PR)

**Mål**: catch leaks og UB på tight flows.

**Nye filer**:
- `stress/scripts/valgrind-smoke.sh` — `flatpak install --include-sdk --include-debug ...`, drop i `flatpak run --command=sh --devel ...`, `valgrind --leak-check=full`
- `stress/scripts/asan-smoke.sh` — native nightly med `RUSTFLAGS="-Z sanitizer=address"`, kun pure-Rust tests (ikke GTK; ASan + libadwaita ustabil)

### Phase 7: Nightly CI orchestrering

**Mål**: orkestrér post-baseline phases i ét nightly job.

**Modificerede filer**:
- `.github/workflows/validate.yml` — tilføj `stress` job triggered på `schedule:` eller `workflow_dispatch:`, separat fra PR-blocking `test` job

**Steps**: genér corpus → boundary tests med `G_DEBUG=fatal-criticals` → proptest suite → cargo-fuzz (30 min/target) → Flatpak build → `riteed-stress` mod hvert script → upload artifacts ved fejl.

---

## Kritiske filer

| Path | Rolle | Action |
|------|-------|--------|
| `app/src/document_limits.rs:5,6,43` | OPEN/SEARCH caps, inclusive check | Pin i `document_limits.rs::tests` (module-local) |
| `app/src/editor_tab/compare/diff.rs:10-12,122` | Compare caps, exclusive checks | Pin i `editor_tab/compare/diff.rs::tests` (module-local) + diff fuzz target |
| `app/src/editor_tab/view.rs:14,257` | MARKDOWN_PREVIEW_MAX_BYTES, exclusive | Add private `markdown_preview_uses_fallback(len)` helper + pin i `editor_tab/view.rs::tests` (module-local) |
| `app/src/gtk_tests_boundaries.rs` (ny) | End-to-end GTK smokes (kun 1-2) | Faktisk åbn 25 MiB + faktisk søg 2M chars |
| `app/src/git_process/support.rs:1-4` | Git binary cfg(test) split | Flatpak job exercises /app/bin/git (Phase 5) |
| `app/src/lib.rs:204-227` | `init_gtk_for_tests` / `lock_for_tests` (`pub(crate) #[cfg(test)]`) | Reuse internt i de 2 GTK smokes |
| `app/src/gtk_tests.rs:20-73` | `spin_until`, `drain_events`, `wait_millis`, `build_window`, `write_temp_file` | Reuse internt i de 2 GTK smokes |
| `app/src/lib.rs:160` | g_critical! site | Audit før G_DEBUG flip |
| `app/src/project_tree_monitor.rs:66` | g_warning! site | Audit |
| `app/src/app.rs:53` | GApplicationFlags (HANDLES_OPEN) | Bekræft separat-binary-pattern OK i Phase 4 |
| `.github/workflows/validate.yml:29-32` | CI env block | Add G_DEBUG i Phase 2 |
| `policy/rust.policy.json:261` | property_or_fuzz_tests intention | Lukkes af Phase 3a coverage |
| `policy/validation-tooling.policy.json:142` | `cargo check --workspace --all-targets --all-features` | Spike skal verificere fuzz-crate ekskluderet |
| `tools/coverage_check.py:79` | workspace llvm-cov | Spike skal verificere fuzz-crate ekskluderet |
| `app/Cargo.toml` | App crate | Add proptest dev-dep (Phase 3a), `[[bin]] riteed-stress` med `required-features = ["stress"]` (Phase 4) |
| `app/Cargo.lock` | Lockfile | Bekræft kun proptest-add i Phase 3a; flatpak cargo sources opdateres hvis nødvendigt |
| `app/build-aux/io.github.cadric.Riteed.yml` | Flatpak manifest | Optional run-tests block (Phase 5) |

## Verifikation

**Phase 1**: `cd app && cargo test gtk_tests_boundaries -- --test-threads=1` grøn; `stress/corpus/seeds/` committed; corpus selv gitignored.

**Phase 2**: indfør deliberat GTK critical i test path → CI fejler → revert før merge.

**Phase 3a**: `cd app && cargo test proptest_ -- --test-threads=1` grøn; CI-tid stiger ikke mere end ~30 sekunder; `.proptest-regressions/` committed med tomme placeholders + README.

**First wave samlet validation** (køres før PR åbnes):
- `cd app && cargo fmt --all --check`
- `cd app && cargo check --workspace --all-targets --all-features`
- `cd app && cargo test --workspace --all-targets --all-features`
- `python3 -m tools.policy_check --root app --strict`
- `python3 -m tools.coverage_check --root app`
- `git diff --check`
- `flatpak-builder --show-manifest app/build-aux/io.github.cadric.Riteed.yml` (kun hvis cargo sources skal regenereres efter proptest-add)

**Phase 3b spike**: rapport i `.agent/design-spikes/cargo-fuzz-workspace.md` der konkluderer hvilken workspace-konfiguration der ekskluderer fuzz fra stable gates. Brugeren godkender før implementation-PR åbnes.

**Phase 3b implementation**: `cd app/fuzz && cargo +nightly fuzz run markdown_parse -- -max_total_time=60` for hvert target uden crash; `cd app && cargo check --workspace --all-targets --all-features` og `python3 -m tools.coverage_check --root app` passerer (beviser fuzz-crate ekskluderet).

**Phase 4**: `cargo build --bin riteed-stress --features stress && RITEED_STRESS_SCRIPT=stress/scripts/open-save-search.json ./target/debug/riteed-stress` exits 0 på happy path, exits 1 på intentional failure. `cargo build` uden `--features stress` bygger ikke binary'en.

**Phase 5**: deliberately broken `/app/bin/git` invocation reproducerer i Flatpak job men ikke native.

**Phase 6**: Valgrind smoke viser ingen `definitely lost` blocks på basic open-save-close flow.

**Phase 7**: nightly CI kører uden manual trigger.

## Scope-disciplin

- Stress-infrastruktur i `stress/` (repo root). Fuzz crate i `app/fuzz/` (ekskluderet fra stable workspace per spike-konklusion).
- **Phase 4 stress runner er separat binary**, ikke runtime path i `riteed`-app. `required-features = ["stress"]` sikrer aldrig i Flatpak release.
- Fuzz targets tester pure-Rust modules — ingen GTK init i fuzz.
- AGENTS.md 600-line limit pr. ny Rust-fil.
- `proptest` som `[dev-dependencies]` i `app/Cargo.toml`. `app/fuzz/` må tilføje `libfuzzer-sys` og `arbitrary` i sin egen Cargo.toml.
- Generated corpus gitignored. Generator + small seeds committed.
- Stress runner FS-probes via async Gio paths (per AGENTS runtime-sync-fs rule).
- Fuzz crashes med deterministisk repro seed for CI triage.
- `tight proptest configs` (64 cases, persistence file) — ellers CI-langsom.
- "Closes policy-intention" formulering, ikke "closes hard gate" — Phase 3a leverer coverage; håndhævelse i policy_check.py er separat opgave.

## Antagelser

- 25 MiB / 2M chars / 1 MiB / 20k lines caps stabile i V14.5/V15-pre. Hvis V15 ændrer dem, stress tests opdateres med V15.
- Boundary tests kører som del af `cargo test` (interne), ikke gated bag feature flag.
- Cargo-fuzz nightly targets er opt-in nightly, ikke per-PR.
- Stress runner JSON flows er developer tool, ikke user-facing feature.
- Phase 3b spike kan konkludere "cargo-fuzz ikke værd at konfigurere som workspace member" — i så fald accepteres proptest-only og cargo-fuzz droppes.

## Rollout-rækkefølge

1. **First wave PR**: Phase 1 + 2 + 3a som én PR. Unblocker V15-design.
2. **Phase 3b spike**: ren rapport, godkendelse fra dig før implementation-PR åbnes. Hvis spike viser cargo-fuzz ikke kan ekskluderes fra stable gates: stop her, proptest-only er final state.
3. **Phase 3b implementation, Phase 4, Phase 5**: separate PRs, kan parallelliseres efter first wave er merged.
4. **Phase 6 + 7**: sidste, infrastruktur-orkestrering.

Hver post-baseline fase er uafhængigt shippable og kan stoppes/skubbes uden at blokere andet arbejde.
