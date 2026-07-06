---
created: 2026-07-06
status: complete
type: external-audit-report
auditor: ekstern kode-auditor (Fable-audit)
commit: 7276d9158bd5ff48d2961ecc4ca13ee076c97402
branch: main
app_version: 0.3.7
---

# Riteed — Ekstern kode-audit (2026-07-06)

Sprog: dansk. Fil-stier, symboler, kommandonavne, Rust-identifikatorer og
fejlmeddelelser er bevaret på originalsproget.

Dette er en **audit-only**-leverance. Der er ikke implementeret rettelser, ikke
ændret arkitektur, ikke slettet filer og ikke kørt destruktive kommandoer.

---

## 1. Resumé (executive summary)

- **Ét kritisk fund.** Source Control sender rå filnavne som Git-pathspecs uden
  `GIT_LITERAL_PATHSPECS`, så *discard*/*unstage*/*stage* på filer med
  glob-tegn (`[`, `*`, `?`, `:`) rammer forkerte filer eller korrumperer
  indexet — reel, tavs datatab-vej (`RIT-GEN-001`).
- **Fem høj-severity fund**, alle koordinator-verificerede mod nuværende kode:
  en forældet "File Changed on Disk"-banner-Reload kan smide indtastede
  ændringer væk uden bekræftelse og rydder undo-stakken (`RIT-GEN-002`);
  compare/diff bruger to forskellige linje-tokenizers, så filer med enkelt
  `\r` vises med forkert justerede/forkerte linjer (`RIT-GEN-003`); Rc-cyklusser
  lækker hele per-vindue `Workspace` og holder Git-filmonitorer og
  `git`-subprocesser kørende efter vindueslukning (`RIT-GEN-004`); vinduesstørrelse
  gemmes kun via dirty-close-dialogen, så en ren luk aldrig gemmer størrelsen
  (`RIT-GEN-005`); og `project-sidebar-visible` overskrives til `false` under
  vinduesopbygning, så sidebaren aldrig gendannes og en GSettings-skrivning sker
  ved opstart (`RIT-GEN-006`).
- **Datatab er den dominerende risikoklasse.** De alvorligste fund samler sig om
  fire grænser: Git-mutations-argumenter, ekstern-ændrings/reload-flowet,
  overlappende open-race, og Git-subprocess-livscyklus (grace-vinduet er dødt
  kode, så mutationer SIGKILL'es straks og kan efterlade `index.lock`,
  `RIT-GEN-007`).
- **Supply-chain og release er stærkt bygget, men har konkrete huller.**
  `cargo-sources.json` er byte-for-byte i sync med `Cargo.lock`, alle GitHub
  Actions er SHA-pinned, finish-args er minimale (kun `wayland` +
  `fallback-x11`), og bundlet Git er dobbelt-pinned. Men gate'en validerer kun
  `static.crates.io`-entries (`RIT-GEN-015`), CI-container-images er
  flydende tags (`RIT-GEN-016`), og fire release-policy-nøgler er implementeret i
  workflowet uden maskinel håndhævelse (`RIT-GEN-018`).
- **Ydelse på den interaktive sti.** Markørflytning i en git-ændret ren fil
  re-spawner `git cat-file` pr. idle-cyklus (`RIT-GEN-012`), og `connect_changed`
  genberegner titel/subtitle/dirty-sæt 2-3× pr. tastetryk (`RIT-GEN-026`).
- **Parser-grænsen er robust.** Alle fem fuzz-targets bestod 1000 runs, den
  lokale `pulldown-cmark`-patch matcher sit manifest uden drift, og
  søge/large-file-koden er omhyggelig (generation-guards, `NOFOLLOW_SYMLINKS`,
  match/byte-caps). Ét normaliseringsfund: CRLF bliver til `\n\n`
  (`RIT-GEN-013`), og `decode_page_window` mangler fuzz/registry-dækning
  (`RIT-GEN-014`).
- **Valideringssuiten er grøn.** `cargo fmt/check/clippy -D warnings/test`
  (403 tests), `policy_check --strict`, `coverage_check` (82,0 %), samt alle
  metadata- og fuzz-smokes bestod. En grøn suite beviser dog ikke fravær af de
  ovenstående fund — flere ligger i stier testene ikke rammer.
- **Fund-optælling (38 i alt).** Severity: Critical 1, High 5, Medium 12,
  Low 14, Info 6. Kategori (ca.): correctness 7, data-loss 4, performance 5,
  security 2, tests 2, dead-code 4, maintainability 6, UX 4, docs 1, policy 3.
  Ingen bekræftede sandbox-fund.
- **Positiv sikring.** Ingen `unsafe` i runtime-kode; ingen schema-drift (alle 35
  GSettings-nøgler bruges); ingen gettext-bypass fundet; farveskema-kontrakten
  holder; tidligere lukkede fund `RIT-AUD-001/002/003/004/005/006/009/010/011/012/013/016/017`
  blev genverificeret uden regression.

---

## Triage (P0–P3) — arbejdsrækkefølge for en fixing-agent

Formålet med denne blok er, at en fixing-agent ikke drukner i 38 fund. Ret
oppefra og ned; giv **ikke** P3 til samme session som P0/P1.

- **P0 — ret først, som isoleret commit:** `RIT-GEN-001` (Critical, tavs
  datatab via Git-glob-pathspecs).
- **P1 — skal rettes før næste offentlige beta:** `RIT-GEN-002` (reload-datatab),
  `RIT-GEN-003` (diff-korrekthed), `RIT-GEN-004` (Rc/monitor-lækage),
  `RIT-GEN-005`, `RIT-GEN-006` (UI-state-persistering).
- **P2 — bør rettes snart:** `RIT-GEN-007`, `RIT-GEN-008`, `RIT-GEN-009`,
  `RIT-GEN-010`, `RIT-GEN-011`, `RIT-GEN-012`, `RIT-GEN-013`, `RIT-GEN-014`,
  `RIT-GEN-015`, `RIT-GEN-016`, `RIT-GEN-017`, `RIT-GEN-018`.
- **P3 — opportunistisk oprydning, separat session:** `RIT-GEN-019` til
  `RIT-GEN-038` (Low/Info).

**Severity-nuance (behandl labels som foreløbige):** severity måler her
sandsynlighed × impact, ikke kun risikotype. `RIT-GEN-005` og `RIT-GEN-006` er
markeret **High**, men deres impact er UX/persistens med **lav datatabsrisiko** —
de skal rettes tidligt fordi de er små og sikre, ikke fordi de er samme
risikoklasse som `RIT-GEN-001/002/004` (datatab / Git / index-korruption /
lækage). Lad ikke deres "High" udløse samme alarm.

**Fælles rod (læs før refaktor):** de fleste P0–P2-fund er samme fejlklasse —
asynkron callback-livscyklus med delte `Cancellable`s og stærke `Rc`-captures på
tværs af `workspace` / `editor_tab` / `source_control`. En samlet
cancellable-slot- og weak-capture-strategi (se §"Afsluttende kvalitetsvurdering")
adresserer `RIT-GEN-004/007/008/010/011/019` mere holdbart end punktfixes.

---

## 2. Omfang og metode

### Revideret kilde-tilstand
- Branch: `main`, commit `7276d9158bd5ff48d2961ecc4ca13ee076c97402`
  ("Document batch continuity").
- Worktree ren bortset fra utrackede `docs/fable_plan/` (selve audit-opgaven).
- App-version `0.3.7` (tidlig offentlig beta), Rust `1.95.0`, edition 2024.
- `EXTERNAL-REVIEW-HANDOFF.txt`, som audit-prompten kræver læst som fil nr. 1,
  findes ikke i repoet. **Afklaret af maintaineren (2026-07-06):** selve prompten
  `fable_audit_prompt.md` skulle have heddet `EXTERNAL-REVIEW-HANDOFF.txt`; der
  mangler altså ingen repo-fil — kun en navne-uoverensstemmelse i prompten. Benign
  (se §11).

### Kørte kommandoer (alle bestået)
Fra repo-roden:
- `git diff --check` — OK
- `scripts/dependency-preflight --root app` — OK
- `scripts/integration-preflight` — OK
- `glib-compile-schemas --strict --dry-run app/data/schemas` — OK
- `msgfmt --check-format --check-header -o /dev/null app/po/da.po` — OK
- `desktop-file-validate app/data/io.github.cadric.Riteed.desktop` — OK
- `appstreamcli validate --no-net --pedantic …metainfo.xml` — OK (1 pedantisk
  note: `cid-contains-uppercase-letter`, forventet for `io.github.cadric.Riteed`)
- `flatpak-builder --show-manifest …io.github.cadric.Riteed.yml` — OK
- `python3 -m tools.policy_check --root app --strict` — OK (108 s)
- `python3 -m tools.coverage_check --root app` — OK, linjedækning **82,0 %**
  (minimum 80,0 %)

Fra `app/`:
- `cargo fmt --all --check` — OK
- `cargo check --workspace --all-targets --all-features` — OK
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` — OK
- `GTK_A11Y=none GSK_RENDERER=cairo G_DEBUG=fatal-criticals RUST_TEST_THREADS=1
  cargo test --workspace --all-targets --all-features` — OK, **403 tests**
  bestået, 0 fejlede (82 s)

Fra `app/fuzz/` (cargo-fuzz 0.13.1, nightly `1.97.0`):
- `cargo fuzz list` — OK (5 targets)
- `cargo +nightly fuzz run <target> -- -runs=1000` for `markdown_parse`,
  `frontmatter_split`, `git_status_parse`, `diff_compute`,
  `unsupported_scanner` — **alle OK**

Derudover: `python3 -m unittest discover -s tools/tests` (delagent) — 153 tests
OK (1 skipped).

### Ikke kørte kommandoer og hvorfor
- Fuld Flatpak-build (`flatpak-builder --user --install`): kun manifest-parsning
  lokalt; fuld build dækkes af CI (`flatpak`-job).
- Nightly stress-suite (`riteed-stress`, ASan/Valgrind): lang GTK/Xvfb/DBus-flow;
  scripts inspiceret statisk, ikke kørt.
- `tools.ruleset_governance_check` mod live GitHub API: kræver netværk + token;
  audit kørte offline. Den live governance-verifikation dækkes af CI-jobbet
  `ruleset-governance`.
- Manuel runtime-UX-verifikation (klik-igennem): ikke udført; UI-fund er statisk
  begrundede og markeret ved behov for runtime-bekræftelse.

### Miljøbegrænsninger
- Audit kørt på maintainer-maskine (Linux, ikke i Flatpak-sandbox); sandbox-adfærd
  vurderet statisk ud fra manifestet.
- `RUST_TEST_THREADS=1` som i CI (parallel GTK-testflakiness er kendt).
- Ingen netværksadgang brugt.

### Metode
Fase 1 baseline-læsning → Fase 2 fuld valideringssuite → Fase 3 seks uafhængige
audit-spor kørt som separate read-only delagenter (data-tab/livscyklus;
GTK/UI-livscyklus; Markdown/parser/large-file/søgning; Git/compare/diff;
Flatpak/release/supply-chain/policy; dødkode/ydelse/vedligehold). Delagenter måtte
ikke redigere filer eller køre cargo-kommandoer. Fase 4: koordinator
genverificerede hvert høj-severity funds file:line mod nuværende kode (se §13),
deduplikerede og tildelte ensartede severities.

---

## 3. Arkitekturopsummering

Riteed er en GNOME-native teksteditor i Rust (gtk4-rs + libadwaita +
GtkSourceView 5), ~222 Rust-filer / ~57.000 linjer under `app/src`. Ingen
produktionsfil overstiger 600-linjers-kappen (max `editor_tab/compare/controller.rs`
598); testfiler under 800; `bin/riteed_stress.rs` (617) under sin 620-waiver.

- **App/vindue/workspace/dokument.** `lib.rs` bootstrapper GResource + gettext
  (System/English/Danish via GSettings) og starter `app::RiteedApp` —
  `adw::Application` med `HANDLES_OPEN` og multi-vindue. `window*.rs` ejer
  UI-skallen; `workspace*.rs` ejer fanebladsmodellen: åbning (`workspace_open.rs`
  med pending-open-registry), lukning (`workspace_close.rs`, `close_flow.rs`),
  autosave (`workspace/autosave.rs`), filovervågning (`workspace_monitor.rs`,
  `editor_monitor*`). Dokumenttilstand (fil-identitet, encoding, linjeskift,
  dirty-generation) er centraliseret i `document*.rs`/`editor_tab/state.rs`; I/O
  via `editor_io.rs` (async Gio, snapshot-gem med dirty-generation-guards, 25 MiB
  hård grænse).
- **Editor-faner.** `editor_tab/`: `open.rs` (async load, chunked cancellable
  apply), `save.rs` (snapshot-gem/save-as/konflikt), `runtime.rs`
  (callback-generationer), `large_file.rs` (viewer-routing), `markdown_preview.rs`,
  `compare/`.
- **Markdown-preview.** `markdown/` — egen CommonMark-pipeline oven på lokalt
  patchet `pulldown-cmark 0.13.4` + `yaml-rust2`-frontmatter, sikre pladsholdere
  for billeder/rå HTML, ingen browser-engine. Parser-grænser registreret i
  `app/build-aux/validation/parser-boundaries.v1.json` med bidirektionelle
  `PARSER-BOUNDARY`-markører.
- **Compare/diff.** `editor_tab/compare/` + `window_compare*` på `similar`:
  split/unified, inline token-highlights, hatch-filler, kollapsede regioner,
  multi-fil review-sessions.
- **Large-file viewer.** `large_file/` (V15): read-only, async Gio paged reads,
  bounded page-hukommelse, streaming-søgning; `document_limits.rs` ejer grænser.
- **Projekttræ.** `project_tree*`, `project_browser.rs`, `sidebar_host.rs`.
- **Source Control/Git.** `source_control*` taler kun med den typede
  Gio-subprocess-grænse i `git_process.rs` mod bundlet `/app/bin/git` (aldrig
  host-Git), med wall-clock-timeouts, cancellation og reaping. `git_status.rs`
  parser porcelain v2 `-z` med 10.000-entry cap.
- **Søgning.** `editor_search/` + `find_in_files/`.
- **Indstillinger.** `settings*` binder til GSettings-skemaet; kun
  `settings/appearance.rs` må tvinge farveskema.
- **Pakning.** Flatpak-first på `org.gnome.Platform//50`, minimale finish-args,
  bundlet trimmet Git 2.54.0 (GPG-verificeret kernel.org-kilde), offline
  cargo-build.
