---
created: 2026-04-19
updated: 2026-04-26
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
- Extended the embedded Riteed app to a v6 lightweight workspace with Open Folder, Close Folder, an adaptive project sidebar, lazy file-tree browsing, manual refresh, hidden-file toggling, and tab/tree synchronization.
- Extended the embedded Riteed app to a v7 compare workflow with in-tab split diffs, compare-with-disk, compare-with-file, compare-two-files, manual reference refresh, and F8/Shift+F8 diff navigation.
- Extended the embedded Riteed app to a v8 polish layer with editor palette selection, current-line highlight preferences, fullscreen support, F9 project-sidebar focus, and conservative autosave for already-saved writable files.
- Extended the embedded Riteed app to a v9 lightweight source-control workflow with a Git sidebar, project-tree file status badges, per-file Git compare, stage/unstage actions, and local commits.
- Added a sandbox-bundled `/app/bin/git` Flatpak module with source checksum and maintainer-signature verification, avoiding host Git and `flatpak-spawn`.
- Added Git identity preferences backed by GSettings and applied only on explicit user action for local commit support.
- Added a real New Window action that opens a fresh editor window without copying tabs, compare state, or project folders.
- Vendored the pinned `similar` crate for deterministic offline Flatpak-oriented compare/diff builds.

### Changed
- Simplified the repository layout so `AGENTS.md`, `policy/`, `tools/`, and `scripts/` live only at the root while `app/` validates directly against the root contract.
- Refactored the embedded Riteed window controller into smaller workspace, tab, close-flow, session, and I/O modules so the GNOME app remains policy-compliant and maintainable as features grow.
- Migrated the embedded Riteed editor core from `GtkTextView` to `GtkSourceView`, moving dirty-state tracking onto the buffer modified flag for better performance on longer text files.
- Made the embedded Riteed editor more code-friendly by auto-detecting languages for highlighting, keeping the minimap optional, and surfacing external file changes through in-tab banners and guarded save/reload flows.
- Extended the embedded Riteed editor to a v5a format-aware IO contract with `GtkSourceFileLoader/FileSaver`, deterministic line-ending state in the status bar, recoverable encoding-reopen flows, and guarded non-UTF-8 save/load handling.
- Extended the embedded Riteed editor to a v5b controls layer with staged indentation preferences, monospace-only editor font selection, window-scoped zoom controls, and updated status/menu/shortcuts surfaces.
- Extended the embedded Riteed editor to a v5c polish layer with direct status-bar zoom controls, document format controls in Preferences, fixed-size minimap rendering during zoom, and scroll-past-end editor padding.
- Added a folder-navigation split layout using `AdwOverlaySplitView` inside the existing toast overlay, keeping project state separate from document/tab state while preserving the lightweight editor workflow.
- Made compare highlighting follow the effective editor palette instead of the application theme, and kept autosave saves silent so they do not reorder recent files, persist session state, or show save toasts.
- Moved quick app appearance and editor palette controls into an icon-only header-bar Appearance panel with visual palette previews, while keeping editor view toggles such as current-line highlight in Preferences.
- Refined the editor chrome by putting Project Sidebar first in the header bar, moving Save beside Open, moving a friendly file location into the bottom status bar, and removing the status-bar Actual Size button.
- Simplified the primary menu into a lean app menu (New Window, Open..., Open Folder..., Recent Files..., Search, Compare..., Keyboard Shortcuts, Preferences, Help, About).
- Reworked Compare into a single-entry flow: Compare... opens a dedicated compare dialog where the user chooses sources (current document, saved version, file, or pasted text), with left as the editable side and right as the reference.
- Moved compare-session actions (refresh reference, exit compare, next/previous diff) out of the main menu and into the compare view/toolbar.
- Switched Recent Files to a lightweight dialog listing recent documents (most recent first) instead of nested menu flows.
- Split the left sidebar into Files and Source Control modes with a libadwaita view stack while preserving project-tree state.
- Reworked the Source Control changed-file list into compact single-line rows with row-activated Git compare, hover/focus Stage and Unstage icons, and consistent `U` badges for untracked files.
- Refactored compare controller plumbing into smaller modules so Git-backed compare could reuse the existing split-diff engine without growing the compare file past policy limits.
- Revised the runtime policy to allow only typed Gio subprocess Git operations in `src/git_process.rs`; `std::process::Command` and `flatpak-spawn` remain forbidden.

