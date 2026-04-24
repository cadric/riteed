# Continuity

## OUTCOMES
- Implemented v6 lightweight folder navigation for Riteed: Open Folder, Close Folder, project sidebar, lazy file tree, hidden-file toggle, refresh, tab/tree reveal sync, symlink handling, and mixed file/folder `GApplication::open`.
- Added portal-aware fallback polling for document and project-tree monitors so Flatpak/document-portal paths detect external edits and structural folder changes even when native GIO monitor events are missed.
- Implemented v7 compare and advanced split workflows: compare-with-disk, compare-with-file, compare-two-files, in-tab split diff UI, manual reference refresh, F8/Shift+F8 hunk navigation, external-reload Compare response, and ephemeral compare state.
- Corrected compare polish found during Flatpak testing: status now reports changed lines while keeping hunk navigation, and Exit Compare clears compare highlights immediately.
- Implemented v8 polish and editing-safety work: editor palette selection, current-line highlight preference, fullscreen/F11 and Escape exit, F9 project-sidebar focus, headerbar/compare a11y labels, read-only/autosave banners, and conservative silent autosave for already-saved writable files.
- Added the V8 appearance panel polish: an icon-only header-bar Appearance button for app appearance, visual editor palette previews, and current-line highlight while keeping deeper editor options in Preferences.
- Replaced the fragile `GtkPopover` Appearance path with a small libadwaita visual panel presented directly from the header-bar button.
- Added an explicit visible close button to the Appearance panel because Escape-only dismissal was not discoverable enough.
- Built and installed the local user Flatpak for testing; current installed user commit is `1e8216f1e52bab562067ee1b624a88e9b267514895038721f411d3fe55efcabb`.
- Final validation passed before commit preparation: `python3 -m tools.policy_check --root app --strict` and `python3 -m tools.coverage_check --root app`.

## DECISIONS
- v6 split layout is sidebar/editor navigation via `AdwOverlaySplitView`, not an editor/editor split-pane feature.
- Project state remains window-global through GSettings because Riteed currently has a single-window app model.
- v7 compare mode is tab-local and ephemeral; it is not persisted in session/GSettings and exits on reload/open/restore.
- Reference snapshots update only on explicit Refresh Reference or save-to-disk compare refresh; no reference file monitor was added.
- v8 deliberately does not add recovery snapshots; autosave is limited to saved writable files with idle external state and never writes recents/session/GSettings or shows dialogs/toasts.
- Editor palettes are independent of the application theme; compare diff colors follow the effective editor palette dark/light classification.
- The quick appearance UI keeps deliberate app/editor light-dark mismatch support; only the `Match App Appearance` palette follows the app appearance.
- `.agent/CONTINUITY.md` is local continuity state and is ignored by Git unless explicitly force-added.

## DISCOVERIES
- The initially launchable local Flatpak was stale; installing the fresh `app/build-dir` export fixed the missing project sidebar.
- `cond_move` was an unrelated untracked ELF artifact and was safe to remove during commit cleanup.
- The line-diff UI should distinguish changed-line count from hunk count; users read "differences" as changed lines in this surface.
- Compare highlights must be treated as transient compare state, not as post-compare document annotations.

## PROGRESS
- `CHANGELOG.md`, gettext POTs, validation review artifacts, tests, and local Flatpak build are updated for the V8 appearance panel refresh.
- Latest validation passed: `python3 -m tools.policy_check --root app --strict` and `python3 -m tools.coverage_check --root app`.