- **Policy/tooling.** `policy/*.json` (bundle + 10 domæner) håndhævet af
  `tools/policy_check.py` (linjegrænser, review-evidens-ankre, parser-registry,
  release/stress) + `tools/coverage_check.py` (min. 80 %). CI: `validate.yml`,
  release: `publish-flatpak.yml`.

**Højest-risiko-grænser:** async-callback-ejerskab mellem `editor_tab`-runtime,
`workspace`-faner og `source_control` (open/save/close/refresh deler
cancellables); Git-subprocess-livscyklus og argument-konstruktion;
ekstern-ændrings/reload-flowet; og release/signing-kæden.

---

## 4. Dækningsmatrix

| Område | Reviderede filer | Kørte checks | Resultat | Konfidens |
|---|---|---|---|---|
| Data-tab / dokument-livscyklus | `editor_tab/{open,save,state,runtime,apply,banner,callbacks,large_file}.rs`, `editor_io.rs`, `document*.rs`, `workspace*.rs`, `workspace/{autosave,tabs,selection,session_state}.rs`, `close_flow.rs`, `editor_monitor*`, `session.rs` | cargo test, statisk flow-trace | 1 High, 1 Medium, 2 Low/Info | Høj for F1/F2 (verificeret) |
| GTK/UI-livscyklus + a11y/UX | `window*.rs`, `app.rs`, `app_chrome/`, `dialogs*`, `sidebar_host.rs`, `settings*`, `data/ui/*`, `data/schemas/*.xml`, `po/da.po`, `.desktop`, `.metainfo.xml` | schema/desktop/appstream/msgfmt-validering, statisk trace | 3 High, 3 Low, 1 Info | Høj for F1/F2/F3 (verificeret) |
| Markdown/parser/large-file/søgning | `markdown/**`, `editor_tab/markdown_preview.rs`, `fuzz/fuzz_targets/**`, `editor_search/**`, `find_in_files/**`, `large_file/**`, `document_limits.rs` | 5× fuzz `-runs=1000`, patch-drift-diff, statisk trace | 1 Medium, 1 Medium(tests), 3 suspected | Moderat-høj |
| Compare/diff/source-control/Git | `editor_tab/compare/**`, `window_compare*`, `source_control/**`, `git_process*`, `git_status*`, `build-aux/git/**` | statisk trace, `similar`-kildeverifikation | 1 Critical, 1 High, 4 Medium, 3 Low | Moderat-høj |
| Flatpak/release/supply-chain/policy | `build-aux/*.yml`, `cargo-sources.json`, `cargo-patches/**`, `Cargo.lock` (app+fuzz), `.github/workflows/*`, `dependabot.yml`, `tools/checks/*`, `policy/*.json` | `tools/tests` (153), json-validering, sha256, patch-verifikation | 4 Medium, 1 Low, 1 Info | Høj (statisk); live-API ikke testet |
| Dead-code/ydelse/vedligehold | alle `app/src/**/*.rs` (scannet), ~25 læst tæt | linje-cap-audit, crate-wide usage-count, hot-path-trace | 1 Medium, 3 Low, 4 Info | Høj |

---

## 5. Fund-oversigt

| ID | Severity | Kategori | Titel | Primær fil | Konfidens | Fix-størrelse |
|---|---|---|---|---|---|---|
| RIT-GEN-001 | Critical | data-loss/security | Rå filnavne som glob-pathspecs i stage/unstage/discard | `git_process/support.rs` | Confirmed | XS |
| RIT-GEN-002 | High | data-loss | Forældet reload-banner smider ikke-gemte ændringer væk + rydder undo | `workspace_monitor.rs` | Confirmed | S |
| RIT-GEN-003 | High | correctness | Compare bruger anden linje-tokenizer end diff-modellen (lone `\r`) | `editor_tab/compare/controller.rs` | Confirmed | M |
| RIT-GEN-004 | High | maintainability/perf | Rc-cyklusser lækker `Workspace` + holder Git-monitorer kørende efter luk | `window/sidebar_wiring.rs` | Confirmed | S |
| RIT-GEN-005 | High¹ | UX | Vinduesstørrelse gemmes kun via dirty-close-stien | `window.rs` | Confirmed | XS |
| RIT-GEN-006 | High¹ | UX/policy | `project-sidebar-visible` overskrives til false ved opstart | `window_project/sidebar_state.rs` | Confirmed | S |
| RIT-GEN-007 | Medium | correctness | Timeout grace-vindue er dødt; mutationer SIGKILL'es straks | `git_process.rs` | Confirmed | S |
| RIT-GEN-008 | Medium | correctness | Ingen index-lock-gate for actions → overlappende mutation fejler | `source_control/actions.rs` | Confirmed | S |
| RIT-GEN-009 | Medium | security/UX | Review-fane viser Git-stier uden escaping (line-map desync + bidi) | `editor_tab/compare/review_session.rs` | High | S |
| RIT-GEN-010 | Medium | correctness | Diff-action annullerer igangværende status-refresh uden reschedule | `source_control/actions.rs` | Confirmed | S |
| RIT-GEN-011 | Medium | data-loss | Overlappende open kaprer en loading clean-untitled fane | `workspace_open.rs` | Confirmed | S |
| RIT-GEN-012 | Medium | performance | Markørflytning re-spawner `git cat-file` pr. idle | `source_control/minimap.rs` | High | S |
| RIT-GEN-013 | Medium | correctness | CRLF-normalisering laver `\r\n` om til `\n\n` | `markdown/normalize.rs` | Confirmed | XS |
| RIT-GEN-014 | Medium | tests | `decode_page_window` untrusted-grænse uden fuzz/registry | `large_file/page_text.rs` | Confirmed | S |
| RIT-GEN-015 | Medium | security | `dependency_preflight` validerer kun `static.crates.io`-entries | `tools/checks/dependency_preflight.py` | Confirmed | S |
| RIT-GEN-016 | Medium | security | Release-gatende CI-jobs kører i flydende container-tags | `.github/workflows/validate.yml` | Confirmed | S |
| RIT-GEN-017 | Medium | policy | Rollback-gate binder `CANDIDATE_REF` til `GITHUB_REF_NAME` | `.github/workflows/publish-flatpak.yml` | Confirmed | XS |
| RIT-GEN-018 | Medium | policy | 4 release-policy-nøgler uden maskinel håndhævelse | `tools/checks/release.py` | Confirmed | M |
| RIT-GEN-019 | Low | maintainability | `commit()` overskriver delt cancellable uden cancel | `source_control.rs` | Confirmed | XS |
| RIT-GEN-020 | Low | correctness | Ignore-whitespace dropper afsluttende whitespace-linje | `editor_tab/compare/diff.rs` | Confirmed | S |
| RIT-GEN-021 | Low | performance | Review-loader læser hele worktree-fil før aggregeret cap | `source_control/review_loader.rs` | Confirmed | S |
| RIT-GEN-022 | Low | correctness | Pending-open ryddes kun pr. URI → kan servere detached fane | `workspace_open.rs` | Medium | S |
| RIT-GEN-023 | Low | UX | Sidebar-action sætter true uden root (Ctrl+Shift+F) | `window_project.rs` | High | XS |
| RIT-GEN-024 | Low | maintainability | `EditorZoom` CssProvider fjernes aldrig ved luk | `editor_zoom.rs` | Confirmed | XS |
| RIT-GEN-025 | Low | UX | Encoding-dialog mangler header bar (HIG) | `dialogs/encoding.rs` | High | S |
| RIT-GEN-026 | Low | performance | `connect_changed` genberegner præsentation/dirty 2-3× pr. tastetryk | `editor_tab/callbacks.rs` | High | S |
| RIT-GEN-027 | Low | performance | Large-file-søgning kloner match-vektor pr. 256 KB-chunk | `large_file/search.rs` | Confirmed | S |
| RIT-GEN-028 | Low | security | Publish build checker tag-navn ud, ikke commit-SHA | `.github/workflows/publish-flatpak.yml` | Confirmed | XS |
| RIT-GEN-029 | Low | maintainability | Test-only overflader kompileres ind i produktion (ikke `cfg(test)`) | `settings.rs` | Confirmed | M |
| RIT-GEN-030 | Low | maintainability | `gtk_tests_v4..v13` milepæls-navngivne; små kan foldes ind | `lib.rs` | Confirmed | M |
| RIT-GEN-031 | Low | maintainability | Duplikeret `line_slices` ×3 + `ngettext`-count-mønster ×10 | `editor_tab/compare/*` | Confirmed | S |
| RIT-GEN-032 | Low | docs | README-drift (V14.5, "Format"-side, manglende handoff) | `README.md` | Confirmed | XS |
| RIT-GEN-033 | Info | dead-code | 5 døde `pub`-funktioner | `editor_tab/runtime.rs` m.fl. | Confirmed | XS |
| RIT-GEN-034 | Info | dead-code | `SearchOutcome.scanned_bytes` kun for en test | `large_file/search.rs` | Confirmed | XS |
| RIT-GEN-035 | Info | dead-code | `ExternalFileEvent::Moved`-sti unåelig i produktion | `editor_monitor.rs` | Confirmed | S |
| RIT-GEN-036 | Info | performance | `compare_skip_reason` allokerer linje-slices bare for at tælle | `editor_tab/compare/diff.rs` | Confirmed | XS |
| RIT-GEN-037 | Info | maintainability | Micro-observationer (dead clone, vestigial test, maximized, css) | `app.rs` | Confirmed | XS |
| RIT-GEN-038 | Info | security | `ruleset-governance` passerer vakuøst for fork/dependabot-PR'er | `.github/workflows/validate.yml` | Confirmed | S |

Fix-størrelse: XS ≈ 1 linje / lokal; S ≈ én funktion; M ≈ flere filer/moduler.

¹ `RIT-GEN-005`/`RIT-GEN-006`: High på sandsynlighed × impact, men impact-typen er
UX/persistens med **lav datatabsrisiko** — ret tidligt (små, sikre fixes), men
ikke samme risikoklasse som datatab/Git/lækage-fundene. Se Triage-blokken.

---

## 6. Detaljerede fund

### RIT-GEN-001 — Rå filnavne sendes som glob-fortolkende Git-pathspecs i stage/unstage/discard

- Severity: **Critical**
- Confidence: Confirmed
- Category: data-loss / security
- Status: confirmed finding
- Files:
  - `app/src/git_process/support.rs:25-37` (`git_env()` — mangler `GIT_LITERAL_PATHSPECS`)
  - `app/src/git_process/ops.rs:300-319` (`restore --worktree -- <path>`), `:215-242` (`ls-tree`), `:244-277` (unstage-flow), `:279-298` (force-remove)
  - `app/src/source_control/actions.rs:187-207` (discard-flow)
- Evidence:
  - `ops.rs:310`: `self.run(["restore", "--worktree", "--", path], ...)` — `path` er det rå filnavn fra `git status` (`GitPath::as_utf8()`), sendt verbatim som pathspec.
  - `git_env()` sætter 9 env-vars, men `rg "GIT_LITERAL_PATHSPECS|:\(literal\)"` over `app/src` giver **0 hits** (koordinator-verificeret). Git fortolker som standard pathspecs med fnmatch: `*`, `?`, `[...]` er wildcards, og et `:`-præfiks er pathspec-magic.
  - Data-flow discard: `confirm_discard_entry` → `discard_entry` → `restore_worktree_path` → `restore --worktree -- 'pages/[id].tsx'`. `[id]` fortolkes som en tegnklasse, så Git gendanner `pages/i.tsx`/`pages/d.tsx` (ødelægger *deres* ikke-gemte ændringer) og lader den navngivne fil urørt.
  - Data-flow unstage: `ls-tree -z HEAD -- 'pages/[id].tsx'` matcher intet literalt → tom output → `Ok(None)` → `update-index --force-remove -- pages/[id].tsx` (literal) sletter index-entryen. Brugeren bad om at unstage en ændring og fik en staged sletning.
- Why this matters: Ét klik på en bekræftet destruktiv handling (som netop navngiver én fil i dialogen) kan ramme andre filer eller korrumpere indexet. Ægte, tavs datatab. Rammer alle projekter med glob-tegn i filnavne (fx Next.js/SvelteKit dynamiske ruter `pages/[id].tsx`).
- Reproduction: repo med tracked, modificeret+staged `pages/[id].tsx` og et søskende `pages/i.tsx` med ikke-gemte ændringer; tryk Unstage/Discard i Source Control-sidebaren. Argv-konstruktionen ovenfor er eksakt.
- Suggested fix: Tilføj `("GIT_LITERAL_PATHSPECS", "1")` i `git_env()` (`support.rs`). Ingen kommando i den tilladte overflade er afhængig af globbing; alle pathspec-tagende kald (`ls-tree`, `restore`, `update-index`) modtager præcis ét eksakt filnavn. Opdatér kontrakt-noten i `build-aux/git/README.md`.
- Suggested regression test: `git_process/tests.rs` — temp-repo med `[id].txt` (tracked, modificeret, staged) og `i.txt`; kør `unstage_path` og assertér at `[id].txt` stadig er staged-modified (ikke slettet) og `i.txt` urørt; kør `restore_worktree_path("[id].txt")` og assertér at `i.txt`'s worktree-ændringer overlever.
- Risk of fix: **Low.** `GIT_LITERAL_PATHSPECS` påvirker kun pathspec-parsing; `rev-parse/status/config/check-attr/cat-file/hash-object/commit/log` tager ingen.

### RIT-GEN-002 — Forældet "File Changed on Disk"-banner-Reload smider ikke-gemte ændringer væk og rydder undo-stakken

- Severity: **High**
- Confidence: Confirmed
- Category: data-loss
- Status: confirmed finding
- Files:
  - `app/src/editor_tab/banner.rs:82-93` (banner vises kun når `!is_dirty` ved sync-tid)
  - `app/src/editor_tab/callbacks.rs:11-19` (`connect_changed` re-synker ikke banneret)
  - `app/src/workspace_monitor.rs:83-85` (`Reload => request_reload(workspace, tab, false)`)
  - `app/src/editor_tab/runtime.rs:376-388` (`can_apply_reload`: `UserRequested => should_apply()` — ingen `is_dirty()`-check)
  - `app/src/editor_tab/apply.rs:61-64` (`set_enable_undo(false)` tømmer undo-stakken)
