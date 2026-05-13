# Riteed

Riteed is a small native GNOME text editor written in Rust. The current source
version is `0.3.1`, an early public beta. It is useful for daily local editing
and compare work, but it is not feature complete and has only been tested by the
primary maintainer so far.

The application lives under `app/`. The repository root also contains the
policy and validation tooling used to keep the app strict, native, and
Flatpak-first.

## Status

- Early beta: expect rough edges and missing features.
- First public source push: no stable release has been published.
- Target platform: GNOME on Linux, packaged through Flatpak.
- Current tester base: the maintainer only.

## Features

- Tabbed editing for local text, code, config, and markdown files.
- GtkSourceView syntax highlighting with editor palette selection.
- Find and replace, optional line numbers, optional minimap, and zoom controls.
- Encoding-aware open/save behavior with line-ending controls.
- Session restore, recent files, guarded reload prompts, and large-file safety
  limits.
- Optional autosave for already-saved writable files.
- Lightweight folder sidebar with lazy file browsing, hidden-file toggle,
  refresh, and tab/tree reveal.
- Split compare workflows for saved versions, files, pasted text, and Git
  changes.
- Polished split diffs with syntax highlighting, original-line gutters,
  inline token highlights, full-row changed backgrounds, and filler hatching.
- Local-only Source Control sidebar with Git status, file badges,
  stage/unstage, safe discard, Git compare, recent commit history, and commits.
- Bundled sandbox-local Git for Flatpak builds; Riteed does not call host Git.

## What Riteed Is Not Yet

- Not a full IDE.
- No push, pull, branch management, merge editor, terminal, debugger, or LSP.
- No external beta program yet.
- No stable API or release promise before `1.0`.

## Layout

- `app/` - Riteed application source, resources, metadata, tests, and Flatpak
  Cargo source manifest.
- `AGENTS.md` - repository-wide contract for app and policy work.
- `policy/` - machine-readable policy files used to validate the app.
- `tools/` - hard-fail validation tooling.
- `scripts/` - thin wrappers around the root tooling.
- `VERSIONS.md` - versioning rules for this repository.
- `CHANGELOG.md` - notable repository changes.
- `THIRD_PARTY_LICENSES.md` - license notes for vendored and bundled
  third-party components.

## Application Stack

Riteed is intentionally narrow and GNOME-native:

- Rust 1.95
- GTK 4 bindings for Rust
- libadwaita
- GtkSourceView
- GNU gettext localization
- GSettings-backed preferences
- Flatpak-first packaging and sandboxing

## Validate Riteed

Run validation from the repository root and point the root tooling at `app/`:

```bash
python3 -m tools.policy_check --root app --strict
python3 -m tools.coverage_check --root app
```

Direct app checks can still be run from `app/`:

```bash
cd app
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Build the local Flatpak from the repository root:

```bash
flatpak-builder --user --install --force-clean app/build-dir app/build-aux/io.github.cadric.Riteed.yml
flatpak run io.github.cadric.Riteed
```

GitHub Actions validates the root tooling, the app subtree, and a full Flatpak
build.

## License

Riteed's own source code and repository policy/tooling are licensed under the
MIT License; see `LICENSE`.

Cargo dependencies are pinned in `app/Cargo.lock`; the Flatpak build downloads
the locked crate archives listed in `app/build-aux/cargo/cargo-sources.json`
into a build-local `cargo/vendor/` tree. Local `app/vendor/` directories are
ignored and must not be committed.

The Flatpak manifest also bundles a trimmed local-plumbing Git binary for the
Source Control sidebar. Git is distributed under GPL-2.0-only overall and
contains some files under LGPL-2.1-or-later, BSD-3-Clause, and MIT-compatible
terms. The Flatpak build installs the relevant Git license texts under
`/app/share/licenses/io.github.cadric.Riteed/`.

See `THIRD_PARTY_LICENSES.md` for the current license review.
