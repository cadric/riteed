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
- Added document-portal host-path display resolution so status/title surfaces, Recent Files, and Compare show user-facing paths while keeping portal paths for actual I/O.
- Added deterministic indentation behavior coverage for tab insertion, space indentation, indent width, and unindent behavior.
- Implemented V9 lightweight Git source control: Files/Source Control sidebar modes, project-tree status badges, typed `/app/bin/git` Gio subprocess operations, porcelain-v2 status parsing, Git-backed compare, stage/unstage, commit UI, and GSettings-backed Git identity.
- Added the Flatpak Git source module with Kernel.org checksum-autosigner verification so Riteed uses sandbox-bundled Git instead of host Git.
- Built and installed the local user Flatpak from the V9 files; current installed user commit is `aaf46011e751d64dfd7fd0cb448fef9bcc29693ae9a05c77bf4ba417001c6917`, and `/app/bin/git` reports `git version 2.54.0`.
- Fixed V9 Git refresh inside Flatpak document-portal project folders by running Git from `/` with explicit `GIT_DIR` and `GIT_WORK_TREE`, avoiding portal cwd failures.
- Trimmed the bundled Git Flatpak payload to local plumbing only and stripped `/app/bin/git`; the post-trim installed size baseline is 7,617,536 bytes with a +10% review ceiling of 8,379,290 bytes.
- Reworked V9 Source Control rows to a compact one-line list: row activation starts Git compare, hover/focus icons handle Stage/Unstage, and untracked files now use the shared `U` badge.
- Replaced the V9 Source Control flat changed-file list with a virtual Git path tree that preserves expanded folders and selected rows across refresh.
- Restored the resizable V9 sidebar layout so Files and Source Control share the left sidebar again, while drag resizing is clamped and no longer persists a fully hidden sidebar by accident.
- Final validation passed before commit preparation: `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app`, and `python3 -m unittest tools.tests.test_policy_check -v`.

## DECISIONS
- v6 split layout is sidebar/editor navigation via `AdwOverlaySplitView`, not an editor/editor split-pane feature.
- Project state remains window-global through GSettings because Riteed currently has a single-window app model.
- v7 compare mode is tab-local and ephemeral; it is not persisted in session/GSettings and exits on reload/open/restore.
- Reference snapshots update only on explicit Refresh Reference or save-to-disk compare refresh; no reference file monitor was added.
- v8 deliberately does not add recovery snapshots; autosave is limited to saved writable files with idle external state and never writes recents/session/GSettings or shows dialogs/toasts.
- Editor palettes are independent of the application theme; compare diff colors follow the effective editor palette dark/light classification.
- The quick appearance UI keeps deliberate app/editor light-dark mismatch support; only the `Match App Appearance` palette follows the app appearance.
- V9 source control deliberately uses a typed operation allowlist in `src/git_process.rs`; no generic Git runner is exposed, and host Git/`flatpak-spawn` remain forbidden.
- V9 stages raw editor/on-disk bytes only when Git filters, working-tree encoding, EOL attrs, and repo EOL conversion are absent; unsupported states stay visible with unsafe actions disabled.
- Source Control uses `U` rather than Git porcelain's `?` for untracked files across both the Source Control list and project tree badges.
- Source Control tree rows resolve actions by raw Git path against the current status snapshot, not by visible row index.
- Lightweight recent commit history and discard-file-changes were deferred from the first V9 delivery.
- `.agent/CONTINUITY.md` is local continuity state and is ignored by Git unless explicitly force-added.

## DISCOVERIES
- The initially launchable local Flatpak was stale; installing the fresh `app/build-dir` export fixed the missing project sidebar.
- `cond_move` was an unrelated untracked ELF artifact and was safe to remove during commit cleanup.
- The line-diff UI should distinguish changed-line count from hunk count; users read "differences" as changed lines in this surface.
- Compare highlights must be treated as transient compare state, not as post-compare document annotations.
- Document Portal `GetHostPaths` can map `/run/user/$UID/doc/...` mounts back to host paths for display, but Riteed must keep the portal path as the authoritative access path.
- Git can read document-portal project trees through `GIT_DIR`/`GIT_WORK_TREE`, but `git status` fails if the subprocess current directory itself is inside the portal mount.
- Local-plumbing Git packaging keeps `/app/bin/git` only, leaves `/app/libexec/git-core` present but empty, and disables/removes network and scripting helpers until a future Git feature re-justifies them.

## PROGRESS
- Source Control virtual tree refactor validation passed: `cargo test --workspace --all-targets --all-features -- --nocapture`, `python3 -m tools.policy_check --root app --strict`, and `python3 -m tools.coverage_check --root app` (80.3% line coverage).
- Local user Flatpak rebuild/install passed with commit `aaf46011e751d64dfd7fd0cb448fef9bcc29693ae9a05c77bf4ba417001c6917`; `flatpak info --user io.github.cadric.Riteed` reports 7.7 MB installed size and `/app/bin/git` reports `git version 2.54.0`.
- Sidebar layout fix validation passed: `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features`, `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app`, and `python3 -m unittest tools.tests.test_policy_check -v`.
- `CHANGELOG.md`, validation review artifacts, tests, and local Flatpak build are updated for the portal path display and indentation coverage pass.
- V9 validation passed: `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app` (80.0% line coverage), and `python3 -m unittest tools.tests.test_policy_check -v`.
- V9 Flatpak build passed: `flatpak-builder --user --install --force-clean app/build-dir app/build-aux/io.github.cadric.Riteed.yml`; smoke checked with `flatpak info --user io.github.cadric.Riteed`, `flatpak run --user --command=/app/bin/git io.github.cadric.Riteed --version`, local Git plumbing commands with an empty `GIT_TEMPLATE_DIR`, and a document-portal Git status command using `GIT_DIR`/`GIT_WORK_TREE`.
