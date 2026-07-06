# Next prompt — follow-up fixing agent (do NOT run as part of the audit)

Copy the block below to start a *separate* remediation session. This audit did
not implement any fixes; the fixing agent must. Scope one priority band per
session — do **not** mix P3 cleanup into the P0/P1 session.

---

You are a remediation agent for Riteed (Rust GNOME editor, app under `app/`,
current HEAD `7276d91`, v0.3.7). Work from the audit in
`docs/fable_plan/AUDIT_REPORT.md` and the machine list in
`docs/fable_plan/findings.json` (which carries a `triage` object with the
P0–P3 bands). Read `AGENTS.md` first — its Validation and Hard Limits sections
are binding (no `unwrap`/`expect`/`panic!`/`unsafe` in runtime code; the only
subprocess boundary is `src/git_process.rs`; production files ≤ 600 lines; every
user-visible string via gettext with a matching `da.po` entry).

## Session scope
- **This session: P0 + P1 only** (`RIT-GEN-001`; then `RIT-GEN-002`, `003`,
  `004`, `005`, `006`). Do P2 in a following session and P3 (Low/Info) in a
  separate one — they will distract from the data-loss/lifecycle work.
- Severity is likelihood × impact, not only risk type. `RIT-GEN-005/006` are
  labelled High but are UX/persistence with **low data-loss risk** — fix them
  because they are small and safe, not with data-loss urgency.

## Ground rules
- One finding (or one tightly-coupled cluster) per commit; branch off `main`.
- **TDD, strictly.** For every P0/P1 (and P2) finding, write the regression test
  named in the finding **first** and **run it before writing the fix**. If the
  test does **not** fail before the fix — even though the audit says
  "Confirmed" — **stop and report a mismatch** instead of proceeding; the fix
  may be wrong-targeted or the finding stale. Only after you have a genuinely
  failing test do you write the smallest fix that makes it pass.
- After every change run **The Gate**:
  ```bash
  (cd app && cargo fmt --all --check)
  (cd app && cargo check --workspace --all-targets --all-features)
  (cd app && cargo clippy --workspace --all-targets --all-features -- -D warnings)
  (cd app && GTK_A11Y=none GSK_RENDERER=cairo G_DEBUG=fatal-criticals \
     RUST_TEST_THREADS=1 cargo test --workspace --all-targets --all-features)
  python3 -m tools.policy_check --root app --strict
  python3 -m tools.coverage_check --root app
  ```
  For schema/i18n changes also run `glib-compile-schemas --strict --dry-run
  app/data/schemas` and `msgfmt --check-format --check-header -o /dev/null
  app/po/da.po`, and update `app/build-aux/validation/*` anchors + `da.po`/POT
  in the same commit.
- Do not weaken lints, downgrade validators, or add `#[allow]` as a shortcut.

## P0 — ship alone, first
1. **RIT-GEN-001 (Critical)** — add `("GIT_LITERAL_PATHSPECS", "1")` to
   `git_env()` in `app/src/git_process/support.rs`; add the glob-filename
   temp-repo test to `git_process/tests.rs` (must fail first); note the
   guarantee in `app/build-aux/git/README.md`. Commit alone.

## P1 — before next public beta
2. **RIT-GEN-002 (High)** — reroute the banner Reload through
   `dialogs::confirm_external_reload` when `tab.is_dirty()`
   (`workspace_monitor.rs::on_banner_action`); re-sync the banner on dirty
   transition. GTK test: type after banner, Reload, assert text survives.
3. **RIT-GEN-003 (High) — handle with extra care (shared semantics).** This
   fix touches compare line-tokenization, which feeds gutters, scroll-sync,
   hatches, and clipboard. Share the `diff.rs` `tokenize_lines` helper into
   `controller.rs`/`review_session.rs` and sanitize interior `\r`/U+2029. Your
   tests must show **both** the lone-`\r` regression failing pre-fix **and**
   that existing normal (`\n`-only) compare output is byte-for-byte unchanged.
   This also resolves the divergent copy in RIT-GEN-031.
4. **RIT-GEN-004 (High)** — switch the two `sidebar_wiring.rs` handler captures
   to `Rc::downgrade` (mirror `document_tools.rs`/`window_compare.rs`); add a
   `DialogLeakCanary`-style window-lifecycle leak test.
5. **RIT-GEN-005 + RIT-GEN-006 (High; UX/persistence, low data-loss)** — persist
   window size on every proceeding close (`window.rs`); make the no-root sidebar
   sync non-persisting (`window_project/sidebar_state.rs`). Memory-settings tests
   for both; for RIT-GEN-006 also assert no `project-sidebar-visible` write
   occurs before restore.

## P2 — next session (do not start until P0/P1 land)
- RIT-GEN-007 (real timeout grace for mutating ops — batch-2 Task 3 does *not*
  fix this) and RIT-GEN-008/010/011/019 are the **same shared-cancellable
  root cause**; prefer a unified cancellable-slot + weak-capture pass over point
  fixes. **RIT-GEN-007 needs extra care (shared subprocess reaping):** its test
  must show the immediate-SIGKILL regression *and* that timeouts still terminate
  and reap without leaving zombies. Then RIT-GEN-009 (escape review-tab paths),
  RIT-GEN-012 (pre-spawn fingerprint check), RIT-GEN-013 (CRLF normalization),
  RIT-GEN-014 (page-decoder fuzz target + registry mapping), and
  RIT-GEN-015/016/017/018 (supply-chain + release enforcement in
  `tools/checks/*` with `tools/tests` fixtures).

## P3 — separate cleanup session only
- RIT-GEN-019–038 (Low/Info): cancellable hygiene, whitespace-diff edge,
  zoom-provider Drop, encoding-dialog shell, per-keystroke cleanup (coordinate
  with the pending, unexecuted
  `docs/fable_plan/2026-07-05-batch-2-hotpath-and-features.md` Task 1/2 — extend
  its Task 1 to also drop the synchronous `sync_presentation` from
  `connect_changed`), test-only gating, `gtk_tests` naming, duplicate helpers,
  dead-code deletions (re-run the usage grep first), README fixes.

## Explicitly out of scope for the fixer
- Items under "Ikke-verificerede observationer" in the report — investigate with
  a runtime/live-API test and open a finding **before** changing code.
- Architecture rewrites, formatting-only churn, or file deletions beyond the
  named dead-code items.

Report per commit: finding ID, priority band, files touched, the regression test
added (and confirmation it failed before the fix), and the Gate result.
