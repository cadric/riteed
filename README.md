# Riteed

Riteed is a native GNOME plain-text editor written in Rust. The application lives under `app/`.

This repository also keeps the authoritative policy and validation tooling at the root so the app can be validated without maintaining duplicate copies of `AGENTS.md`, `policy/`, `tools/`, or `scripts/`.

## Layout

- `app/` — the Riteed application source, resources, metadata, tests, and vendored Cargo dependencies
- `AGENTS.md` — repository-wide contract for app and policy work
- `policy/` — machine-readable policy files used to validate the app
- `policy/README.md` — scope mapping and review-artifact contract
- `tools/` — hard-fail validation tooling
- `scripts/` — thin wrappers around the root tooling
- `VERSIONS.md` — versioning rules for this repository
- `CHANGELOG.md` — notable repository changes

## Application stack

Riteed is intentionally narrow and GNOME-native:

- Rust
- GTK 4 bindings for Rust
- libadwaita
- GNOME HIG alignment
- gettext-based localization
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

## Notes

- The root policy files are the only authoritative contract copy in this repository.
- Review-required evidence for the app lives under `app/build-aux/validation/`; `.agent/CONTINUITY.md` is continuity only and never validator evidence.
- `app/scripts/dev-run` is the app-local helper that remains under `app/`.