- Evidence:
  - `banner.rs:86`: `let should_offer_reload = is_selected && window_active && !is_dirty;` — banneret tilbydes kun når fanen er ren på sync-tidspunktet.
  - Intet call-site re-synker banneret på tastetryk (koordinator verificerede alle kald til `sync_external_banner`).
  - `runtime.rs:385-387`: `Automatic => !self.is_dirty() && should_apply(),` men `UserRequested => should_apply(),`. `request_reload(_, _, false)` → `ReloadCause::UserRequested` → `should_apply` returnerer ubetinget `true`. Ingen dirty-guard.
  - Design-hensigten er, at dirty + ekstern ændring skal kræve bekræftelse: `workspace_monitor.rs:61-79` viser `confirm_external_reload` ("...while you also have unsaved changes"). Banner-stien omgår den beskyttelse.
- Why this matters: Banner vises mens fanen er ren; brugeren taster videre (banneret bliver stående); ét klik på Reload erstatter bufferen med disk-indhold, og undo kan ikke gendanne det.
- Reproduction: Åbn fil, hold vindue fokuseret + fane valgt. Ret filen eksternt (`echo x >> file`) → banner "This File Changed on Disk. [Reload]". Tast ny tekst. Klik Reload → buffer erstattet; Ctrl+Z gendanner intet.
- Suggested fix: I `on_banner_action` (`workspace_monitor.rs:85`), når `tab.is_dirty()`, præsentér `dialogs::confirm_external_reload` i stedet for at kalde `request_reload` direkte; re-synk/skjul banneret ved dirty-transition (`connect_modified_changed`).
- Suggested regression test: GTK-test der injicerer `ContentPossiblyChanged`, viser Reload-banner, taster tekst, udløser banner-action og assertér at bufferen stadig indeholder den tastede tekst (kø-dialog-mekanismen findes allerede via `queue_*_responses_for_tests`).
- Risk of fix: **Low** — dirty-bekræftelsesdialogen findes og testes allerede; ændringen er én branch-reroute.

### RIT-GEN-003 — Compare/review bruger en anden linje-tokenizer end diff-modellen → forkert justerede rækker og forkert tekst for filer med enkelt `\r`

- Severity: **High**
- Confidence: Confirmed
- Category: correctness
- Status: confirmed finding
- Files:
  - `app/src/editor_tab/compare/diff.rs:142-144` (`text.tokenize_lines()` — modelside)
  - `app/src/editor_tab/compare/controller.rs:578-584` og `app/src/editor_tab/compare/review_session.rs:576-582` (`text.split_inclusive('\n')` — displayside)
  - `app/src/editor_tab/compare/render.rs` (row-index→buffer-linje-tagging)
- Evidence:
  - Koordinator-verificeret: `diff.rs:143` bruger `text.tokenize_lines()`; `controller.rs:582` og `review_session.rs:580` bruger `text.split_inclusive('\n').collect()`.
  - `similar-3.1.1/src/text/abstraction.rs` viser at `tokenize_lines` afslutter en linje ved et enkelt `\r`, mens `split_inclusive('\n')` ikke gør. `DiffRowModel` gemmer linje-indekser fra `tokenize_lines`; display-stien indekserer den *anden* opdeling med dem.
  - Statisk eksempel: reference `"a\rb\nX\n"` vs current `"a\rb\nY\n"` — modellen markerer linje 2 (`X`) som ændringen, men display-slices er `["a\rb\n","X\n"]`, så den faktiske ændring rendres som uændret. Interiør `\r` i buffer-teksten behandler GTK som paragraf-separator, så row-index-baserede tags (gutter, scroll-sync, hatch, clipboard) lander på forkerte buffer-linjer.
- Why this matters: For tekst med enkelt CR (blandede/legacy line-endings, strejfende `\r` i en linje) skifter hvert linje-indeks efter CR'et; ændringer vises som kontekst og omvendt.
- Reproduction: Åbn en fil med `"a\rb\nX\n"`, ret `X`→`Y`, kør Compare with Saved Version; se den ændrede linje rendret som kontekst.
- Suggested fix: Brug en `tokenize_lines`-ækvivalent i `controller.rs`/`review_session.rs` (eksportér diff.rs-helperen), og strip/escape interiør `\r` (og U+2028/2029) når buffer-teksten bygges, så display-rækker forbliver 1:1 med buffer-linjer. Bemærk overlap med RIT-GEN-031 (tre ens-navngivne `line_slices`).
- Suggested regression test: Unit-test der assertér at `display_for_model`-tekst for `"a\rb\nX\n"` giver modify-row-tekst `"X"`/`"Y"`, plus en buffer-test der assertér `buffer.line_count() == display.rows.len()` for CR-holdig input.
- Risk of fix: **Medium** — linjeopdeling fødder gutters, scroll-sync, hatches og clipboard; kræver konsekvent brug af den delte helper.

### RIT-GEN-004 — Rc-referencecyklusser lækker hele per-vindue `Workspace`; lækkede Git-filmonitorer kører `git status` efter vindueslukning

- Severity: **High**
- Confidence: Confirmed
- Category: maintainability / performance / lifecycle
- Status: confirmed finding
- Files:
  - `app/src/window/sidebar_wiring.rs:72-85` (stærke `Rc::clone`-captures i handlers)
  - `app/src/window/git_actions.rs:14-22` (`WindowGitActions { workspace: Rc<Workspace>, ... }`)
  - `app/src/workspace.rs:74,443-444` (`git_action_sync_handler: OnceCell<...>`, setter rydder aldrig)
  - `app/src/source_control.rs:96,234-236` (`state_change_handler: Option<Rc<dyn Fn()>>` — self-cyklus)
  - `app/src/source_control/live.rs` (Git-`FileMonitor` afmeldes kun ved root-skift)
- Evidence (koordinator-verificeret topologi):
  - Cyklus A: `workspace.rs:74` gemmer handleren i en `OnceCell` (aldrig ryddet); closuren ejer `Rc<WindowGitActions>`, som (`git_actions.rs:21`) ejer `Rc<Workspace>`. `Workspace → handler → WindowGitActions → Workspace`.
  - Cyklus B: `source_control.rs:234-236` gemmer den anden handler i `SourceControlState`; closuren ejer en klon af selve controlleren (`Rc<RefCell<SourceControlState>>`) — self-cyklus.
  - Den korrekte konvention findes andre steder: `document_tools.rs` og `window_compare.rs:68` bruger `Rc::downgrade`. `sidebar_wiring.rs` er den eneste caller med stærke captures.
  - `SourceControlState` ejer `live_refresh` med aktive `gio::FileMonitor` på `.git`; dens callback-guard bruger `Weak<SourceControlState>`, men weak-upgraden lykkes altid pga. cyklus B → et lukket vindues lækkede state modtager fortsat monitor-events og re-kører `refresh_status` (spawner `git`-subprocesser) resten af app-levetiden.
- Why this matters: Hvert lukket vindue (sekundære vinduer via `Ctrl+N`, tab-transfer-vinduer) lækker hele editor-workspacet og fortsætter baggrunds-Git-polling mod den lækkede projekt-root. Forstærker RIT-GEN-012.
- Reproduction: Statisk bevis (topologien ovenfor). Runtime: byg vindue, hold `Weak<Workspace>`, luk vinduet, drain events, assertér at weak stadig kan upgrades (lækket).
- Suggested fix: I `sidebar_wiring.rs`, capture `Rc::downgrade(&git_actions)` og en weak af source-control-state (fabrikker findes allerede, fx `root_change_handler`), og upgrade inde i closuren — som `document_tools.rs`/`window_compare.rs`.
- Suggested regression test: GTK-test i stil med `DialogLeakCanary`: byg + destruér vindue, drain, assertér at `Weak<Workspace>` og source-control-weak ikke længere kan upgrades.
- Risk of fix: **Low** — handlers fyrer kun mens vinduet lever; weak-upgrade-fejl er en no-op, identisk med compare/document-tools-mønsteret.

### RIT-GEN-005 — Vinduesstørrelse persisteres kun når luk går gennem dirty-dialogen; en ren luk gemmer den aldrig

- Severity: **High** (impact-type: UX/persistens, **lav datatabsrisiko** — se Triage-blokken; ret tidligt, men ikke samme alarm som RIT-GEN-001/002/004)
- Confidence: Confirmed
- Category: UX
- Status: confirmed finding
- Files:
  - `app/src/window.rs:463-478` (`on_close_request` → `persist_window_size` kun når `allow_window_close()`)
  - `app/src/workspace.rs:150,257-258` (`allow_window_close` starter `false`)
  - `app/src/workspace_close.rs:9-21,278-281` (sat `true` kun i `finish_window_close`; ren luk returnerer `Proceed` ved tom `dirty_tabs`)
- Evidence (koordinator-verificeret):
  - `window.rs:464-465`: `if self.workspace.allow_window_close() { self.persist_window_size(); return Proceed; }`.
  - `allow_window_close` skrives kun ét sted: `workspace_close.rs:279 finish_window_close` (kun ende af dirty-bekræftelses-flow).
  - Ren luk: `handle_window_close_request` rammer `if dirty_tabs.is_empty() { return Propagation::Proceed; }` (`workspace_close.rs:19`) — vinduet lukker uden `persist_window_size`. `set_window_size` har ingen anden produktions-caller (`window.rs:477` + en test-helper).
- Why this matters: At ændre størrelse og lukke uden ikke-gemte ændringer (normalcasen) mister størrelsen; den huskes kun hvis brugeren tilfældigvis lukkede gennem "Save changes?"-dialogen. Samme sti gælder tomme vinduer via `on_page_detached`.
- Reproduction: Ret størrelse på et rent vindue, `window.close()`, se at `window-width`/`window-height` ikke opdateres.
- Suggested fix: I `Window::on_close_request`, persistér før *enhver* `Proceed` (fx uafhængigt øverst — `set_window_size` er saniteret og billig).
- Suggested regression test: GTK-test med memory-backend: ret størrelse på et rent vindue, luk, assertér `write_log_for_tests()` indeholder `window-width`-skrivningen.
- Risk of fix: **Low** — én ekstra guarded settings-skrivning på en bruger-initieret luk (stadig en eksplicit brugerhandling per GSettings-policy).

### RIT-GEN-006 — `project-sidebar-visible` overskrives til `false` under vinduesopbygning: sidebaren gendannes aldrig, via en opstarts-GSettings-skrivning

- Severity: **High** (impact-type: UX/persistens + policy, **lav datatabsrisiko** — se Triage-blokken; ret tidligt, men ikke samme alarm som RIT-GEN-001/002/004)
- Confidence: Confirmed
- Category: UX / policy (opstarts-GSettings-skrivning)
- Status: confirmed finding
- Files:
  - `app/src/window_project.rs:183-189` (opbygning: `install_callbacks` før async `restore_from_settings`)
  - `app/src/window_project/sidebar_state.rs:19-24,44,154-158` (`sync_actions_for_root` uden root → `set_sidebar_visibility(false)` → `persist_sidebar_visible(false)`)
  - `app/src/window_project/root.rs` (restore læser den nu-overskrevne nøgle)
- Evidence (koordinator-verificeret):
  - `sidebar_state.rs:19-23`: `if !has_root { state.sidebar_visible_action.set_state(&false...); set_sidebar_visibility(&mut state, false); }`.
  - `set_sidebar_visibility(false)` → `persist_sidebar_visible(state, false)` → `state.settings.set_project_sidebar_visible(false)` når den gemte værdi var `true` (`sidebar_state.rs:154-158`).
  - Dette kører ved opbygning (`install_callbacks`, root er `None`) **før** den async restore læser `project_sidebar_visible()` — som nu er `false`. Nettoresultat: at efterlade sidebaren synlig og genstarte åbner altid med den skjult, og præferencen destrueres før den læses.
- Why this matters: Vedvarende præference tabt hver opstart, plus en GSettings-skrivning ved opstart (som den tidligere audit-kontrakt netop begrænser til eksplicitte brugerhandlinger).
- Reproduction: Sæt `project-folder-uri` + `project-sidebar-visible=true`, byg vindue, vent til root gendannet, se at sidebaren er skjult og at nøglen blev skrevet før restore.
- Suggested fix: Gør no-root-syncen ikke-persisterende (parameter eller søster-funktion), så `sync_actions_for_root` kollapser panelet og synker action-state **uden** `persist_sidebar_visible`; persistering kun ved bruger-toggle og root-open/close.
- Suggested regression test: Memory-settings GTK-test: sæt nøglerne, byg vindue, spin til root gendannet, assertér `project_sidebar_visible_for_tests()` er `true` og at `write_log_for_tests()` ikke har en `project-sidebar-visible`-skrivning før restore.
- Risk of fix: **Low** — panelet animerer stadig lukket ved opstart; kun den durable skrivning fjernes.

### RIT-GEN-007 — Ved timeout SIGKILL'es mutations-børn straks; 2-sekunders grace-vinduet er dødt kode

- Severity: **Medium**
- Confidence: Confirmed
- Category: correctness (data-loss-risiko: strandet `index.lock`)
- Status: confirmed finding
- Files: `app/src/git_process.rs:324-360,393-423` (communicate-callback + timeout-installer), `app/src/git_process/ops.rs:18-21` (design-kommentar)
- Evidence (koordinator-verificeret):
  - `install_git_timeout` fyrer → `timed_out.set(true)`, `cancellable.cancel()`, skemalægger et `GIT_CANCEL_KILL_GRACE`-kill (2 s).
  - Men `cancellable.cancel()` får `communicate_async` til at returnere `Err(Cancelled)` næsten straks; dens callback (`git_process.rs:341-345`) `timeout_handle.cancel()` fjerner den ventende grace-source, og fordi `timed_out.get()` er `true` kører `kill_unfinished_git` (SIGKILL via `force_exit`) med det samme — også for ops flagget `MUTATING_KILL_ON_CANCEL = false`, hvis hele formål (ops.rs:18-20: "a killed index writer strands .git/index.lock") var at undgå netop det. Grace-timeren kan aldrig gøre sit arbejde.
