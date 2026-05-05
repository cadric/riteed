---
created: 2026-04-19
updated: 2026-05-05
status: current
priority: high
type: changelog
---

# Changelog

All notable changes to this repository are documented in this file.

The format follows Keep a Changelog. Riteed is still pre-1.0; 0.x entries describe public beta snapshots.

## Unreleased

## 0.2.0 - 2026-05-05

### Added
- Initial machine-readable policy bundle for strict native GNOME applications written in Rust.
- Policy files for Rust, gtk4-rs, libadwaita, GNOME HIG, gettext/i18n, GSettings, Flatpak metadata, and validation tooling.
- Hard-fail validation tooling for policy enforcement, maximum file size enforcement, and minimum Rust coverage enforcement.
- Root `AGENTS.md` contract for LLM coding agents working against the policy bundle.
- Package-safe validator layout under `tools.checks` and `tools.scanners`, plus canonical `python3 -m tools.policy_check` and `python3 -m tools.coverage_check` entrypoints with direct-script compatibility.
- Deterministic review-artifact contract under `build-aux/validation/`, documented in `policy/README.md`, with shard discovery, line-anchored matching, and two-way coverage enforcement.
- Maintainer-only `--update-artifact-index` support for regenerating bundle policy hashes.
- Unit coverage in `tools/tests/test_policy_check.py` for entrypoints, site matching, and artifact-index upkeep.
- Embedded `app/` as the canonical Riteed application subtree, including the GTK4/libadwaita plain-text editor, Flatpak metadata, gettext catalogs, GSettings schema, review artifacts, and vendored Cargo dependency tree for offline Flatpak-oriented builds.
- Extended the embedded Riteed app to a tabbed v2 editor with multi-document tabs, recent files, session restore, unsaved-close coordination, and drag-and-drop file opening.
- Extended the embedded Riteed app to a v3 editor with in-document search and replace, optional line numbers, and a bottom status bar for file and cursor feedback.
- Extended the embedded Riteed app to a v4 editor with syntax highlighting, an optional minimap, and conservative external-file monitoring that protects unsaved work.
- Extended the embedded Riteed app to a v6 lightweight workspace with Open Folder, Close Folder, an adaptive project sidebar, lazy file-tree browsing, manual refresh, hidden-file toggling, and tab/tree synchronization.
- Extended the embedded Riteed app to a v7 compare workflow with in-tab split diffs, compare-with-disk, compare-with-file, compare-two-files, manual reference refresh, and F8/Shift+F8 diff navigation.
- Extended the embedded Riteed app to a v8 polish layer with editor palette selection, current-line highlight preferences, fullscreen support, F9 project-sidebar focus, and conservative autosave for already-saved writable files.
- Extended the embedded Riteed app to a v9 lightweight source-control workflow with a Git sidebar, project-tree file status badges, per-file Git compare, stage/unstage actions, and local commits.
- Extended the embedded Riteed app to a v10 source-control completion pass with tree/list view modes, recent commit history, per-file discard for safe unstaged tracked changes, and coalesced live Git refresh.
- Extended the embedded Riteed app to a v11 split-diff polish pass with logical row alignment, blank presentation placeholder rows, token-aware intra-line highlighting, hunk navigation, and the same renderer for manual Compare and Git compare.
- Added a sandbox-bundled `/app/bin/git` Flatpak module with source checksum and maintainer-signature verification, avoiding host Git and `flatpak-spawn`.
- Added Git identity preferences backed by GSettings and applied only on explicit user action for local commit support.
- Added a GSettings-backed Source Control view-mode preference and a packaged Source Control symbolic icon.
- Added a real New Window action that opens a fresh editor window without copying tabs, compare state, or project folders.
- Vendored the pinned `similar` crate for deterministic offline Flatpak-oriented compare/diff builds.

