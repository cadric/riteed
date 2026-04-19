# AGENTS — Root Contract (Strict GNOME Rust Application)

This repository defines policy and validation tooling for native GNOME desktop applications written in Rust. The validators are intended to run against a target application repository that vendors this pack; this repository is the policy pack itself, not the target app layout.

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
0.5. Read `policy/README.md` for scope mapping and review-artifact rules when policy or validation work is in scope.
1. Identify every affected policy surface before editing: Rust, gtk4-rs, libadwaita, HIG, gettext/i18n, GSettings, Flatpak metadata, and validation tooling.
2. Read the current implementation and summarize the present behavior.
3. Plan the smallest safe native-GNOME change.
4. Implement directly with focused edits.
5. In a target application repository that vendors this pack, run `python3 -m tools.policy_check --strict`.
6. In a target application repository that vendors this pack, run `python3 -m tools.coverage_check`.
7. Update resources, metadata, schemas, translations, and docs when behavior or user-visible strings change.

## Architecture Summary
- Target app type: native GNOME desktop application.
- Language: Rust `1.95.x`, edition `2024`, stable only.
- UI stack: `gtk4-rs` + `libadwaita` only.
- Styling: Adwaita first, custom CSS only for app-specific additions.
- Localization: GNU gettext via `gettext-rs`.
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
- `policy/validation-tooling.policy.json`

## Validation
Primary gate:
1. `python3 -m tools.policy_check --strict`

Coverage gate:
2. `python3 -m tools.coverage_check`

Direct fallback commands:
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets --all-features`
- `cargo llvm-cov --workspace --all-features --json --summary-only`
- `glib-compile-schemas --strict --dry-run data/schemas`
- `msgfmt --check-format --check-header -o /dev/null po/<catalog>.po`
- `desktop-file-validate data/<application-id>.desktop`
- `appstreamcli validate --no-net --pedantic data/<application-id>.metainfo.xml`
- `flatpak-builder --show-manifest build-aux/<application-id>.yml`

## Hard Limits
- No source or enforced metadata file may exceed `600` total lines.
- No runtime Rust path may use `unsafe`, `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, `dbg!`, or external command spawning.
- No broad Flatpak permissions.
- No non-GNOME UI framework.
- No gettext bypass for user-visible strings.
- No ad hoc config files for preferences.
- No warning relaxation, lints suppression, or validator downgrades as a shortcut.

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
- `gettext`, alternate GUI frameworks, generic config crates, and broad async runtimes are forbidden unless policy is explicitly revised.
- `Cargo.lock` must be committed.

## Stop or Escalate If
- A change introduces or requires `unsafe`, FFI, or native code outside approved GNOME bindings.
- A change adds broad Flatpak permissions or bypasses portals.
- A change weakens lint levels, validation gates, or coverage thresholds.
- A change introduces non-localizable user-visible strings.
- A change replaces `GSettings` with custom config persistence.
- A change breaks application ID consistency across metadata surfaces.
- A change requires runtime downloads, shell execution, or external helpers.

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
