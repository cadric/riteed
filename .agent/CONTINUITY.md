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
- Implemented V10 source-control completion and UX regression fixes: tree/list view mode, recent commit history, safe tracked-file discard, live Git refresh, Source Control icon packaging, Appearance menu action, Recent Files bottom action layout, status separators, sidebar animation, and anchor-based compare scroll sync with reference syntax highlighting.
- Moved app theme selection into the primary menu as stateful GNOME-style System/Light/Dark swatches and narrowed the Appearance dialog to editor palette selection.
- Fixed V10 follow-up regressions: sidebar animations now reach fully hidden/restored positions, automatic Git refreshes skip no-op Source Control/Files rebuilds and history refetches, and compare/Git diff panes scroll in sync again.
- Reduced `EditorTab` pressure by keeping one shared tab state cell but splitting its contents into document runtime, I/O, external-file, autosave, compare, and UI owner structs.
- Final validation passed before commit preparation: `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app`, and `python3 -m unittest tools.tests.test_policy_check -v`.

## DECISIONS
- v6 split layout is sidebar/editor navigation via `AdwOverlaySplitView`, not an editor/editor split-pane feature.
- Project state remains window-global through GSettings because Riteed currently has a single-window app model.
- v7 compare mode is tab-local and ephemeral; it is not persisted in session/GSettings and exits on reload/open/restore.
- Reference snapshots update only on explicit Refresh Reference or save-to-disk compare refresh; no reference file monitor was added.
- v8 deliberately does not add recovery snapshots; autosave is limited to saved writable files with idle external state and never writes recents/session/GSettings or shows dialogs/toasts.
- Editor palettes are independent of the application theme; compare diff colors follow the effective editor palette dark/light classification.
- App theme selection lives in the primary menu; editor palettes stay independent, and only the `Match App Appearance` palette follows the app appearance.
- V9 source control deliberately uses a typed operation allowlist in `src/git_process.rs`; no generic Git runner is exposed, and host Git/`flatpak-spawn` remain forbidden.
- V9 stages raw editor/on-disk bytes only when Git filters, working-tree encoding, EOL attrs, and repo EOL conversion are absent; unsupported states stay visible with unsafe actions disabled.
- Source Control uses `U` rather than Git porcelain's `?` for untracked files across both the Source Control list and project tree badges.
- Source Control tree rows resolve actions by raw Git path against the current status snapshot, not by visible row index.
- Theme and Source Control view mode now use GSettings enum keys while preserving the existing `theme` and `source-control-view-mode` key names for valid stored values.
- V10 keeps Source Control local-only: no push, pull, branch UI, remotes, full log browser, merge conflict editor, Markdown preview, or new language catalogs.
- `.agent/CONTINUITY.md` is local continuity state and is ignored by Git unless explicitly force-added.

## DISCOVERIES
- The initially launchable local Flatpak was stale; installing the fresh `app/build-dir` export fixed the missing project sidebar.
- `cond_move` was an unrelated untracked ELF artifact and was safe to remove during commit cleanup.
- The line-diff UI should distinguish changed-line count from hunk count; users read "differences" as changed lines in this surface.
- Compare highlights must be treated as transient compare state, not as post-compare document annotations.
- Document Portal `GetHostPaths` can map `/run/user/$UID/doc/...` mounts back to host paths for display, but Riteed must keep the portal path as the authoritative access path.
- Git can read document-portal project trees through `GIT_DIR`/`GIT_WORK_TREE`, but `git status` fails if the subprocess current directory itself is inside the portal mount.
- Local-plumbing Git packaging keeps `/app/bin/git` only, leaves `/app/libexec/git-core` present but empty, and disables/removes network and scripting helpers until a future Git feature re-justifies them.
- UI copy now uses real ellipses at runtime while keeping Rust gettext msgids ASCII-safe by appending ellipses in code where needed; Help separates user guidance from technical Source Control notes.
- Document-portal host-path display lookup is now cache-first and async; display names may update after open/save/move, while portal access paths remain authoritative for I/O.