### Changed
- Tailored the repository contract and validation model away from the earlier Go-focused setup and toward a strict GNOME Rust application workflow.
- Promoted the remaining HIG, libadwaita, gettext, GSettings, and runtime blockers into deterministic validator rules using hard-fail checks plus `review_required` evidence.
- Added `xgettext` as a required validator tool and switched gettext completeness checks to normalized extractor output instead of heuristic-only scanning.
- Extended hard line-limit enforcement to `scripts/**` and `policy/README.md`.
- Simplified the repository layout so `AGENTS.md`, `policy/`, `tools/`, and `scripts/` live only at the root while `app/` validates directly against the root contract.
- Refactored the embedded Riteed window controller into smaller workspace, tab, close-flow, session, and I/O modules so the GNOME app remains policy-compliant and maintainable as features grow.
- Migrated the embedded Riteed editor core from `GtkTextView` to `GtkSourceView`, moving dirty-state tracking onto the buffer modified flag for better performance on longer text files.
- Made the embedded Riteed editor more code-friendly by auto-detecting languages for highlighting, keeping the minimap optional, and surfacing external file changes through in-tab banners and guarded save/reload flows.
- Moved document file-stamp checks onto async GIO queries, avoiding GTK-side metadata stalls on portal, FUSE, and network-backed files while preserving guarded reload/missing-file prompts.
- Extended the embedded Riteed editor to a v5a format-aware IO contract with `GtkSourceFileLoader/FileSaver`, deterministic line-ending state in the status bar, recoverable encoding-reopen flows, and guarded non-UTF-8 save/load handling.
- Extended the embedded Riteed editor to a v5b controls layer with staged indentation preferences, monospace-only editor font selection, window-scoped zoom controls, and updated status/menu/shortcuts surfaces.
- Extended the embedded Riteed editor to a v5c polish layer with direct status-bar zoom controls, document format controls in Preferences, fixed-size minimap rendering during zoom, and scroll-past-end editor padding.
- Added a folder-navigation split layout using `AdwOverlaySplitView` inside the existing toast overlay, keeping project state separate from document/tab state while preserving the lightweight editor workflow.
- Switched the project sidebar container from `AdwOverlaySplitView` to `GtkPaned`, keeping the Files/Source Control modes but making sidebar width adjustable through a drag handle.
- Made compare highlighting follow the effective editor palette instead of the application theme, and kept autosave saves silent so they do not reorder recent files, persist session state, or show save toasts.
- Moved quick app appearance and editor palette controls into an icon-only header-bar Appearance panel with visual palette previews, while keeping editor view toggles such as current-line highlight in Preferences.
- Refined the editor chrome by putting Project Sidebar first in the header bar, moving Save beside Open, moving a friendly file location into the bottom status bar, and removing the status-bar Actual Size button.
- Simplified the primary menu into a lean app menu (New Window, Open…, Open Folder…, Recent Files…, Search, Compare…, Keyboard Shortcuts, Preferences, Help, About).
- Moved text-file, folder, and recent-file opening into a header-bar Open split button, filtering the Open Folder chooser to folders where supported, removing duplicate open commands from the primary menu, and using the standard new-tab icon.
- Aligned keyboard shortcuts and shortcut labels with GNOME HIG conventions, including Ctrl+N for New Window, Ctrl+T for New Tab, Ctrl+G for find navigation, Ctrl+R for project refresh, and F9 for toggling the project sidebar.
- Added a native tab context menu for moving tabs backward/forward, moving a tab to a new window, closing other tabs, and closing the current tab, with per-window zoom styling preserved after tab transfer.
- Fixed the modified-tab indicator so dirty tabs use an available Adwaita symbolic icon instead of showing a missing-icon placeholder.
- Preserved extensionless filenames during Save As instead of appending `.txt`, so code-oriented names such as `Makefile`, `LICENSE`, and `.gitignore` stay unchanged.
- Refreshed the pinned Kernel.org `sha256sums.asc` checksum used by the bundled Git Flatpak module after verifying the new signed checksum file.
- Reworked Compare into a single-entry flow: Compare… opens a dedicated compare dialog where the user chooses sources (current document, saved version, file, or pasted text), with reference on the left and current content on the right.
- Moved Compare entry points into the tab context menu with file, saved-version, and pasted-text actions, hiding Saved Version when autosave makes it irrelevant.
- Polished the pasted-text Compare dialog with a standard header close button, a bottom-aligned primary Compare action, and an expanding text area.
- Hardened Source Control safety so unreadable Git attributes disable Git actions and commits instead of being treated as no attributes, while compare highlighting now uses neutral high-contrast colors.
- Moved compare-session actions (refresh reference, exit compare, next/previous diff) out of the main menu and into the compare view/toolbar.
- Switched Recent Files to a lightweight dialog listing recent documents (most recent first) instead of nested menu flows.
- Split the left sidebar into Files and Source Control modes with a libadwaita view stack while preserving project-tree state.
- Reworked the Source Control changed-file view into a virtual folder tree with compact file rows, click-to-compare activation, hover/focus Stage and Unstage icons, and consistent `U` badges for untracked files.
- Moved app theme selection into GNOME-style System/Light/Dark swatches at the top of the primary menu, while keeping Appearance focused on editor palettes.
- Replaced editor palette preview tiles with custom non-selectable code previews, compact preview-only swatches, tooltips, and a bundled Classic Dark palette alongside the renamed Classic Light palette.
- Added a family-based Window Palette to Appearance that derives per-window chrome colors from GtkSourceView schemes, while editor palettes remain exact scheme choices.
- Reworked Riteed's Window Palette chrome from per-window scoped surface CSS to one global libadwaita `:root` color-variable provider, so tabs, header bars, status bars, dialogs, and popovers share palette colors consistently.
- Moved Riteed's document tab strip into the libadwaita toolbar top-bar stack, keeping sidebar-friendly flat chrome while preserving tab search and transfer behavior.
- Moved Appearance palette controls from a separate primary-menu dialog into a dedicated page inside Preferences, leaving the primary menu focused on standard app actions.
- Simplified Appearance preferences into Style and Palette controls, keeping Auto as the default adaptive editor palette while deriving window chrome from the editor palette family and current app style.
- Added explicit standard symbolic icons to the Preferences Editor and Appearance pages so the page switcher no longer shows missing-icon placeholders.
- Grouped the primary menu into native `GMenu` sections so theme choices, workflow actions, and standard GNOME items render with clear separators.
- Reworked compare scroll synchronization to use diff anchors instead of normalized ratios, and applied active document language highlighting to reference buffers.
- Refactored compare controller plumbing into smaller modules so Git-backed compare could reuse the existing split-diff engine without growing the compare file past policy limits.
- Replaced compare's hunk-only `DiffPlan` with a shared `DiffRowModel` built from full `similar::TextDiff` ops, making row alignment, presentation buffers, custom gutters, intra-line tags, and hunk status one tab-local source of truth.
- Made Compare views read-only in V11; exit compare to edit. This removes V10's editable compare pane while preserving the original editor widget, undo stack, cursor, selection, and modified state for restore on exit.
- Standardized Compare panes to the usual diff convention: reference/old content on the left in red and current/working content on the right in green, with token-aware inline ranges strengthening those same side colors.
- Added subtle diagonal hatch backgrounds to Compare filler rows, marking the empty side of insertions and deletions without changing unchanged context lines.
- Clarified the Riteed settings model into scoped modules and converted theme and Source Control view mode preferences to GSettings enums.
- Reduced `EditorTab` pressure by moving document runtime, I/O, external-file, autosave, and compare state into focused internal owners while keeping the public tab workflow unchanged.
- Polished UI copy by using real ellipses in dialog labels, sentence-style Preferences subtitles, user-first Help pages, and defensive restored-window size clamping.
- Revised the runtime policy to allow only typed Gio subprocess Git operations in `src/git_process.rs`; `std::process::Command` and `flatpak-spawn` remain forbidden.

