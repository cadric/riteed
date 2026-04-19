# AGENTS — Riteed Target Repository Contract

This subtree is a vendored target application repository that consumes the root policy pack. The root `policy/`, `tools/`, and `scripts/policy-check` are authoritative; the copies under this subtree are synced from root and must not be edited manually.

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

## Application Contract
- App name: `Riteed`
- Application ID: `io.github.cadric.Riteed`
- Language: Rust `1.95.x`, edition `2024`, stable only
- UI stack: `gtk4-rs` + `libadwaita`
- Localization: `gettext-rs`
- Preferences and lightweight durable state: `GSettings`
- Packaging: Flatpak-first on `org.gnome.Platform//50`

## Required Runtime Constraints
- No runtime `unsafe`.
- No runtime `unwrap`, `expect`, `panic!`, `todo!`, `unimplemented!`, or `dbg!`.
- No runtime external command spawning.
- No enforced source or metadata file may exceed `600` total lines.
- All user-visible strings must be localizable.
- Preferences must use `GSettings`; no ad hoc config files.

## Identity Consistency
The application ID must stay consistent across:
- `adw::Application`
- Flatpak manifest `id`
- desktop file basename
- metainfo component ID
- GSettings schema ID or prefix
- gettext domain
- resource prefix
- icon basename

## Validation
Primary gate:
1. `python3 -m tools.policy_check --strict`

Coverage gate:
2. `python3 -m tools.coverage_check`

Direct checks:
- `cargo fmt --all --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets --all-features`
- `glib-compile-schemas --strict --dry-run data/schemas`
- `msgfmt --check-format --check-header -o /dev/null po/<catalog>.po`
- `desktop-file-validate data/io.github.cadric.Riteed.desktop`
- `appstreamcli validate --pedantic data/io.github.cadric.Riteed.metainfo.xml`
- `flatpak-builder --show-manifest <build-dir> build-aux/io.github.cadric.Riteed.yml`
