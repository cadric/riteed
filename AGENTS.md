# AGENTS — Root Contract (Strict GNOME Rust Application)

This repository contains the authoritative policy and validation tooling for native GNOME desktop applications written in Rust, plus the Riteed application under `app/`. The root remains the single source of truth for `AGENTS.md`, `policy/`, `tools/`, and `scripts/`; the in-tree app is validated by pointing the root tooling at `app/`.

## Golden Rules
- `native_gnome_first`
- `libadwaita_required`
- `flatpak_sandbox_first`
- `portal_first`
- `no_unsafe`
- `localizable_by_default`
- `gsettings_for_preferences`
- `app_id_is_authoritative`
- `strict_over_convenient`

## Core Workflow
0. Read `.agent/CONTINUITY.md` if it exists. Treat it as continuity only, never as policy.
0.4. Use Web search and context7 for up to date code information always.
0.5. Read `policy/README.md` for scope mapping and review-artifact rules when policy or validation work is in scope.
1. Identify every affected policy surface before editing: Rust, gtk4-rs, libadwaita, HIG, gettext/i18n, GSettings, Flatpak metadata, and validation tooling.
2. Read the current implementation and summarize the present behavior.
3. Plan the smallest safe native-GNOME change.
4. Implement directly with focused edits.
5. In this repository, validate the in-tree app with `python3 -m tools.policy_check --root app --strict`.
6. In this repository, validate the in-tree app with `python3 -m tools.coverage_check --root app`.
7. Update resources, metadata, schemas, translations, and docs when behavior or user-visible strings change.

## Architecture Summary
- Target app type: native GNOME desktop application.
- Language: Rust `1.95.x`, edition `2024`, stable only.
- UI stack: `gtk4-rs` + `libadwaita` only.
- Styling: Adwaita first, custom CSS only for app-specific additions.
- Localization: GNU gettext via `gettext-rs`.
- AppStream identity, app description, and screenshot copy are localizable.
  AppStream release descriptions may exist for GNOME Software/Flatpak version
  history, but they must use `translate="no"` and stay out of POT/PO catalogs.
  `CHANGELOG.md` and GitHub Releases remain the full release-note sources.
- Preferences and lightweight durable state: `GSettings`.
- Packaging and sandbox: Flatpak-first on `org.gnome.Platform//50`.
- Assets: packaged UI, CSS, shortcuts, and related app assets must be resource-backed.
- Permissions: minimal by default; prefer portals over static sandbox permissions.

## Required Policies
Load and enforce these whenever their scopes apply:
- `policy/gnome-rust-app.bundle.json`
- `policy/rust.policy.json`
- `policy/gtk4-rs.policy.json`
- `policy/libadwaita.policy.json`
- `policy/hig.policy.json`
- `policy/gettext-i18n.policy.json`
- `policy/gsettings.policy.json`
- `policy/flatpak-metadata.policy.json`
- `policy/release.policy.json`
- `policy/stress-fuzz.policy.json`
- `policy/validation-tooling.policy.json`

## Validation
Primary gate for the in-tree app:
1. `python3 -m tools.policy_check --root app --strict`

Policy/tooling/contract-root gate when `AGENTS.md`, `policy/`, `tools/`, or
`scripts/` changes:
2. `python3 -m tools.policy_check --policy-pack-check --strict`

Policy validation reports Rust source categories from `source_scope`; every
source must have one declared validation owner set. This is scope accounting,
not a test-coverage claim. See `docs/policy-validation.md` for automatic,
delegated and human-review boundaries. Explicit `--root` never falls back to
another target. Runtime review fields must satisfy the policy's typed schema.

Coverage gate for the in-tree app:
3. `python3 -m tools.coverage_check --root app`

Direct fallback commands:
System gettext is encoded through the `gettext-rs/gettext-system` Cargo feature in `app/Cargo.toml`; keep these direct Cargo commands usable without manual environment overrides.
- `cd app && cargo fmt --all --check`
- `cd app && cargo check --workspace --all-targets --all-features`
- `cd app && cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cd app && cargo test --workspace --all-targets --all-features`
- `cd app && cargo llvm-cov --workspace --all-features --json --summary-only`
- `glib-compile-schemas --strict --dry-run app/data/schemas`
- `msgfmt --check-format --check-header -o /dev/null app/po/<catalog>.po`
- `desktop-file-validate app/data/<application-id>.desktop`
- `appstreamcli validate --no-net --pedantic app/data/<application-id>.metainfo.xml`
- `flatpak-builder --show-manifest app/build-aux/<application-id>.yml`

## Branch Integration Gate for Local Flatpak Builds
Before any local Flatpak test build, report:
- `git branch --show-current`
- `git status --short --branch`
- `git branch --no-merged main`

Local Flatpak test builds normally run only from `main` or `integrate/*`.
Feature-branch Flatpak builds are allowed only when the user explicitly asks
for a feature-only build; report them as partial and not representative of all
unmerged local work.

Use `scripts/local-flatpak-build` for local test builds. It runs
`scripts/integration-preflight` before `flatpak-builder` and then verifies the
installed user Flatpak with `flatpak info --user io.github.cadric.Riteed`.
If relevant parallel feature branches exist, stop and create or update an
integration branch before building.

