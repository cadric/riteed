---
created: 2026-04-19
updated: 2026-04-19
status: current
priority: high
type: release
---

# Changelog

All notable changes to this repository are documented in this file.

The format follows Keep a Changelog and the repository follows Semantic Versioning.

## Unreleased

### Added
- Embedded `app/` as the canonical Riteed application subtree, including the GTK4/libadwaita plain-text editor, Flatpak metadata, gettext catalogs, GSettings schema, review artifacts, and vendored Cargo dependency tree for offline Flatpak-oriented builds.
- Extended the embedded Riteed app to a tabbed v2 editor with multi-document tabs, recent files, session restore, unsaved-close coordination, and drag-and-drop file opening.
- Extended the embedded Riteed app to a v3 editor with in-document search and replace, optional line numbers, and a bottom status bar for file and cursor feedback.
- Extended the embedded Riteed app to a v4 editor with syntax highlighting, an optional minimap, and conservative external-file monitoring that protects unsaved work.

### Changed
- Simplified the repository layout so `AGENTS.md`, `policy/`, `tools/`, and `scripts/` live only at the root while `app/` validates directly against the root contract.
- Refactored the embedded Riteed window controller into smaller workspace, tab, close-flow, session, and I/O modules so the GNOME app remains policy-compliant and maintainable as features grow.
- Migrated the embedded Riteed editor core from `GtkTextView` to `GtkSourceView`, moving dirty-state tracking onto the buffer modified flag for better performance on longer text files.
- Made the embedded Riteed editor more code-friendly by auto-detecting languages for highlighting, keeping the minimap optional, and surfacing external file changes through in-tab banners and guarded save/reload flows.

### Fixed
- Stabilized GTK coverage runs by teaching `tools.coverage_check` to invoke `cargo llvm-cov` with a deterministic `GSK_RENDERER=cairo` environment, with unit coverage for the new tool behavior.
- Ignored embedded app build outputs such as `app/target/`, Flatpak cache directories, and app-local coverage directories so routine local validation does not keep leaving commit noise behind.
- Polished the embedded Riteed v2 tabbed editor so the editor fills the full window, the tab bar hides when only one document is open, and file drag-and-drop still opens documents in tabs instead of inserting links into the text view.
- Replaced external F1 help launching with an in-app help dialog and shortened recent-file menu labels so the primary menu stays more compact.
- Corrected the embedded Riteed v4 external-file monitor flow so atomic saves from other editors mark documents stale correctly, selected tabs show reload banners instead of silently reloading, and manual reload actions apply immediately.
- Restored the embedded Riteed minimap synchronization by removing the incorrect shared scroll-adjustment wiring that desynced the viewport preview.
- Restored the embedded Riteed development icon setup so About and the window icon can resolve the app icon during local `cargo run` workflows, and fixed clean window close requests so an untouched window no longer needs a second click to close.
- Switched the embedded Riteed About dialog to a dedicated full-color app icon alias so it no longer resolves to the symbolic icon variant.

## 1.0.1 — 2026-04-19

### Fixed
- Corrected Flatpak and AppStream validation commands to supported official forms and restored `--no-net --pedantic` for AppStream validation.
- Restored tolerant crate-root lint regexes, fixed the broken `extern "C"` detector, tightened Flatpak manifest discovery, and narrowed required runtime-crate checks to the root application package.
- Moved coverage tooling back into machine-readable coverage policy instead of requiring `cargo-llvm-cov` during the primary policy gate.
- Realigned HIG primary-menu enforcement with the current GNOME HIG: allow up to 12 items, require `about`, and forbid `quit`/`close` in primary menus.
- Clarified bundle scope, repo-root detection, review-artifact semantics, and template-file coverage for `.desktop.in.in` and `.metainfo.xml.in.in` sources.

## 1.0.0 — 2026-04-19

### Added
- Package-safe validator layout under `tools.checks` and `tools.scanners`, plus canonical `python3 -m tools.policy_check` and `python3 -m tools.coverage_check` entrypoints with direct-script compatibility.
- Deterministic review-artifact contract under `build-aux/validation/`, documented in `policy/README.md`, with shard discovery, line-anchored matching, and two-way coverage enforcement.
- Maintainer-only `--update-artifact-index` support for regenerating bundle policy hashes.
- Unit coverage in `tools/tests/test_policy_check.py` for entrypoints, site matching, and artifact-index upkeep.

### Changed
- Promoted the remaining HIG, libadwaita, gettext, GSettings, and runtime blockers into deterministic validator rules using hard-fail checks plus `review_required` evidence.
- Added `xgettext` as a required validator tool and switched gettext completeness checks to normalized extractor output instead of heuristic-only scanning.
- Extended hard line-limit enforcement to `scripts/**` and `policy/README.md`.

## 0.1.1 — 2026-04-19

### Fixed
- Aligned policy IDs, bundle metadata, and artifact indexing with the shipped policy files.
- Corrected validator behavior for Flatpak manifest filenames, JSON source pinning checks, permission justifications, gettext bootstrap detection, conditional GSettings enforcement, and conditional gresource requirements.
- Tightened machine-readability by enforcing line limits on AGENTS/policy/tooling files, removing unused required tools, adding missing required tools, and replacing overbroad glob and regex matching.
- Clarified that validation commands target application repositories that vendor this pack, not the policy-pack repository itself.

## 0.1.0 — 2026-04-19

### Added
- Initial machine-readable policy bundle for strict native GNOME applications written in Rust.
- Policy files for Rust, gtk4-rs, libadwaita, GNOME HIG, gettext/i18n, GSettings, Flatpak metadata, and validation tooling.
- Hard-fail validation tooling for policy enforcement, maximum file size enforcement, and minimum Rust coverage enforcement.
- Root `AGENTS.md` contract for LLM coding agents working against the policy bundle.

### Changed
- Tailored the repository contract and validation model away from the earlier Go-focused setup and toward a strict GNOME Rust application workflow.