- Why this matters: En legitim langsom mutation (fx `git commit` på et stort repo der overstiger 30 s-timeouten) SIGKILL'es midt i index-skrivning → strandet `index.lock`. Testen `git_operations_have_wall_clock_timeout_and_kill_grace` assertér kun konstanterne, ikke adfærden; `RIT-AUD-006` blev lukket med henvisning til "grace-window force-exit" som koden ikke faktisk leverer.
- Reproduction: Statisk (trace ovenfor). Runtime: en git-wrapper der sover >30 s viser øjeblikkelig SIGKILL efter cancel, ikke +2 s.
- Suggested fix: I communicate-callbacken, når `timed_out.get() && !kill_on_cancel`, undlad at kill'e straks; hold grace-sourcen i live (undlad `timeout_handle.cancel()` på den cancelled sti) og lad den fyre, eller send SIGTERM først med SIGKILL efter grace.
- Suggested regression test: Harness-test med en stub-langsom "git" der assertér at kill sker tidligst `GIT_CANCEL_KILL_GRACE` efter timeout for en `kill_on_cancel=false`-spec.
- Risk of fix: **Medium** — rører den delte subprocess-reaper; må ikke genindføre zombier (behold `wait_async`-reap).

### RIT-GEN-008 — Ingen index-lock-gate for actions: en annulleret-men-kørende mutation overlapper næste → falsk "The Git operation failed."

- Severity: **Medium**
- Confidence: Confirmed
- Category: correctness / lifecycle
- Status: confirmed finding
- Files: `app/src/source_control/actions.rs:360-389` (`begin_action_inner`), `app/src/git_process/ops.rs:20` (`MUTATING_KILL_ON_CANCEL=false`), `app/src/source_control/refresh.rs:30-42` (lock-gate findes kun for refresh)
- Evidence (koordinator-verificeret): `begin_action_inner` annullerer forrige `state.cancellable` og starter straks den nye op. Siden 2026-07-05 dræbes et annulleret mutations-barn ikke (det kører færdigt; dets callback droppes som `Cancelled`). Klik Stage på A og hurtigt Stage på B kører B's `update-index` mens A stadig holder `.git/index.lock`; git fejler hurtigt → `finish_error` viser den generiske fejl og markerer status stale. Refresh har en lock-gate; actions har ikke.
- Why this matters: Rutinemæssig hurtig staging af flere filer giver intermitterende bruger-synlige fejl og stale UI; A's egen completion refresher heller ikke UI'en (callbacken var annulleret).
- Reproduction: Repo hvor `update-index` tager >~100 ms (stort index/langsom disk); klik Stage på to rækker hurtigt.
- Suggested fix: I `begin_action_inner`, hvis `live::index_lock_exists(state)` (eller en tidligere mutation er in-flight), kø/afvis med den eksisterende "Waiting for another Git operation to finish"-besked i stedet for at fyre samtidigt.
- Suggested regression test: Unit-test på en `begin_action`-gate med en stubbed lock-probe (mønster findes i `live_scheduler.rs`-tests).
- Risk of fix: **Low** — additiv gate der genbruger eksisterende scheduler-plumbing.

### RIT-GEN-009 — Review-fane rendrer Git-stier uden den påkrævede display-escaping → line-map-desync og bidi-spoofing

- Severity: **Medium**
- Confidence: High
- Category: security (display) / correctness
- Status: confirmed finding
- Files: `app/src/editor_tab/compare/review_session.rs:554-562` (`path_display`), `:159-178` (`render_text` samler én streng pr. række med `\n`), `:346-370` (`rebuild_rendered_lines` antager 1 række = 1 buffer-linje)
- Evidence: `path_display` indlejrer den rå sti (`std::str::from_utf8(raw_path)...`), i modsætning til sidebaren der bruger `git_status.rs escape_git_path_display`. Et filnavn med `\n` (lovligt på Linux, repræsenterbart i porcelain `-z`) får `render_text()` til at producere **to** buffer-linjer for én `rendered_lines`-entry, hvilket forskyder alle efterfølgende linjer; navigation, `current_file_for_line`, `open_reviewed_file` og change-list-targets rammer forkerte rækker/filer. C0/bidi-kontroltegn rendres uescaped (display-spoofing som den tidligere audit lukkede for andre overflader, RIT-AUD-016).
- Why this matters: Regression af path-escaping-kontrakten på en nyere overflade; forkert fil kan åbnes, og bidi kan skjule det reelle filnavn.
- Reproduction: repo med en modificeret tracked fil navngivet `a\nb.txt`; åbn "Review Unstaged Changes"; navigation-targets efter grænsen er forskudt.
- Suggested fix: Route `path_display` gennem `escape_git_path_display` (og brug den for `DisplayFileBoundaryRow.path`).
- Suggested regression test: review-session-unit-test med en `\n`-holdig sti der assertér `render_text().lines().count() == rendered_lines.len()`.
- Risk of fix: **Low** — display-only ændring; rå bytes for git-identitet holdes separat.

### RIT-GEN-010 — Åbning af en rækkes Diff annullerer en igangværende status-refresh der aldrig reschedules

- Severity: **Medium**
- Confidence: Confirmed
- Category: correctness / lifecycle
- Status: confirmed finding
- Files: `app/src/source_control/actions.rs:349-357,360-389` (`begin_diff_action` → ubetinget `previous.cancel()`), `app/src/source_control/refresh.rs:30-57`
- Evidence: `begin_action_inner` deles af Diff (non-mutating). Den annullerer `state.cancellable` — som under en Manual/Initial refresh er refreshens cancellable — og alle refresh-callbacks bailer via `should_ignore_cancelled`. Intet reschedules. En Manual refresh havde allerede sat `status_stale = true` og label til "Refreshing Git status…"; efter diff-klikket bliver de hængende til næste FS-event eller manuel refresh: label fast, commit disabled, snapshot stale.
- Why this matters: Tryk Refresh på et stort repo, klik straks en række (single-click aktiverer Diff) → UI hænger i "Refreshing…".
- Reproduction: Som ovenfor.
- Suggested fix: For non-mutating diff, undlad at annullere refresh-cancellablen (brug en separat cancellable-slot for diff-blob-loads), eller reschedule via `live::schedule(state)` efter cancel.
- Suggested regression test: Controller-test: start manuel refresh med stubbed langsom status, kør Diff-action, assertér at en refresh re-issues / label genoprettes.
- Risk of fix: **Low-Medium** — den delte cancellable-slot er load-bearing; en separat slot for diff er den sikrere form.

### RIT-GEN-011 — Overlappende open-requests kaprer en stadig-loading "clean untitled"-fane — den første fil tabes tavst

- Severity: **Medium**
- Confidence: Confirmed
- Category: data-loss / open-race
- Status: confirmed finding
- Files: `app/src/workspace_open.rs:434-444,463-471`, `app/src/editor_tab.rs:325-329` (`is_clean_untitled`), `app/src/editor_tab/state.rs:151-154`, `app/src/editor_tab/open.rs:240-243`
- Evidence (koordinator-verificeret): `acquire_open_target` genbruger den ene fane hvis `is_clean_untitled()`, som kun tjekker `is_document() && path().is_none() && !is_dirty()` — en fane der *lige nu loader fil A* opfylder alle tre (`is_loading()` konsulteres ikke). En anden overlappende request for fil B genbruger A's fane; B's load annullerer A's cancellable, og A's completion fejler med `AppError::Cancelled`, som `handle_open_failure` sluger tavst. Resultat: A åbnes aldrig og brugeren får ingen feedback.
- Why this matters: At åbne to filer hurtigt efter hinanden åbner kun én, tavst; under session-restore kan en indskudt bruger-open slå en session-fil ud af restaurering.
- Reproduction: Start med én tom fane. Åbn stor/langsom fil A, så straks fil B før A er færdig. Kun B er åben; ingen fejl vises.
- Suggested fix: I `acquire_open_target`, afvis genbrug når kandidat-fanen er en registreret pending-open-target eller `tab.is_loading()`.
- Suggested regression test: GTK-test: én untitled fane; `request_open_files([A])` med en multi-MB-fixture, så `request_open_files([B])` i samme main-loop-tur; spin til settled; assertér `tab_count == 2` og begge URIs til stede.
- Risk of fix: **Low** — værste tilfælde er én ekstra tom fane i et sjældent race.

### RIT-GEN-012 — Markørflytning i en git-ændret ren fil re-spawner `git cat-file` pr. UI-idle

- Severity: **Medium**
- Confidence: High (statisk traced end-to-end)
- Category: performance
- Status: confirmed finding
- Files: `app/src/editor_tab/callbacks.rs:33-41` → `workspace/tabs.rs:448-453` → `workspace/selection.rs:14-26` → `window/sidebar_wiring.rs:75-79` → `source_control/minimap.rs:29-90` → `git_process/ops.rs:141-156`
- Evidence (koordinator-verificeret): Hver markørflytning fyrer `connect_cursor_moved` → `queue_refresh_selected_state()` (idle-coalesced) → `git_action_sync_handler` → `refresh_editor_minimap_diff_for_tab` → `refresh_tab_without_cancel`. For et rent dokument med git-entry `ReferenceInput::Blob(oid)` er der **ingen fingerprint-check før spawn** — det går direkte til `cat_blob` → `run(["cat-file","blob",oid], …)` (op til 1 MB output). Dedupe (`already_current`, `minimap_diff.rs:72-79`) sker først *inde i* `apply_source_control_minimap_diff`, altså efter blobben er hentet. Hver ny idle annullerer også den in-flight → kill/spawn-churn ved holdt piletast.
- Why this matters: Subprocess-spawn + op til 1 MB pipe-read i UI-event-tempo på den interaktive sti, for normalcasen "redigeringssession i et dirty git-repo". Forstærkes af RIT-GEN-004.
- Reproduction: Åbn en modificeret fil i et git-repo, flyt markøren gentagne gange; observér gentagne `git cat-file`-spawns.
- Suggested fix: Før `load_reference_blob`, spørg fanen om `state.ui.minimap_diff.applied.source == source && !stale` (source-tokenet koder allerede repo/path/oids/flags) og returnér tidligt. Én lille `pub(crate)`-prædikat på `EditorTab`.
- Suggested regression test: Test-only spawn-tæller (analog til `LINE_DIFF_CALLS`) i `GitProcess::run`; i et GTK-flow: apply minimap diff, flyt markør N gange, `drain_events`, assertér tælleren ikke voksede.
- Risk of fix: **Low** — fingerprintet skal inkludere alt der invaliderer diffen; `source`-tokenet + `stale`-flaget gør det allerede.

### RIT-GEN-013 — CRLF-normalisering laver hver `\r\n` om til en blank linje (`\r\n` → `\n\n`)

- Severity: **Medium**
- Confidence: Confirmed (kode-adfærd); Medium på live-sti-reachability
- Category: correctness (parser)
- Status: confirmed finding
- Files: `app/src/markdown/normalize.rs:11-23`
- Evidence (koordinator-verificeret): `normalize_markdown_character` mapper char-for-char: `'\r' => '\n'`. Fordi transformen er per-tegn, bliver et CRLF-par `\r\n` til `\n` + `\n`. Statisk: `'a\r\nb'` → `'a\n\nb'` (én soft line-break bliver til et paragraf-brud); et helt CRLF-dokument bliver dobbelt-spaced. Dette omgår pulldown-cmarks egen korrekte `\r\n`-håndtering.
- Why this matters: Preview divergerer fra korrekt CommonMark for enhver `\r`-holdig input. Live-stien er mitigeret fordi GtkSourceViews `FileLoader` normaliserer line-endings til `\n` ved load, men indsat CRLF-indhold og fuzz-entrypoints (`String::from_utf8_lossy`) rammer stien.
- Reproduction: `parse_document("a\r\nb")` giver to `MdBlock::Paragraph` i stedet for én paragraf med `SoftBreak`.
- Suggested fix: Kollaps CRLF før per-char-mappen (fx `str::replace "\r\n"→"\n"` og derefter `'\r'→'\n'` for enkelte CR), eller drop `\r`→`\n`-omskrivningen og lad pulldown-cmark håndtere line-endings; behold control-char→space og U+FFFD→space.
- Suggested regression test: `parse_document("Line 1\r\nLine 2")` skal give én paragraf med en `SoftBreak`; plus en `render_tests`-case der assertér at et CRLF-dokument ikke dobbelt-spaces.
- Risk of fix: **Low** — afgrænset til normalisering; eksisterende `\n`-input upåvirket.

### RIT-GEN-014 — `decode_page_window` (large-file page-decoder) er en untrusted-byte-grænse uden fuzz-target og uden registry-entry

- Severity: **Medium**
- Confidence: Confirmed (dæknings-gap); koden er panik-sikker ved statisk analyse
- Category: tests (fuzz/parser-boundary)
- Status: policy gap
- Files: `app/src/large_file/page_text.rs:11-84`; `app/build-aux/validation/parser-boundaries.v1.json` (`large_file_paged_reader`)
- Evidence: `decode_page_window(offset, bytes)` decoder vilkårlige fil-bytes ved en vilkårlig page-offset med rå slice-indeksering og `String::from_utf8_lossy`. Registryets `large_file_paged_reader` lister kun `reader.rs` i `source_paths`; `page_text.rs` har ingen `PARSER-BOUNDARY`-markør og intet fuzz-target. Delagenten hånd-tracede alle index-stier (tomme vinduer, all-continuation-bytes, korte vinduer, `offset==0` vs `>0`) og fandt **ingen reachable panik i dag**; fundet er den manglende maskinelt-mappede fuzz/registry-evidens for en untrusted byte-grænse — netop den klasse stress-fuzz-policyen findes for at lukke (`parser_or_untrusted_input_boundary_without_registry_mapping_or_reviewed_exception`).
- Why this matters: Decoderen kører på hver page af hver meget stor/viewer-fil (op til 500 MiB) med angriber-påvirkede bytes og vilkårlige split-punkter; en fremtidig ændring af slice-aritmetikken har ingen fuzz-backstop.
- Reproduction: `page_text.rs` optræder hverken i `app/fuzz/fuzz_targets/` eller i registryets `source_paths`/`coverage`.
- Suggested fix: Tilføj en `fuzz_page_decode(bytes, offset)`-shim i `lib.rs` `fuzzing` + et `page_text_decode`-fuzz-target med seed; tilføj `page_text.rs` til `large_file_paged_reader`-grænsens `source_paths` + coverage + `PARSER-BOUNDARY`-markør.
- Suggested regression test: proptest i `page_text.rs`: for tilfældige `bytes`/`offset`, assertér `visible_start ≤ visible_end`, `next_offset > offset` når `!bytes.is_empty()`, aldrig panik.
- Risk of fix: **Low** (tilføjer kun dækning).