### Fixed
- Stabilized GitHub Actions app validation by avoiding real portal-backed file chooser launches in GTK smoke tests, running the Fedora validation container with the device access expected by GNOME/Flatpak CI, and preserving both stdout and stderr when validator commands fail.
- Switched the GitHub Actions Flatpak job to Flatpak's official builder action and GNOME 50 container, avoiding host apt-install hangs while preserving the beta Flatpak build artifact.
- Aligned policy IDs, bundle metadata, and artifact indexing with the shipped policy files.
- Corrected validator behavior for Flatpak manifest filenames, JSON source pinning checks, permission justifications, gettext bootstrap detection, conditional GSettings enforcement, and conditional gresource requirements.
- Tightened machine-readability by enforcing line limits on AGENTS/policy/tooling files, removing unused required tools, adding missing required tools, and replacing overbroad glob and regex matching.
- Clarified that validation commands target application repositories that vendor this pack, not the policy-pack repository itself.
- Corrected Flatpak and AppStream validation commands to supported official forms and restored `--no-net --pedantic` for AppStream validation.
- Restored tolerant crate-root lint regexes, fixed the broken `extern "C"` detector, tightened Flatpak manifest discovery, and narrowed required runtime-crate checks to the root application package.
- Moved coverage tooling back into machine-readable coverage policy instead of requiring `cargo-llvm-cov` during the primary policy gate.
- Realigned HIG primary-menu enforcement with the current GNOME HIG: allow up to 12 items, require `about`, and forbid `quit`/`close` in primary menus.
- Clarified bundle scope, repo-root detection, review-artifact semantics, and template-file coverage for `.desktop.in.in` and `.metainfo.xml.in.in` sources.
- Made startup resource registration and gettext initialization failures visible instead of silently discarding them.
- Blocked normal editor opens above 25 MiB, prevented very large files from being restored automatically in later sessions, and disabled in-document search before SourceView indexes very large buffers.
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
- Removed synchronous document-portal host-path lookups from display-path rendering by caching host paths and resolving active document names asynchronously.
- Removed invalid Appearance CSS size properties and added deterministic indentation coverage for tabs, spaces, indent width, and unindent behavior.
- Guarded Git stage/compare actions for unsupported repository states such as SHA-256 object format, configured EOL conversion, content filters, working-tree encodings, submodules, binary blobs, large blobs, dirty open tabs, and non-UTF-8 paths.
- Compare: split diff panes now keep syntax highlighting active for code while preserving diff colors.
- Compare changed lines now use full-row backgrounds while inline token highlights remain stronger.
- Compare filler runs now expose one localized placeholder marker for the empty side, while copy keeps generated markers out of the clipboard.
- Compare, Paste Text, Recent Files, and Encoding dialogs no longer leak state when opened and closed repeatedly.
- Source Control: Git compare now rejects binary blobs cleanly instead of partially rendering them.
- Source Control: refresh is significantly faster on large repositories by avoiding per-row filesystem metadata checks.
- Source Control: discard dialog now clearly states unstaged changes will be permanently lost.
- Window: stored window dimensions are now bounded by schema ranges in addition to runtime clamping.
- Fixed Flatpak Git status refresh for document-portal project folders by running bundled Git from a stable sandbox cwd with explicit `GIT_DIR` and `GIT_WORK_TREE`.
- Fixed Source Control Git execution and live refresh for worktrees and non-standard Git directories by resolving Git metadata paths instead of assuming `<worktree>/.git`.
- Trimmed the bundled Flatpak Git module to local plumbing only and explicitly stripped `/app/bin/git`, reducing the installed app size baseline to 7,617,536 bytes while preserving V9 status, stage, unstage, compare, and commit flows.
- Restored the resizable sidebar to the left side, kept Files and Source Control inside one switchable sidebar, and clamped drag resizing so the sidebar cannot be hidden permanently by the handle.
- Fixed V10 polish regressions: Source Control icon registration, first-open Appearance tile sizing, Recent Files bottom actions, animated sidebar show/hide, and status-bar segment dividers.
- Fixed V10 follow-up regressions where sidebar animation prevented full hiding, automatic Git refreshes repeatedly rebuilt Source Control and Files UI, and compare/Git diff panes drifted out of scroll sync.
- Fixed V11 compare follow-up regressions by reserving identical measured custom-gutter width for original line numbers, adding visual `-`/`+` gutter markers in a fixed column for reference/current changed rows, making compare copy selection-safe, using strict viewport-based hunk navigation, opening compares at the first changed display row, and removing the previous current-hunk accent overlay so diff colors stay red for reference/old content and green for current/working content.
- Compare: scroll between panes now stays in sync within the same row, eliminating the one-line drift introduced when row-based sync replaced pixel mirroring.
- Compare inline diff budget is now a hard total cap, preventing UI jank when many rows have small modifications.
- Compare recompute now reuses one line diff for both row alignment and presentation buffers, reducing duplicate work on large compares.
- Source Control compare now defers the compare layout switch after opening a file, avoiding a first-activation crash when starting from an empty window.
- Project sidebar reveal no longer polls every 20ms; reveal now coalesces work on model changes.
- Fixed a crash when opening a second file into a running Riteed instance.
- AppStream metadata: modernized developer info using the current `<developer>` element, added content rating, and removed deprecated `<developer_name>`.
- Internal: system gettext is now enforced through the Cargo dependency feature, so maintainer validation no longer depends on a manual environment override.
- Source Control: action buttons no longer reserve inline space; status badges color-coded by Git state.
- Fixed Window Palette chrome coverage so scoped scheme colors reach the sidebar, tab strip, libadwaita dialogs, primary menu, and card-like dialog content without global theme rebinding.

### Packaging
- Marked the first public beta as `0.2.0`, updated the README for the beta feature set, added GitHub Actions Flatpak build coverage, and documented bundled Git and Rust dependency license surfaces.