### Fixed
- Stabilized GTK coverage runs by teaching `tools.coverage_check` to invoke `cargo llvm-cov` with a deterministic `GSK_RENDERER=cairo` environment, with unit coverage for the new tool behavior.
- Stabilized headless GTK validation further by defaulting `GTK_A11Y=none` for policy and coverage test commands.
- Ignored embedded app build outputs such as `app/target/`, Flatpak cache directories, and app-local coverage directories so routine local validation does not keep leaving commit noise behind.
- Polished the embedded Riteed v2 tabbed editor so the editor fills the full window, the tab bar hides when only one document is open, and file drag-and-drop still opens documents in tabs instead of inserting links into the text view.
- Replaced external F1 help launching with an in-app help dialog and shortened recent-file menu labels so the primary menu stays more compact.
- Corrected the embedded Riteed v4 external-file monitor flow so atomic saves from other editors mark documents stale correctly, selected tabs show reload banners instead of silently reloading, and manual reload actions apply immediately.
- Restored the embedded Riteed minimap synchronization by removing the incorrect shared scroll-adjustment wiring that desynced the viewport preview.
- Restored the embedded Riteed development icon setup so About and the window icon can resolve the app icon during local `cargo run` workflows, and fixed clean window close requests so an untouched window no longer needs a second click to close.
- Switched the embedded Riteed About dialog to a dedicated full-color app icon alias so it no longer resolves to the symbolic icon variant.
- Fixed v5 format controls so Preferences can change the selected document encoding and save LF, CRLF, or CR line endings reliably, with GTK coverage for the actual UI path.
- Fixed v5 editor zoom so the minimap remains a narrow overview instead of scaling with the editor font, and kept zoom feedback visible as a direct percentage in the bottom status bar.
- Fixed gettext completeness sorting for mixed-context extraction results and expanded GTK coverage around folder restore, tree filtering, reveal, symlink, and Flatpak-local project navigation behavior.
- Added portal-aware fallback polling to Riteed document and project-tree monitors so external edits and folder changes still refresh when document-portal paths miss native monitor events.
- Ignored local Flatpak build output under `app/build-dir/` so installed test builds do not leave accidental commit noise.
- Added a Compare response to the external-reload prompt so users can inspect the current buffer against the changed on-disk file before choosing whether to reload.
- Corrected compare status text to count changed lines instead of hunks, and made Exit Compare clear diff highlights immediately.
- Added read-only and autosave-paused banners with explicit actions, guarded dirty external-change prompts while compare is active, and preserved the last non-fullscreen window size when closing from fullscreen.
- Corrected the policy checker so GSettings enum and flags keys are accepted as typed schema keys instead of being flagged as missing a free-form type.
- Fixed the new Appearance header-bar button so it opens a small libadwaita visual panel reliably instead of relying on fragile popover activation.
- Added a visible close button to the Appearance panel so it is not only dismissible with Escape.
- Fixed follow-up regressions in the menu/compare pass: editor zoom and font changes now style the GtkSourceView text node, compare mode restores normal editor scrolling, Recent Files uses a wider dialog, and Saved Version is suppressed while autosave is enabled.
- Displayed document-portal files with host/home-relative paths in the title/status, Recent Files, and Compare source UI while preserving portal access paths for sandbox-safe I/O.
- Removed invalid Appearance CSS size properties and added deterministic indentation coverage for tabs, spaces, indent width, and unindent behavior.
- Guarded Git stage/compare actions for unsupported repository states such as SHA-256 object format, configured EOL conversion, content filters, working-tree encodings, submodules, binary blobs, large blobs, dirty open tabs, and non-UTF-8 paths.
- Fixed Flatpak Git status refresh for document-portal project folders by running bundled Git from a stable sandbox cwd with explicit `GIT_DIR` and `GIT_WORK_TREE`.
- Trimmed the bundled Flatpak Git module to local plumbing only and explicitly stripped `/app/bin/git`, reducing the installed app size baseline to 7,617,536 bytes while preserving V9 status, stage, unstage, compare, and commit flows.

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