### RIT-GEN-015 — `dependency_preflight` validerer kun `static.crates.io`-entries i `cargo-sources.json`; alle andre entry-former er usynlige for gaten

- Severity: **Medium**
- Confidence: Confirmed
- Category: security (supply chain)
- Status: policy gap (bypassable enforcement)
- Files: `tools/checks/dependency_preflight.py:404-414` (`_is_static_crates_io_archive`), `:417-442` (`_check_cargo_sources`); `app/build-aux/io.github.cadric.Riteed.yml:68`
- Evidence (koordinator-verificeret): `archives`-dict-comprehensionen forfiltrerer med `_is_static_crates_io_archive` (kræver `url.startswith("https://static.crates.io/crates/")` og `dest.startswith("cargo/vendor/")`). Stale-checken `set(archives) - set(expected)` ser derfor aldrig: en `archive` med anden URL, `type git`, `type file`, eller `inline`-entries med `dest-filename` ≠ `.cargo-checksum.json` (fx `dest ".cargo"`, `dest-filename "config.toml"` der injicerer `[patch]`/`rustc-wrapper`). flatpak-builder materialiserer alle entries før `cargo --offline --locked build` i den signerede publish-build. Nuværende tilstand er ren (113 archives, alle `static.crates.io`, checksums identiske med `Cargo.lock`).
- Why this matters: `cargo-sources.json` er en stor genereret fil; en ondsindet hunk skjult i en "regenererings"-diff kan tilføje upinnede/angriber-kontrollerede build-inputs til det signerede beta-release uden at nogen validator fejler. Med solo-maintainer-governance (0 påkrævede approvals) er offline-gaten det primære forsvar.
- Reproduction: `rg "static.crates.io" tools/` viser at URL-begrænsningen er den eneste; intet andet værktøj læser `cargo-sources.json`.
- Suggested fix: Gør `_check_cargo_sources` til en eksakt allowlist over *alle* entries: hard-fail alt der ikke er (a) den kendte `inline` cargo-config på `("cargo","config.toml")`, (b) et `static.crates.io`-archive i `expected`, eller (c) en `inline` `.cargo-checksum.json` på en `expected`-dest.
- Suggested regression test: `tools/tests`-case med en cargo-sources-liste der indeholder et archive med ikke-crates.io-URL og en inline `config.toml` på `.cargo` — assertér begge fejler.
- Risk of fix: **Low** — allowlisten matcher nuværende fil eksakt.

### RIT-GEN-016 — Release-gatende CI-jobs kører i flydende-tag container-images; policy forbyder upinnede mutable inputs, men intet værktøj tjekker container-images

- Severity: **Medium**
- Confidence: Confirmed
- Category: security (CI supply chain) / missing enforcement
- Status: policy gap
- Files: `.github/workflows/validate.yml:50` (`docker pull fedora:42`), `:186,:204` (`image: ghcr.io/flathub-infra/flatpak-github-actions:gnome-50`); `policy/release.policy.json:151-165`; `tools/checks/release.py:160-173`
- Evidence (koordinator-verificeret): Policy siger `"mutable_inputs": { "allowed_unpinned_inputs": [] }`. `_check_mutable_inputs` scanner kun for `uses: x@vN`, `curl | sh` og upinned `cargo install`. `rg "docker|container|image|@sha256|ghcr" tools/` matcher kun en urelateret token-leak-check. `native-tests`/`flatpak-tests`/`flatpak`-kontekster produceret i disse mutable images er præcis dem publish-preflight kræver før signing-secrets importeres.
- Why this matters: Et kompromitteret `fedora:42`- eller `gnome-50`-tag kan tavst grønne en release-kritisk gate. Afgrænset til validerings-integritet (publish-build genbygger fra tag på runneren), derfor Medium.
- Reproduction: Se ovenstående `rg`; ingen enforcement.
- Suggested fix: Digest-pin begge images (`fedora:42@sha256:…`, `ghcr.io/…:gnome-50@sha256:…`) og udvid `_check_mutable_inputs` til at flagge `image:`/`docker pull` uden `@sha256:`.
- Suggested regression test: `tools/tests`-workflow-fixture med `image: foo:latest` → forvent fejl; `image: foo@sha256:<64hex>` → pass.
- Risk of fix: **Low-Medium** — digest-pins bliver stale når gnome-50-imaget opdateres; dependabot opdaterer ikke container-digests, så dokumentér en refresh-procedure.

### RIT-GEN-017 — Rollback/same-version-gaten binder til dispatch-ref (`GITHUB_REF_NAME`), ikke den validerede `release_ref`