## Hard Limits
- Production source and enforced metadata default to `600` total lines. Test files matched by `test_file_globs` may reach `800` total lines. Production files over `600` require a registered line-limit waiver with `scope`, scope-relative path, reason, finding ID, review date, and frozen per-file cap no higher than `720`; inline `#[cfg(test)]` in a production file still counts as production. Gettext `po/*.po` and `po/*.pot` catalogs/templates remain exempt from generic line-count enforcement and covered by gettext extraction, i18n review, and `msgfmt`.
- No runtime Rust path may use `unsafe`, `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `dbg!`, or external command spawning, except the reviewed typed `/app/bin/git` Gio subprocess boundary in `src/git_process.rs`.
- Synchronous runtime filesystem probes require `runtime-sync-fs` review evidence and must stay native-only; portal, FUSE, and user-selected project/document paths should use async Gio APIs.
- No broad Flatpak permissions.
- No non-GNOME UI framework.
- No gettext bypass for user-visible strings.
- No ad hoc config files for preferences.
- No warning relaxation, lints suppression, or validator downgrades as a shortcut.
- Only `src/settings/appearance.rs` may force Light/Dark color schemes to honor explicit user theme choices; every other forced color scheme remains forbidden.

## Identity and Consistency
The application ID is authoritative and must stay consistent across:
- `GApplication` application ID
- Flatpak manifest `id`
- desktop file basename
- metainfo component ID
- GSettings schema ID or prefix
- gettext domain
- resource prefix
- exported icon basename
- D-Bus name when present

## Desktop UI Expectations
- Use `adw::Application` and `adw::ApplicationWindow`.
- Use GTK resources for packaged UI and CSS.
- Use adaptive libadwaita patterns, not custom shell architecture.
- Use action-based commands and standard GNOME shortcuts.
- Keep UI work on the main loop and move long-running work off the UI thread.
- Keep user-facing copy clear, localizable, and aligned with the GNOME HIG.

## Secrets, Privacy, and Storage
- Never store secrets in `GSettings`.
- Never commit tokens, keys, or credentials.
- Treat logs, settings values, and crash diagnostics as potentially sensitive.
- Use app sandbox paths and portals; do not assume host filesystem access.

## Dependency Rules
- Prefer the Rust standard library and GNOME stack crates.
- New dependencies require a clear reason and must not weaken sandbox, i18n, or safety constraints.
- `gtk4`, `libadwaita`, and `gettext-rs` are required runtime crates for the primary app package.
- GTK/GNOME binding updates must follow `docs/dependency-updates.md` so safe bindings, `*-sys` crates, fuzz lockfiles, and Flatpak cargo sources stay coordinated.
- Release workflow, signing, rollback, Pages remote, GitHub ruleset governance, signing-key, and local release-critical patch changes must follow `policy/release.policy.json`.
- Release-gating workflow job containers and supported literal `docker pull`/`docker run` image operands must use exact `@sha256:` digests; update them through the reviewed image procedure in `docs/dependency-updates.md`.
- Manual release workflow dispatch may target an explicit `v*` release tag only
  when preflight validates that tag, verifies exact release-critical checks on
  its commit, the monotonic rollback gate uses that validated tag as its
  candidate ref, and the build job checks out the preflight's immutable commit
  SHA before signing.
- Release preflight must peel the validated tag to one full commit SHA, keep
  that commit on fetched `origin/main`, and read Cargo/AppStream content from
  that object. Check-run selection, rollback provenance, build checkout and
  the checked-out HEAD assertion must use the same SHA. Private-key import must
  run only after that assertion on the policy-approved hosted Ubuntu build,
  inside a mode 700 temporary `GNUPGHOME` whose agent and files are removed on
  exit.
- Release governance must keep offline policy checks deterministic; live GitHub ruleset/environment checks belong only in the token-scoped governance job and must enforce the exact reviewed actors from release policy.
- Pull-request governance uses an unconditional tokenless `governance-static`
  context. Protected-main `governance-live` may run only for the policy-owned
  push, schedule, and manual events, and publish evidence must bind its newest
  check to the exact run, job, commit, repository, and successful decisive step.
- Local release-critical crate patches must keep their patch manifest, upstream `.crate` anchor, allowed-file diff checksum, unsafe/FFI baseline, and binary artifact marker in sync.
- Parser, untrusted-input, fuzz, and stress-boundary changes must follow `policy/stress-fuzz.policy.json` and keep parser-boundary evidence current.
- Every policy-required fuzz target must have an exact Cargo bin registration, policy-declared semantic seeds, parser-boundary coverage, and an active scheduled/manual CI invocation.
- `gettext`, alternate GUI frameworks, generic config crates, and broad async runtimes are forbidden unless policy is explicitly revised.
- `Cargo.lock` must be committed.

## Stop or Escalate If
- A change introduces or requires `unsafe`, FFI, or native code outside approved GNOME bindings.
- A change adds broad Flatpak permissions or bypasses portals.
- A change weakens lint levels, validation gates, or coverage thresholds.
- A change introduces non-localizable user-visible strings.
- A change replaces `GSettings` with custom config persistence.
- A change breaks application ID consistency across metadata surfaces.
- A known audit or policy gap is left as a prose-only TODO instead of typed `planned_remediation`.
- A change requires runtime downloads, shell execution, host helpers, or external helpers outside the reviewed typed `/app/bin/git` boundary.

## Guardrails
- Do not overwrite unrelated local user changes.
- Prefer policy-compliant rewrites over exemptions.
- Treat every validator warning as a failure condition.
- If validation fails, stop and fix the code or metadata. Do not suppress the failure.

## Approval Protocol
- Do not show diffs by default.
- Require explicit approval only for destructive edits or edits that touch user-modified regions.
- For approval-gated edits, present the targeted diff first and then stop.
- Accepted approvals: `approve`, `approved`, `yes`, `go ahead`, `proceed`, `apply the patch`.

## Precedence and Scope
- This file applies repository-wide.
- Nested `AGENTS.md` files apply only to their subtree.
- Policy JSON files apply by their declared path globs.
- Apply the union of constraints.
- When rules conflict, the stricter rule wins.
- Hard-fail validation tooling is authoritative for machine-enforced checks.