## PROGRESS
- Source Control virtual tree refactor validation passed: `cargo test --workspace --all-targets --all-features -- --nocapture`, `python3 -m tools.policy_check --root app --strict`, and `python3 -m tools.coverage_check --root app` (80.3% line coverage).
- Local user Flatpak rebuild/install passed with commit `aaf46011e751d64dfd7fd0cb448fef9bcc29693ae9a05c77bf4ba417001c6917`; `flatpak info --user io.github.cadric.Riteed` reports 7.7 MB installed size and `/app/bin/git` reports `git version 2.54.0`.
- Sidebar layout fix validation passed: `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features`, `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app`, and `python3 -m unittest tools.tests.test_policy_check -v`.
- `CHANGELOG.md`, validation review artifacts, tests, and local Flatpak build are updated for the portal path display and indentation coverage pass.
- V9 validation passed: `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app` (80.0% line coverage), and `python3 -m unittest tools.tests.test_policy_check -v`.
- V9 Flatpak build passed: `flatpak-builder --user --install --force-clean app/build-dir app/build-aux/io.github.cadric.Riteed.yml`; smoke checked with `flatpak info --user io.github.cadric.Riteed`, `flatpak run --user --command=/app/bin/git io.github.cadric.Riteed --version`, local Git plumbing commands with an empty `GIT_TEMPLATE_DIR`, and a document-portal Git status command using `GIT_DIR`/`GIT_WORK_TREE`.
- V10 validation passed: `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app` (80.1% line coverage), and focused `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features`.
- V10 local user Flatpak rebuild/install passed with app commit `2ede5d441cffb4db3114bbaf5a11e60d27c01a582c7d26386a7b0639c8c80085`; `flatpak info --user io.github.cadric.Riteed` reports 7.8 MB installed size and `/app/bin/git` reports `git version 2.54.0`.
- V10 regression follow-up validation passed: `python3 -m tools.policy_check --root app --strict`, `python3 -m tools.coverage_check --root app` (80.5% line coverage), `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features`, `glib-compile-schemas --strict --dry-run app/data/schemas`, and `msgfmt --check-format --check-header -o /dev/null app/po/da.po`.
- V10 regression follow-up local user Flatpak rebuild/install passed with app commit `dee6a3bbe83dee41c214e9a2d15d2e9cd2d51147211879bb3234e17c1e8df274`; `flatpak info --user io.github.cadric.Riteed` reports 7.8 MB installed size and `/app/bin/git` reports `git version 2.54.0`.
- Menu/shortcut/tab workflow validation passed: `cargo fmt --all --check`, `cargo check --workspace --all-targets --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features -- --nocapture`, `python3 -m tools.policy_check --root app --strict`, and `python3 -m tools.coverage_check --root app` (81.0% line coverage).
- Local user Flatpak rebuild/install passed with app commit `d4c2614239074d8008d80b56bfe9039c10741ee5a7541c912e106c2714630bc5`; tab transfer follow-up scopes editor zoom CSS per window so Move to New Window no longer lets zoom changes in the destination window affect the source window.
- Async document-portal display-path validation passed: `cargo fmt --all --check`, `cargo check --workspace --all-targets --all-features`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features -- --nocapture`, `python3 -m tools.policy_check --root app --strict`, and `python3 -m tools.coverage_check --root app` (81.3% line coverage). Local user Flatpak rebuild/install passed with app commit `988f85866de0b3551bed749c0dd361b746ed52bdaac1039d25844d882ff3ee61`, installed size 7.9 MB, and `/app/bin/git` reports `git version 2.54.0`.
- Primary menu theme selector validation passed: `python3 -m unittest tools.tests.test_policy_check -v`, `msgfmt --check-format --check-header -o /dev/null app/po/da.po`, `GTK_A11Y=none GSK_RENDERER=cairo cargo test --workspace --all-targets --all-features`, `python3 -m tools.policy_check --root app --strict`, and `python3 -m tools.coverage_check --root app` (81.2% line coverage).
- Primary menu theme selector local user Flatpak rebuild/install passed with app commit `c9d6f76b68563404067879153fdd9ff457b5c5f3cf61c6f2c122e10d4373a4c7`, installed size 7.9 MB, and `/app/bin/git` reports `git version 2.54.0`.
- Theme selector CSS follow-up now mirrors GNOME Text Editor's 44px swatches, hidden unchecked radio indicator, inset selection ring, and centered expanded menu layout. Local user Flatpak rebuild/install passed with app commit `c62755d7f6599eec45be568101739f14bfcb1e9fb4f462cfddd32fa61e5b1ea7`, installed size 7.9 MB, and `/app/bin/git` reports `git version 2.54.0`.