- Severity: **Medium**
- Confidence: Confirmed (statisk)
- Category: policy / release
- Status: policy gap
- Files: `.github/workflows/publish-flatpak.yml:74-77,283-289,308-309,333`
- Evidence (koordinator-verificeret): `version`/`tag_commit` udledes fra `$release_ref`, men monotoni/rollback-gaten får `CANDIDATE_REF="$GITHUB_REF_NAME"`. For en `workflow_dispatch` på `main` med `release_ref=vOld` + `emergency_rollback=true` skal operatøren sætte `rollback_ref=main` (ikke tag'et) for at `emergency_allowed()` passerer — den publicerede rollback-metadata dokumenterer så `main` i stedet for det faktisk publicerede tag (undergraver `requires_documented_target_ref`). Omvendt fejler en legitim idempotent same-version re-publish dispatchet fra `main` (`candidate_ref="main" != published source_ref`), i modstrid med `"manual_rerun_behavior": "Must be idempotent"`.
- Why this matters: Dokumentations-bindingen er bypassable (forkert rollback-ref registreres), og idempotent re-publish fejler uventet.
- Reproduction: Statisk trace ovenfor.
- Suggested fix: `CANDIDATE_REF="$release_ref"` (én linje; push-tag-adfærd uændret, da `release_ref == GITHUB_REF_NAME` der).
- Suggested regression test: Statisk token-check i `_check_rollback_gate` for `candidate_ref="$release_ref"`, eller workflow-fixture-test der afviser `CANDIDATE_REF="$GITHUB_REF_NAME"` når `release_ref`-override findes.
- Risk of fix: **Low** — strammer kun bindingen.

### RIT-GEN-018 — Release-policy-nøgler implementeret i workflowet men uden maskinel håndhævelse (regression ville passere `policy_check`)

- Severity: **Medium** (aggregeret)
- Confidence: Confirmed
- Category: policy (intent uden enforcement)
- Status: policy gap
- Files: `policy/release.policy.json:91-93,159,207-212,325`; `tools/checks/release.py`, `tools/checks/release_workflow.py`; impl i `.github/workflows/publish-flatpak.yml`
- Evidence (koordinator-verificerede rg-beviser):
  1. `tag_commit_must_be_ancestor_of_main` (:93) — impl `publish-flatpak.yml:96` (`git merge-base --is-ancestor`); `rg "is-ancestor|merge-base" tools/` → kun `integration_preflight.py:38` (lokal build-helper). Sletning ville lade et un-merged tag publicere.
  2. `appstream_top_release_must_match_tag` + `tag_format`-version (:91-92) — impl `:88-93,192-229`; `release_workflow.py check_publish_triggers` tjekker kun `GITHUB_REF`-tokens. Policyens egen `verifiers.workflow_static_checks` (:325) *påstår* dette er static-checket — det er det ikke.
  3. `signing_key_governance.private_key_import.*` (:207-212: temporær GNUPGHOME, agent-kill on exit, ingen passphrase-logging, fingerprint == fuld key-ID) — impl `:404-434`; `rg "GNUPGHOME|gpg-agent|preset|mktemp|kill" tools/` → ingen. `_check_key_governance` dækker kun den *offentlige*-nøgle-pin.
  4. `github_hosted_runner_required...` (:159) — intet inspicerer `runs-on`.
- Why this matters: `AGENTS.md:178` siger "Hard-fail validation tooling is authoritative". Disse fire intents kan tavst regressere i en workflow-edit der passerer `policy_check --strict` og alle 153 validator-tests.
- Reproduction: rg-beviserne ovenfor.
- Suggested fix: Tilføj token-presence-checks i `release.py`/`release_workflow.py`: `merge-base --is-ancestor` + `origin/main`; metainfo-parse-markører (`metainfo.xml`, `releases`, `sys.exit(1)`); `mktemp -d` + `GNUPGHOME` + `gpgconf ... --kill` + fingerprint-equality; `runs-on: ubuntu-`-allowlist.
- Suggested regression test: For hver: en `tools/tests`-fixture med de tilsvarende linjer fjernet → forvent fejl.
- Risk of fix: **Low** — token-checks i eksisterende stil.


### RIT-GEN-019 — `commit()` overskriver delt cancellable uden at annullere igangværende refresh

- Severity: **Low** · Confidence: Confirmed · Category: maintainability/lifecycle · Status: confirmed finding
- Files: `app/src/source_control.rs:404-479` (sætter `state.cancellable = Some(...)` uden `previous.cancel()`, i modsætning til `begin_action_inner`)
- Evidence: Under en Automatic refresh (som ikke sætter `status_stale`, så commit-knappen forbliver aktiv) dropper klik-Commit refreshens cancellable uden at annullere; begge kører videre. Pre-commit-snapshottet kan blive anvendt af refresh A efter commit's refresh B (`apply_status` har ingen generation-guard) → stale entries markeret `status_stale=false`; commit-kontroller kan kortvarigt re-enables midt i commit → to samtidige `git commit`.
- Why this matters: Lille race, men kan tillade en anden samtidig commit og efterlade stale status.
- Reproduction: Statisk; kræver Automatic refresh sammenfaldende med commit-klik.
- Suggested fix: Spejl `begin_action_inner`: annullér forrige cancellable og sæt `status_stale = true` ved commit-start.
- Suggested regression test: Controller-test der starter refresh + commit og assertér én aktiv cancellable.
- Risk of fix: **Low**.

### RIT-GEN-020 — Ignore-whitespace-tilstand dropper en afsluttende whitespace-only-linje uden newline fra diff-modellen

- Severity: **Low** · Confidence: Confirmed · Category: correctness · Status: confirmed finding
- Files: `app/src/editor_tab/compare/diff.rs:146-167` (`trim_line_sides_text`), `:62-75`
- Evidence: For input der ender i en whitespace-only-linje uden terminator (fx `"a\n   "`) bidrager den trimmede linje `"" + ""` til normaliseret tekst, så `tokenize_lines` giver én linje mindre end originalen; `build_row_model` udsender ingen række for den sidste originale linje, og compare-viewet udelader den.
- Why this matters: Sidste linje forsvinder fra sammenligningen i ignore-whitespace-tilstand.
- Suggested fix: Bevar linjetal i `trim_line_sides_text` (fx udsend en bar `\n`-pladsholder for en forsvindende uterminreret sidste linje) eller map ops tilbage til originale indekser.
- Suggested regression test: Diff-unit-test på `"a\n   "` vs `"a\nb"` der assertér at rækkeantal matcher originalen.
- Risk of fix: **Low**.

### RIT-GEN-021 — Review-loader læser hele worktree-filer uden per-fil-cap før den aggregerede cap anvendes

- Severity: **Low** · Confidence: Confirmed · Category: performance/memory · Status: confirmed finding
- Files: `app/src/source_control/review_loader.rs:275-297` (`load_contents_async` → `record_file`), `:147-178` (cap anvendt efter filen er fuldt i hukommelsen)
- Evidence: Blob-læsninger er cappet ved `BLOB_CAP` (~1 MB), men unstaged-review-worktree-stien loader hele filen via GIO før `AGGREGATE_DECODED_BYTE_CAP` konsulteres; en multi-GB modificeret fil giver en transient fuld-størrelse-allokering.
- Suggested fix: Query filstørrelse først (`query_info_async`) og spring over ud over en per-fil-cap konsistent med `BLOB_CAP`.
- Suggested regression test: Loader-test med en over-cap worktree-fil der assertér tidlig skip uden fuld læsning.
- Risk of fix: **Low**.

### RIT-GEN-022 — Pending-open-registry ryddes kun pr. URI og kan servere en detached fane

- Severity: **Low** · Confidence: Medium · Category: correctness/lifecycle · Status: confirmed finding
- Files: `app/src/workspace_open.rs:446-461,506-527`, `app/src/workspace_close.rs:110-121`, `app/src/editor_tab/apply.rs:171-176`
- Evidence: `clear_pending_open` matcher på URI, ikke `(URI, tab)`. (1) Åbn A i tab1, luk tab1 mens loading; den annullerede callback (der kalder `clear_pending_open`) leveres på en senere idle, og open-callbacken holder `tab_for_result` i live, så registry-`Weak` upgrader stadig → gen-åbning af A rammer `find_tab_by_file` og returnerer den **detached** tab1 → `set_selected_page` på en side der ikke er i viewet (GTK-criticals). (2) En efterfølger-entry for A (ny tab2) fjernes af tab1's `clear_pending_open(A)`, hvilket genåbner duplikat-vinduet registryet skulle lukke.
- Suggested fix: Send den ejende tab ind i `clear_pending_open` og behold entries medmindre både URI og tab-identitet matcher (`Weak::ptr_eq`); i `find_tab_by_file`, spring pending-entries over hvis fanens side ikke længere er attached.
- Suggested regression test: GTK-test: åbn stor fixture, `close_selected_page_for_tests()`, gen-åbn straks samme fil, assertér attached+selected fane.
- Risk of fix: **Low**.

### RIT-GEN-023 — `win.project-sidebar-visible` committer `true` uden projekt-root (state-drift ved Ctrl+Shift+F)

- Severity: **Low** · Confidence: High · Category: UX/action-state · Status: confirmed finding
- Files: `app/src/window_project.rs:296-316`, `app/src/window/actions.rs:95-98`
- Evidence: `connect_change_state` sætter state før validering (`action.set_state(...)` derefter `if state.root.is_none() { return; }`). `open_project_search` (Ctrl+Shift+F) kalder `set_sidebar_visible(true)` ubetinget; uden åben mappe bliver action-state `true` mens intet vises, og header-toggle rendres checked-men-insensitive til næste root-skift.
- Suggested fix: Validér root før `action.set_state`, eller nulstil state til `false` i early-return-grenen.
- Suggested regression test: Uden root, `change_state(true)`; assertér action-state forbliver `false`.
- Risk of fix: **Low**.

### RIT-GEN-024 — `EditorZoomController` tilføjer en display-CssProvider pr. vindue og fjerner den aldrig

- Severity: **Low** · Confidence: Confirmed · Category: maintainability (bounded leak) · Status: confirmed finding
- Files: `app/src/editor_zoom.rs:31-38,53-66,80-88`
- Evidence: `install_provider` kalder `style_context_add_provider_for_display` med en per-vindue-provider og unik zoom-CSS-klasse; `EditorZoomController` har ingen `Drop`-impl der fjerner den. Kontrast `AppChromeController` (`app_chrome/mod.rs:109-113`) som fjerner sin provider på `Drop`. Hvert lukket vindue efterlader en død provider på displayet resten af process-levetiden.
- Suggested fix: Spejl chrome-controlleren: `impl Drop { style_context_remove_provider_for_display(...) }`.
- Suggested regression test: Doc-kommentar + code-review-guard, eller tæl providers via test-hook.
- Risk of fix: **Low**.

### RIT-GEN-025 — Encoding-chooser-dialogen udelader den header bar de øvrige custom-dialoger bruger

- Severity: **Low** · Confidence: High · Category: UX/HIG · Status: improvement
- Files: `app/src/dialogs/encoding.rs:238-244` vs `app/src/dialog_shell.rs:10-50` og `app/src/dialogs/recent_files.rs:34-39`
- Evidence: `present_encoding_dialog` bygger `adw::Dialog::builder()...child(&content)` direkte — ingen `AdwToolbarView`/`AdwHeaderBar`, så titlen eksponeres kun til a11y og der er ingen luk-knap, i modsætning til Recent Files der går gennem `build_dialog_shell` (bygget netop for at standardisere dette).
- Suggested fix: Route encoding-dialogen gennem `build_dialog_shell`.
- Suggested regression test: Genbrug leak-canary-wiring; assertér toolbar-view til stede.
- Risk of fix: **Low** (visuelt).

### RIT-GEN-026 — `connect_changed` genberegner præsentation og dirty-tilstand 2-3× pr. tastetryk

- Severity: **Low** · Confidence: High (mekanisme); Medium (impact) · Category: performance · Status: confirmed finding
- Files: `app/src/editor_tab/callbacks.rs:11-30`, `app/src/editor_tab.rs:463-478` (`sync_presentation`), `app/src/workspace/tabs.rs:448-453`, `app/src/workspace.rs:447-463`
- Evidence: `connect_changed` kalder `tab.sync_presentation()` på **hver** buffer-ændring (genopbygger `title()` med pgettext+String og `subtitle()` med `HOME`-opslag), og den visuelle callback kalder synkront `notify_dirty_state_changed()` (genopbygger `dirty_session_uris()`). Samme callback fyrer igen fra `connect_cursor_moved` og en tredje gang i den idle-coalesced `refresh_selected_state`. Dirty-*transitioner* dækkes allerede af `connect_modified_changed`, og `set_dirty_uris` kortslutter på uændret sæt — så ved steady-state-typing genberegnes alt 2-3× pr. tastetryk kun for at blive kasseret.
- **Overlap:** Den planlagte (men ikke-eksekverede) batch-2-plan `docs/fable_plan/2026-07-05-batch-2-hotpath-and-features.md` Task 1 fjerner netop den direkte `notify_dirty_state_changed()` i `workspace/tabs.rs:451` (koordinator-verificeret stadig til stede ved `7276d91`), og Task 2 tilføjer en equality-guard i `source_control/active_row.rs set_active_uri` (også stadig uden guard). Dette fund korroborerer den plan; `sync_presentation`-halvdelen er *ikke* dækket af planen og bør adresseres separat.
- Suggested fix: Fjern `sync_presentation()` + dirty-notifikationen fra `connect_changed` (behold `mark_dirty_generation`, preview-scheduling, stale-check); stol på `connect_modified_changed` for dirty-transitioner.
- Suggested regression test: Udvid v13 status-coalescing-flowet til at tælle `set_dirty_uris`-kald pr. syntetisk tastetryk-burst.
- Risk of fix: **Low-Medium** — verificér at format-dirty stadig refresher tab-indikatoren.

### RIT-GEN-027 — Large-file-søgning kloner den akkumulerede match-vektor pr. 256 KB-chunk

- Severity: **Low** · Confidence: Confirmed · Category: performance · Status: confirmed finding
- Files: `app/src/large_file/search.rs:85` (`let mut matches = matches.clone();`), loop 54-115; `document_limits.rs:18-19`
- Evidence: `search_next` er en selv-rekurserende async-scan; fordi state er fanget i en `Rc<dyn Fn>` (ikke `FnOnce`), klones `matches` (op til 10.000 × u64) og `carry` på hver chunk-callback. En multi-GB-fil er titusinder af chunks → O(chunks × matches) memcpy som en `Rc<RefCell<SearchState>>` ville eliminere.
- Suggested fix: Hold `matches`/`carry` i ét `Rc<RefCell<…>>`-scan-state-objekt delt på tværs af rekursionen.
- Suggested regression test: Eksisterende cross-chunk-tests dækker adfærd; tilføj evt. allokeringstæller.
- Risk of fix: **Low**.

### RIT-GEN-028 — Publish `build`-job checker det mutable tag-navn ud, ikke den validerede commit-SHA (TOCTOU)

- Severity: **Low** · Confidence: Confirmed · Category: security/release · Status: confirmed finding
- Files: `.github/workflows/publish-flatpak.yml:381-383` (`ref: release_ref`), `:95` (preflight opløser `tag_commit`); `tools/checks/release_workflow.py:206-216` (kræver aktuelt ref-formen)
- Evidence: Preflight verificerer check-runs mod `tag_commit`, men build gen-opløser tag-navnet. Hvis `refs/tags/vX` flyttes mellem jobs, bygges den signerede artefakt fra en uvalideret commit. Mitigering: "Protect version tags"-ruleset forbyder `update`/`deletion` uden bypass-actors (live-verificeret), men rulesets kan disables af repo-ejeren.
- Suggested fix: `ref: ${{ needs.preflight.outputs.tag_commit }}` og opdatér `_build_checkout_targets_release_ref`.
- Suggested regression test: `tools/tests`-fixture der kræver `tag_commit`-checkout.
- Risk of fix: **Low** — checkout via SHA er strengt stærkere.

### RIT-GEN-029 — Test-only-overflader kompileres ind i produktions-builds (ikke `cfg(test)`-gated)

- Severity: **Low** · Confidence: Confirmed · Category: maintainability · Status: confirmed finding
- Files: `app/src/settings.rs:123` (`pub fn new_for_tests()` uden `cfg(test)`), `SettingsBackend::Memory` (`:22-25`) + ~60 `SettingsBackend::Memory(...)`-match-arme; `app/src/dialogs/lifecycle.rs:31-47` (tre `pub(crate)`-test-fns)
- Evidence: `new_for_tests` er ikke `cfg(test)`-gated og trækker hele in-memory-backenden ind i release-builds; kun `cfg(test)`-kode kalder den. **Bemærk (koordinator-korrektion):** delagenten formodede en "latent lint-bombe" (dead_code under `warnings = "deny"`), men koordinatorens centrale `cargo clippy -D warnings` og `cargo check --all-targets` **bestod rent** — lint-bomben materialiserede sig ikke (`pub`-items er usynlige for dead_code-linten). Kun vedligeholdelses-pointen står.
- Why this matters: En "for tests"-settings-backend er nåelig fra produktions-stier, og hver settings-ændring implementeres to gange.
- Suggested fix: Gate `new_for_tests`, `Memory`-varianten, `MemorySettings` og arme bag `#[cfg(test)]` (præcedens: `RiteedApp::application()` er `#[cfg(any(test, feature = "stress"))]`); gate de tre lifecycle-fns bag `#[cfg(test)]`.
- Suggested regression test: Ikke direkte testbart; code-review-guard.
- Risk of fix: **Medium** (mekanisk, men rører 14 filer).

### RIT-GEN-030 — `gtk_tests_v4..v13`-familien: levende, ikke legacy; navngivnings- og merge-gæld

- Severity: **Low** · Confidence: Confirmed · Category: maintainability · Status: confirmed finding
- Files: `app/src/lib.rs:97-132`, `app/src/gtk_tests.rs:580-671`, 12 versionerede filer (3.028 linjer)
- Evidence: Hvert versioneret modul eksponerer `exercise_*`-funktioner kaldt fra den **ene** serielle `#[test] gtk_surfaces_and_editor_flow_work` via `run_gtk_flow` — de tester alle nuværende adfærd (ingen er stale). De kan **ikke** merges til én fil (3.028 > 800-cap), men navngivningen er efter release-milepæl frem for feature (ulig `gtk_tests_tabs.rs`, `gtk_tests_markdown.rs`). Små filer er trivielt konsoliderbare: v11_git (38), v10 (61), v13 (108) ind i feature-navngivne filer. Sekundær omkostning: den monolitiske serielle test lader den første fejlende flow maskere senere flows (delvist mitigeret af `gtk-flow-start/end`-markører).
- Suggested fix: Opportunistiske renames til feature-navne når filer røres; fold sub-200-linje-versionsfiler ind i feature-filer. Ingen adfærdsændring.
- Risk of fix: **Low** (test-only; hold hver fil < 800).

### RIT-GEN-031 — Duplikerede helpers: `line_slices` ×3 (to identiske, én ens-navngivet men afvigende) + `ngettext(...).replace("%d", …)`-mønster ×~10

- Severity: **Low** · Confidence: Confirmed · Category: maintainability · Status: confirmed finding
- Files: `app/src/editor_tab/compare/controller.rs:578-584` og `review_session.rs:576-582` (byte-identiske), `app/src/editor_tab/compare/diff.rs:142-144` (samme navn, `tokenize_lines`-semantik); count-formatering i `review_session.rs:549-552`, `editor_search/support.rs:40-46`, `find_in_files/mod.rs:430-441`, `compare/status.rs:54-63`, `compare/display.rs:278-283`, `compare/presentation.rs:268,280`, `large_file/viewer_status.rs:20-25`, `source_control/review_loader.rs:323-328`
- Evidence: Tre private fns kaldet `line_slices` i ét modultræ, hvoraf én opfører sig anderledes (den lone-CR-forskel er kernen i RIT-GEN-003). Pluraliseret count-formatering er reimplementeret ~10 steder.
- Suggested fix: Én `pub(crate)` linje-opdelings-helper pr. semantik (dokumentér `\n`-only vs `similar`-tokenizer i navnene) og én delt `count_label(count, singular, plural)`-i18n-helper.
- Suggested regression test: Eksisterende i18n-tests dækker flere call-sites; gør den delte semantik eksplicit i navnet.
- Risk of fix: **Low** (ren refaktor).

### RIT-GEN-032 — README-drift: "V14.5 is next" og listet "Format"-preferences-side

- Severity: **Low** · Confidence: Confirmed · Category: docs · Status: confirmed finding
- Files: `README.md:75` ("ROADMAP.md - milestone plan through V16; V14.5 is next."), `README.md:34` ("Multi-page preferences for General, Editor, Appearance, Format, and Source Control."); `ROADMAP.md:8` (`completed_through: v14.7`, `next_version: v15`); `CHANGELOG.md` (0.3.7 "Reorganized Preferences into four pages")
- Evidence (koordinator-verificeret): README siger "V14.5 is next", men ROADMAP-frontmatter er `next_version: v15` (V14.7 gennemført). README lister fem preferences-sider inkl. "Format", men CHANGELOG 0.3.7 reorganiserede til **fire** sider (General, Appearance, Editor, Source Control) og flyttede per-dokument-encoding/linjeskift til en status-bar-format-menu — der er ingen "Format"-preferences-side længere.
- Note (afklaret af maintaineren 2026-07-06): Det oprindeligt rapporterede handoff-underpunkt — at `EXTERNAL-REVIEW-HANDOFF.txt` (krævet læst af prompten som fil nr. 1) mangler — er **benignt**. Prompten `fable_audit_prompt.md` skulle selv have heddet `EXTERNAL-REVIEW-HANDOFF.txt`; der mangler ingen repo-fil, kun en navne-uoverensstemmelse i prompten. Ikke et Riteed-doc-problem.
- Suggested fix: Ret README-linje 75 til at matche ROADMAP (`v15` næste); ret linje 34 til fire sider uden "Format". (Handoff-navnet er maintainerens valg og kræver ingen repo-ændring.)
- Suggested regression test: Ingen (docs); overvej en `tools`-check der krydser README-versionslinjen mod ROADMAP-frontmatter.
- Risk of fix: **Low**.

### RIT-GEN-033 — Fem døde `pub`-funktioner (nul referencer crate-wide inkl. tests, fuzz, stress, ui_smoke)

- Severity: **Info** · Confidence: Confirmed · Category: dead-code · Status: dead-code candidate
- Files: `app/src/editor_tab/runtime.rs:53-60` (`current_line_ending_mode`), `app/src/editor_tab/review.rs:171-179` (`current_review_file`), `app/src/editor_monitor.rs:55-61` (`is_acknowledged`), `app/src/document.rs:74-76` (`set_saved` — kun `set_saved_with_display_path` bruges), `app/src/editor_tab.rs:431-434` (`set_writability_for_tests`, `#[cfg(test)]` men ubrugt)
- Evidence (koordinator-verificeret): Hver af de fem symboler har præcis 1 forekomst (definitionen) i `rg -n "\b<sym>\b" --glob '*.rs'`. `pub` i `pub`-moduler skjuler dem for rustc's dead_code-lint.
- Suggested fix: Slet. Kør usage-grep igen ved fix-tid.
- Risk of fix: **Ingen fundet**.

### RIT-GEN-034 — `SearchOutcome.scanned_bytes` eksisterer kun for at tilfredsstille en unit-test

- Severity: **Info** · Confidence: Confirmed · Category: dead-code · Status: dead-code candidate
- Files: `app/src/large_file/search.rs:13,30,92` (skrevet), læst kun `:215` inde i `#[cfg(test)]`; eneste eksterne consumer `large_file/viewer.rs:339-344` bruger kun `matches`/`reached_cap`
- Evidence: `rg -n "scanned_bytes" app/src` — kun definition + test-assertion.
- Suggested fix: Drop feltet + assertionen, eller assertér via en test-lokal beregning.
- Risk of fix: **Ingen**.

### RIT-GEN-035 — `ExternalFileEvent::Moved`-stien er unåelig i produktion

- Severity: **Info** · Confidence: Confirmed · Category: dead-code · Status: dead-code candidate
- Files: `app/src/editor_monitor.rs:135,186-206`, `app/src/editor_tab/runtime.rs:115-150`
- Evidence: `normalize_monitor_event` mapper hver event til `ContentPossiblyChanged` eller `Missing` og konstruerer aldrig `Moved` (`_other_file` ignoreres). Så `Moved`-armen, hele rename-follow-grenen i `handle_external_event` og `Moved`-armene af `next_pending_state` er døde i produktion (kun tests injicerer `Moved`). Den bevarede `Moved`-maskineri opdaterer desuden ikke `state.document.source_file`, hvilket ville springe saverens mtime-check over på næste gem. Ingen data-tab-sti — derfor Info.
- Suggested fix: Slet `Moved`-stien, eller wire faktisk `other_file` for `Renamed/MovedOut`; hvis bevaret, opdatér også `source_file` i Moved-handleren.
- Risk of fix: **Low** (fjernelse) / Medium (wiring ændrer semantik).

### RIT-GEN-036 — `compare_skip_reason` allokerer fulde linje-slice-vektorer bare for at tælle linjer

- Severity: **Info** · Confidence: Confirmed · Category: performance · Status: improvement
- Files: `app/src/editor_tab/compare/diff.rs:123-144`
- Evidence: `line_count(text) = line_slices(text).len() = text.tokenize_lines()` samler en `Vec<&str>` pr. side (op til ~1 M entries ved 1 MB-cap, ~16 MB transient) kun til limit-checken; derefter tokeniseres begge tekster **igen** for modellen.
- Suggested fix: Tæl via en iterator med `similar`-kompatibel semantik, eller tokenisér én gang og genbrug.
- Risk of fix: **Low** — behold lone-`\r`-semantikken (regressionstest `fuzz_regression_lone_cr_line_splitting_matches_diff_ops` pinner den).

### RIT-GEN-037 — Micro-observationer (død klon, vestigial test-assertion, manglende maximized-tracking, legacy CSS-syntaks, dokumenteret lint-suppression)

- Severity: **Info** · Confidence: Confirmed · Category: maintainability · Status: improvement
- Files: `app/src/app.rs:86` (`let _keep_state_alive = self.state.borrow().windows.clone();` — kloner tom `Vec`, misvisende navn), `app/src/app.rs:509` (test asserterer accels for `win.focus-project-sidebar`, en action der ikke findes i `app/src` — vestigial), `app/src/window.rs:471-478` (ingen `is-maximized`-tracking; et maximized luk gendannes som umaximized monitor-størrelse), `app/data/ui/appearance.css:153` (legacy `@accent_bg_color` mens resten bruger `var(--accent-bg-color)`), `app/src/git_process.rs:172` (`#[allow(clippy::too_many_arguments, reason=...)]` — kodebasens eneste lint-suppression, allerede sporet af batch-2 Task 3)
- Evidence: Koordinator-verificeret hver linje. Bemærk: `#[allow]` med `reason` er ikke en blank suppression (clippy kører fortsat `-D warnings` andetsteds), men AGENTS.md-ånden foretrækker en signaturomlægning; batch-2-planen adresserer den.
- Suggested fix: Slet `_keep_state_alive`-klonen og den vestigiale test-assertion; tilføj `is-maximized`-nøgle; normalisér CSS-syntaks; fjern `#[allow]` via `GitRunOptions` (batch-2 Task 3).
- Risk of fix: **Low**.

### RIT-GEN-038 — `ruleset-governance` passerer vakuøst for fork- og dependabot-PR'er

- Severity: **Info** · Confidence: Confirmed · Category: security (bounded) · Status: policy gap
- Files: `.github/workflows/validate.yml:176-180`
- Evidence: `if:`-betingelsen sidder på *steppet*, så jobbet (en påkrævet status-kontekst i `Protect main` og i `required_validate_check_contexts`) lykkes med verifikationen sprunget over for `dependabot[bot]` og fork-head-PR'er. Afgrænset: PR'er kan ikke ændre server-side rulesets; push/schedule/dispatch kører checken rigtigt, og release-preflight forbruger tag-commit- (main-push) check-runs.
- Suggested fix: Overvej at fejle jobbet eksplicit (eller markere det neutral) frem for tavs vakuøs succes, så den påkrævede kontekst ikke fejlagtigt signalerer verificeret governance.
- Risk of fix: **Low**.


---

## 7. Dødkode- og forenklings-inventar

**Bekræftet dødkode (nul call-sites verificeret crate-wide inkl. tests, `app/fuzz/`, `src/bin/`, `app/tests/ui_smoke.rs`, `.ui`-ressourcer):**
- `EditorTab::current_line_ending_mode` (`editor_tab/runtime.rs:53-60`) — RIT-GEN-033
- `EditorTab::current_review_file` (`editor_tab/review.rs:171-179`) — RIT-GEN-033
- `ExternalFileEvent::is_acknowledged` (`editor_monitor.rs:55-61`) — RIT-GEN-033
- `Document::set_saved` (`document.rs:74-76`) — RIT-GEN-033
- `EditorTab::set_writability_for_tests` (`editor_tab.rs:431-434`) — RIT-GEN-033
- `SearchOutcome.scanned_bytes` (`large_file/search.rs:13,30,92`) — RIT-GEN-034
- `ExternalFileEvent::Moved`-pathway (`editor_monitor.rs`, `editor_tab/runtime.rs`) — RIT-GEN-035

**Forenklings-kandidater (verificeret, ikke dødkode):**
- Duplikeret `line_slices` ×3 + `ngettext`-count-mønster ×10 → delte helpers (RIT-GEN-031).
- `#[allow(clippy::too_many_arguments)]` i `git_process.rs:172` → `GitRunOptions`-struct (batch-2 Task 3; RIT-GEN-037).
- `_keep_state_alive`-tom-klon (`app.rs:86`) og vestigial `focus-project-sidebar`-test (`app.rs:509`) (RIT-GEN-037).
- `gtk_tests_v4..v13` → opportunistisk feature-navngivning; små versionsfiler foldes ind (RIT-GEN-030).

**Positiv:** ingen produktionsfil over 600-linjers-cappen; ingen `TODO/FIXME/HACK/todo!/unimplemented!/dbg!` i `app/src`; kun ét `#[allow]` i produktionskilde (dokumenteret).

## 8. Ydelses-muligheder (prioriteret, med kode-evidens)

1. **RIT-GEN-012** (Medium): pre-spawn fingerprint-short-circuit i `source_control/minimap.rs::refresh_tab_without_cancel` — stopper `git cat-file`-spawn pr. markørflytning. Største real-world-gevinst, ~10 linjer.
2. **RIT-GEN-026** (Low): fjern synkron `sync_presentation` + dirty-notify fra `connect_changed`. Batch-2 Task 1/2 dækker to tilstødende halvdele.
3. **RIT-GEN-004** (High, også perf): weak-captures i `sidebar_wiring.rs` fjerner per-vindue-lækket og den efterfølgende baggrunds-Git-polling.
4. **RIT-GEN-027** (Low): del scan-state i large-file-søgning via `Rc<RefCell<…>>` frem for at klone match-vektoren pr. chunk.
5. **RIT-GEN-021** (Low): per-fil-størrelses-query før review-worktree-læsning.
6. **RIT-GEN-036** (Info): tokenisér én gang i `compare_skip_reason` frem for at allokere linje-slices bare for at tælle.

## 9. Test-, fuzz- og stress-huller (konkrete forslag)

Prioriteret efter data-tab / parser / Git-status / save-open / large-file:

- **Git-argument-safety (RIT-GEN-001):** `git_process/tests.rs` temp-repo med glob-navngivne filer (`[id].txt`) der assertér at unstage/discard/stage rammer den eksakte fil og ikke sletter/rammer søskende. **Højeste prioritet.**
- **Ekstern-reload data-tab (RIT-GEN-002):** GTK-test der injicerer ekstern ændring, viser Reload-banner, taster tekst, udløser banner-action, assertér bufferen bevarer den tastede tekst.
- **Compare lone-`\r` display (RIT-GEN-003):** unit-test på `display_for_model` + buffer-linjeantal for CR-holdig input (nuværende regressionstest dækker kun modellen, ikke display-stien).
- **Vindue-lifecycle-lækage (RIT-GEN-004):** `DialogLeakCanary`-stil-test: byg+destruér vindue, assertér `Weak<Workspace>` ikke længere upgrader.
- **Vinduesstørrelse/sidebar-persistens (RIT-GEN-005/006):** memory-settings-tests for ren-luk-persistering og sidebar-restore uden opstartsskrivning.
- **Git-timeout-grace (RIT-GEN-007):** harness-test med stub-langsom git der assertér kill-timing ≥ grace for `kill_on_cancel=false`.
- **Large-file page-decoder-fuzz (RIT-GEN-014):** `page_text_decode`-fuzz-target + proptest på `decode_page_window`-invarianter (lukker registry-gappet).
- **CRLF-markdown (RIT-GEN-013):** parser-unit-test at `\r\n` giver `SoftBreak`, ikke paragraf-brud.
- **Open-race (RIT-GEN-011):** GTK-test der åbner to filer i samme main-loop-tur og assertér to faner.

**Suspected — kræver dedikeret seed/runtime-test:** yaml-rust2 block-style-rekursionsdybde via frontmatter (`frontmatter_split`-target med dyb `- - - -`-seed); `source_range`-drift ved U+FFFD (test der mapper range tilbage til kildebufferen).

## 10. Policy- og validerings-huller

**Intent findes, men håndhævelse mangler:**
- `cargo-sources.json`-entries uden for `static.crates.io` valideres ikke (RIT-GEN-015).
- Container-image-pinning har ingen validator (RIT-GEN-016).
- Fire release-policy-nøgler (ancestor-check, AppStream/tag-match, signing-key-import-hygiejne, github-hosted-runner) er implementeret i workflowet men ikke maskinelt håndhævet (RIT-GEN-018).
- `decode_page_window`-parser-grænsen mangler registry-mapping trods stress-fuzz-policyens `parser_or_untrusted_input_boundary_without_registry_mapping`-forbud (RIT-GEN-014).

**Håndhævelse findes, men er omgåelig:**
- `ruleset-governance`-jobbet passerer vakuøst for fork/dependabot-PR'er (RIT-GEN-038, afgrænset).
- Rollback-gaten binder til dispatch-ref frem for valideret release-ref (RIT-GEN-017).
- Publish-build checker tag-navn ud, ikke commit-SHA (RIT-GEN-028).

**Håndhævelse findes og virker adækvat (positiv sikring):**
- `cargo-sources.json` byte-for-byte i sync med `Cargo.lock` (113/113 checksums).
- Begge cargo-patch-manifester verificerer end-to-end mod pinnede upstream `.crate`-ankre; unsafe/FFI-baselines matcher; `.gitattributes`-binærmarkører til stede.
- Alle GitHub Actions SHA-pinnede; ingen cross-job-artefakt-forbrug; secrets aldrig i `pull_request`-workflows.
- Finish-args minimale (`wayland` + `fallback-x11`); bundlet Git dobbelt-pinned (tarball + `sha256sums.asc` begge sha256'd, GPG som defense-in-depth), server/netværks-binærer fjernet.
- Linjegrænser, review-evidens-ankre og parser-registry håndhæves hårdt; `tools/tests` (153) grønne.

## 11. Dokumentationsdrift

- `README.md:75` — "V14.5 is next" mod ROADMAP `next_version: v15` (V14.7 gennemført). (RIT-GEN-032)
- `README.md:34` — lister "Format" som en preferences-side; den blev fjernet i 0.3.7 (fire sider nu, Format flyttet til status-bar-menu). (RIT-GEN-032)
- `EXTERNAL-REVIEW-HANDOFF.txt` — krævet læst af `docs/fable_plan/fable_audit_prompt.md` (fil nr. 1), men findes ikke i repoet. **Afklaret benign (maintaineren, 2026-07-06):** prompten `fable_audit_prompt.md` skulle selv have heddet dette; ingen repo-fil mangler, kun en navne-uoverensstemmelse i prompten. (RIT-GEN-032)
- `docs/audit_report.md` er dateret 2026-05-27 og anker til commit `6dc24fc`; dets lukkede fund (`RIT-AUD-*`) blev genverificeret mod nuværende kode og er ikke regresseret (forventet stale historik, ikke en fejl). Undtagelse: `RIT-AUD-006` (Git-timeout-grace) blev dér markeret lukket med "force-exit after a grace window", men den adfærd leverer koden ved `7276d91` ikke — delvist genåbnet af RIT-GEN-007.
- `policy/release.policy.json:325` `verifiers.workflow_static_checks` påstår at publish-workflowets tag/version/AppStream-konsistens er static-checket; det er det ikke (indgår i RIT-GEN-018).
- `docs/mangler-og-bugs.md` og `.agent/CONTINUITY.md` er aktuelle for batch-1-arbejdet; batch-2-planen (`docs/fable_plan/2026-07-05-batch-2-hotpath-and-features.md`) er **ikke eksekveret** (se §12).

## 12. Overlap med planlagt batch-2-arbejde

`docs/fable_plan/2026-07-05-batch-2-hotpath-and-features.md` er en **planlagt, ikke-eksekveret** implementeringsplan (checkbokse uafkrydsede, anker til `681924a`; den eneste commit siden, `7276d91`, er en docs-commit). Koordinator-verificeret at alle tre kode-mål stadig er i pre-fix-tilstand:

| Batch-2-opgave | Mål | Relation til audit |
|---|---|---|
| Task 1 (dirty-notify-coalescing) | `workspace/tabs.rs:451` (direkte `notify_dirty_state_changed` stadig til stede) | **Dækker delvist RIT-GEN-026** (dirty-notify-halvdelen; `sync_presentation`-halvdelen forbliver) |
| Task 2 (`set_active_uri`-guard) | `source_control/active_row.rs:4-6` (ingen equality-guard endnu) | **Dækker en dead-code-agent-kandidat** (per-keystroke row-sweep); komplementerer RIT-GEN-026 |
| Task 3 (`GitRunOptions`, drop `#[allow]`) | `git_process.rs:172` (`#[allow]` stadig til stede) | **Dækker RIT-GEN-037**'s lint-suppression-punkt. **Adresserer IKKE RIT-GEN-007**: Task 3 tilføjer kun en kommentar om bruger-cancel-detached-child-stien, ikke timeout-grace-window-bug'en. RIT-GEN-007 forbliver åben. |
| Tasks 4-7 (print-preview-confirm, Go to Line, copy-hash, session-cursor-restore) | Nye features | Ingen overlap med audit-fund |

**Anbefaling:** Når batch-2 eksekveres, udvid Task 1 til også at fjerne den synkrone `sync_presentation` fra `connect_changed` (RIT-GEN-026), og behandl RIT-GEN-007 (timeout-grace) som et separat fix — planens Task 3-kommentar dækker det ikke.

## 13. Remediation-plan

### Skal rettes før næste offentlige beta
- **RIT-GEN-001** (Critical) — `GIT_LITERAL_PATHSPECS=1` i `git_env()`. Én linje, fjerner en hel klasse af forkert-fil-discard/index-korruption.
- **RIT-GEN-002** (High) — route banner-Reload gennem `confirm_external_reload` når dirty.
- **RIT-GEN-004** (High) — weak-captures i `sidebar_wiring.rs` (stopper vindues-lækket + baggrunds-Git-polling).
- **RIT-GEN-003** (High) — foren linje-tokenisering + sanitér interiør `\r` i compare-buffere.
- **RIT-GEN-005 / RIT-GEN-006** (High¹ — UX/persistens, lav datatabsrisiko) — persistér vinduesstørrelse på enhver ren luk; stop opstarts-overskrivning af `project-sidebar-visible`. Ret tidligt fordi de er små/sikre, ikke fordi de er samme risikoklasse som datatab-fundene.

### Bør rettes snart
- **RIT-GEN-007** (Medium) — gør timeout-grace-vinduet reelt for mutations-ops (ikke dækket af batch-2).
- **RIT-GEN-008 / RIT-GEN-010** (Medium) — index-lock-gate for actions; reschedule refresh efter diff-annullering (delt-cancellable-smell).
- **RIT-GEN-011** (Medium) — `acquire_open_target` springer loading/pending faner over.
- **RIT-GEN-012** (Medium) — pre-spawn fingerprint-check i minimap-refresh.
- **RIT-GEN-009** (Medium) — escape Git-stier i review-fanen.
- **RIT-GEN-013** (Medium) — CRLF-normalisering.
- **RIT-GEN-014 / RIT-GEN-015 / RIT-GEN-016 / RIT-GEN-017 / RIT-GEN-018** (Medium) — luk parser-registry-, supply-chain- og release-håndhævelses-hullerne.

### Opportunistisk oprydning
- **RIT-GEN-019..037** (Low/Info) — cancellable-hygiejne, whitespace-diff-kant, zoom-provider-Drop, encoding-dialog-shell, per-keystroke-oprydning (koordinér med batch-2 Task 1/2), test-only-gating, `gtk_tests`-navngivning, duplikat-helpers, dødkode-sletning, README-drift, micro-observationer.

### Kræver manuel UX/runtime-verifikation (se §"Ikke-verificerede observationer")
- Session-wipe ved vindues-dispose; `FileSaver`-cancel-atomicitet; encoding-chooser på redigeret fane; discard-snapshot-staleness; symlink-repo-prefix; stale-snapshot-row-actions; yaml-rekursionsdybde; `source_range`-drift; Find-in-Files-guidance i skjult sidebar; RTL-tema-checkmark; signing-secret-scoping / `RULESET_GOVERNANCE_TOKEN`-scope; container-tag-drift.

---

## Ikke-verificerede observationer / kræver runtime-test

Disse er **ikke** bekræftede fund; de er hypoteser der kræver runtime eller live-API for at afgøre.

- **Session-wipe ved vindues-destroy** hvis `AdwTabView::dispose` udsender `page-detached` mens `Workspace` stadig er stærkt refereret: `on_page_detached` kalder ubetinget `persist_session_state_if_needed()` uden `allow_window_close`-guard (`workspace_close.rs:143`). Statisk ser destroy-handleren ud til at droppe sidste stærke ref først, men signal/dispose-rækkefølge kan ikke bevises statisk.
- **`FileSaver::save_async` annulleret midt i skrivning** (`state.rs:151-154`): GIO `g_file_replace` bør være atomisk (temp+rename), men det er en GIO-garanti, ikke app-kode. Stress-test (hurtig save/save-as-veksling på multi-MB-fil) ville lukke det.
- **Decode-failure-encoding-chooser** anvendt på en fane brugeren redigerede imens (`open.rs:418-471`) — dialogerne er modale, så redigering burde være umulig; kun relevant hvis en ikke-modal sti findes.
- **Discard-flow mangler snapshot-staleness-guard** (`actions.rs:187-207`) som diff-flowet har; afhænger af modalitet-garantier under vilkårligt lange dialog-ventetider.
- **`saved_file_in_repo` prefix-check vs. symlinkede stier** (`live.rs:206-211`): `path.starts_with(repo)` fejler for et dokument åbnet gennem et symlink af repo-roden.
- **Row-actions forbliver aktive på et stale snapshot efter refresh-fejl** (`refresh.rs:99-109`) — indholdssikkert for stage (hasher nuværende fil), men lader brugeren handle på forældet status.
- **`no_history_error` matcher enhver stderr med "your current branch"** (`log.rs:95-99`) — en urelateret fatal med den frase misklassificeres som tomt-historik-repo.
- **`GitRepoContext::parse` trimmer whitespace/splitter på newlines** (`repo.rs:20-56`): en repo-sti med trailing spaces mangles; en sti med `\n` fejler parsing.
- **yaml-rust2 block-style-rekursionsdybde** via frontmatter (`frontmatter.rs:39`): loaderens block-sti har ingen eksplicit depth-cap; dyb `- - - -`-nesting kan risikere stack-udmattelse. Ingen crash-artefakter findes; kræver targeted seed.
- **`source_range`-drift** når body indeholder U+FFFD (`parser.rs:33-34,502-504`): U+FFFD (3 bytes) → space (1 byte) skrumper strengen, så `source_range` efter en replacement-char forskydes; aktuelt latent (ingen feature mapper range tilbage til kildebufferen).
- **Synkron fuld parse+render på UI-tråden for ≤5 MiB Markdown** (`markdown_preview.rs:215-221`) — bounded, men potentielt synligt hak på en patologisk 5 MiB-fil.
- **Find in Files med ingen mappe viser sin vejledning i en skjult sidebar** (`search_coordinator.rs:15-27`) — beskeden kan være usynlig hvis panelet står på position 0.
- **RTL-rendering af tema-radio-checkmarket** (`appearance.css:26-35`, fast `transform: translate(27px, 14px)`) — sandsynligvis fejlplaceret i RTL-locales.
- **Signing-secret-scoping / `RULESET_GOVERNANCE_TOKEN`-scope** (kræver live GitHub-API): om `FLATPAK_GPG_*` er environment-secrets vs. repo-secrets, og PAT-scope for governance-token, kan ikke verificeres offline.
- **Container-tag-drift** (`fedora:42`, `gnome-50`): om tags har flyttet sig under tidligere grønne runs kræver registry-forespørgsler.

---

## Appendix

### Kommando-log (koordinator, alle bestået medmindre andet er noteret)
```
git rev-parse HEAD                         → 7276d91
git diff --check                           → OK
scripts/dependency-preflight --root app    → OK
scripts/integration-preflight              → OK
glib-compile-schemas --strict --dry-run app/data/schemas    → OK
msgfmt --check-format --check-header -o /dev/null app/po/da.po → OK
desktop-file-validate app/data/io.github.cadric.Riteed.desktop → OK
appstreamcli validate --no-net --pedantic …metainfo.xml → OK (1 pedantic note)
flatpak-builder --show-manifest …Riteed.yml → OK
cargo fmt --all --check                    → OK
cargo check --workspace --all-targets --all-features → OK
cargo clippy --workspace --all-targets --all-features -- -D warnings → OK
cargo test … (RUST_TEST_THREADS=1, G_DEBUG=fatal-criticals) → OK, 403 passed
tools.policy_check --root app --strict     → OK
tools.coverage_check --root app            → OK, 82.0% line coverage
cargo fuzz list                            → OK (5 targets)
cargo +nightly fuzz run <target> -- -runs=1000 (×5) → OK
python3 -m unittest discover -s tools/tests → OK, 153 tests (1 skipped)
```

### Repræsentative grep-forespørgsler
```
rg "GIT_LITERAL_PATHSPECS|:\(literal\)" app/src            → 0 hits (RIT-GEN-001)
rg "tokenize_lines|split_inclusive" editor_tab/compare/    → RIT-GEN-003
rg "docker|container|image|@sha256|ghcr" tools/            → RIT-GEN-016 (0 relevante)
rg "is-ancestor|merge-base" tools/                         → RIT-GEN-018
rg "GNUPGHOME|gpg-agent|preset|mktemp|kill" tools/         → RIT-GEN-018 (0)
rg "#\[allow\(" app/src                                    → 1 hit (git_process.rs:172)
rg "TODO|FIXME|HACK|todo!|unimplemented!|dbg!" app/src     → 0 hits
```

### Ikke-reviderede/kun-skimmede filer
- Compare visuelle lag (`hatch`, `minimap`, `layout`, `navigation`, `reveal`, `viewport`) og tree-view-internals blev skimmet, ikke linje-auditeret.
- Fuld Flatpak-build og nightly stress/ASan/Valgrind blev ikke kørt (kun statisk inspiceret).
- Live GitHub ruleset/environment/secret-state (dækkes af CI `ruleset-governance`).

### Antagelser
- Toolchain: Rust 1.95.0 (som `rust-toolchain.toml`), nightly 1.97 til fuzz.
- `cargo-fuzz 0.13.1` fundet i `~/.cargo/bin` (uden for PATH i standard-shell; kørt via `cargo fuzz`).
- GSettings memory-backend antaget testudgave; ikke aktiv i produktion (RIT-GEN-029 er om kildeoverflade, ikke runtime-brug).

### Afviste false positives
- **`app/data/schemas/gschemas.compiled`** som committet binær: git-ignoreret (`.gitignore:35`), ikke tracked — ingen fund.
- **`cid-contains-uppercase-letter`** (appstreamcli pedantic) — forventet for `io.github.cadric.Riteed`, ikke et fund.
- **"Latent lint-bombe"** fra ugatede test-fns (dead-code-agent-hypotese): koordinatorens `cargo clippy -D warnings` + `cargo check --all-targets` bestod rent; `pub`-items er usynlige for dead_code-linten, så bomben materialiserede sig ikke. Kun vedligeholdelses-pointen i RIT-GEN-029 står.

---

## Afsluttende kvalitetsvurdering — hvor vi står

Samlet er dette **solidt håndværk**, klart over gennemsnit for et beta-projekt
med én primær udvikler: `forbid(unsafe)`, `deny(warnings)`, clippy pedantic,
forbud mod `unwrap`/`expect`/`panic` i runtime, 82 % dækning, ægte
fuzz/stress-infra og policy-as-code der faktisk kører i CI. Modulstrukturen er
ren og ansvarsopdelingen tydelig. Det føles gennemtænkt, ikke hacket sammen.

**Én dominerende fejlklasse.** Næsten alle substantielle fund klumper sig om
asynkron callback-livscyklus — delte `Cancellable`s og `Rc`-ejerskab på tværs af
`workspace` / `editor_tab` / `source_control`. Git-subprocess-timeouten
(RIT-GEN-007), reload-flowet (RIT-GEN-002), open-racet (RIT-GEN-011),
commit/diff-refresh-cancellablen (RIT-GEN-010/019) og Rc-cyklus-lækken
(RIT-GEN-004) er alle samme mønster. Det er ikke sjusk; det er den sværeste del
af gtk-rs, men den delte-cancellable-model er en "smell" der bliver ved med at
føde bugs. Hvis noget skal strammes arkitektonisk, er det dét: separate
cancellable-slots og konsekvent weak-capture.

**For løst kun ét farligt sted:** Git-argument-konstruktionen (RIT-GEN-001), som
skiller sig ud netop fordi resten er så omhyggeligt. UI-state-persistering
(vinduesstørrelse RIT-GEN-005, sidebar RIT-GEN-006) har et par "virker kun på én
sti"-bugs, der antyder at close/lifecycle-flowene ikke er tracet helt til bunds.

**For stramt:** policy-maskineriet er stort i forhold til appen. Meget er reelt
værdifuldt, men noget er ceremoni — og ironisk nok er en del af
policy-*intentionen* ikke faktisk håndhævet (fire release-nøgler RIT-GEN-018;
cargo-sources-allowlist RIT-GEN-015; container-image-pinning RIT-GEN-016), så
strammingen er delvist teater nogle steder. 600-linjers hard-cap med waivers er
nok overkill for et solo-projekt og skaber selv lidt af den duplikering der er
fundet (`line_slices` ×3 RIT-GEN-031, count-formatering ×10).

**Et blindt punkt værd at holde øje med:** nogle tests asserterer **konstanter**
i stedet for adfærd. `git_operations_have_wall_clock_timeout_and_kill_grace`
tjekker at grace-konstanten er 2 sekunder — ikke at grace-vinduet virker, hvilket
er præcis hvorfor RIT-GEN-007 slap igennem trods en tidligere "lukket"-markering
i `docs/audit_report.md`. Testene ser dækkende ud, men gør det ikke altid.

**Bundlinje:** ret de fem-seks store fund (Critical + High), og det her er kode
man roligt kan stole på i daglig brug. De skarpe kanter er få og koncentrerede,
ikke spredt ud over kodebasen.
