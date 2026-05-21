# Cargo-Fuzz Workspace Spike

Date: 2026-05-21

## Question

Can Riteed add cargo-fuzz targets without poisoning the stable PR gates that run
from `app/` with:

- `cargo check --workspace --all-targets --all-features`
- `python3 -m tools.coverage_check --root app`

## Current App Workspace Evidence

`cargo metadata --no-deps --format-version 1` from `app/` reports:

- `workspace_root`: `app`
- `workspace_members`: only `riteed@0.3.2`
- `workspace_default_members`: only `riteed@0.3.2`

Cargo documentation says `--workspace` checks all members in the current
workspace. It does not discover arbitrary child packages unless they are
members of that workspace.

## Configuration Options

### Option A: `app/fuzz/` as a member of the app workspace

Rejected. If `app/fuzz/` is added as a workspace member, `--workspace`
selects it. `default-members = ["app"]` is not sufficient because the stable
gate explicitly passes `--workspace`.

### Option B: `app/fuzz/` as an independent nested workspace

Recommended if cargo-fuzz is implemented.

Shape:

```toml
# app/fuzz/Cargo.toml
[workspace]
members = ["."]

[package]
name = "riteed-fuzz"
publish = false
edition = "2024"
```

Then `app/fuzz/rust-toolchain.toml` can pin nightly for cargo-fuzz while
`app/` remains on the stable toolchain selected by the app root. Rustup
toolchain overrides are directory scoped, so commands run from `app/` do not
inherit a descendant `app/fuzz/rust-toolchain.toml`.

## Flatpak Runtime

Cargo-fuzz should be host-only. The Flatpak release manifest builds the app
with stable Rust and `cargo --offline --locked build --release`; it should not
build libFuzzer targets or depend on nightly. Keep fuzz targets outside the app
workspace and outside the Flatpak module.

## Recommendation

Proceed only with Option B:

- `app/fuzz/` owns its own `[workspace]`.
- `app/fuzz/rust-toolchain.toml` pins nightly.
- Stable PR gates continue to run from `app/` and should not see fuzz targets.
- Fuzz jobs run explicitly with `cd app/fuzz && cargo +nightly fuzz run ...`.

Before adding the implementation, prove the exclusion with:

- `cd app && cargo metadata --no-deps --format-version 1`
- `cd app && cargo check --workspace --all-targets --all-features`
- `python3 -m tools.coverage_check --root app`
- `cd app/fuzz && cargo +nightly fuzz run markdown_parse -- -max_total_time=60`

## Decision Gate

The stress-test plan requires user approval before cargo-fuzz implementation.
This spike recommends the independent nested workspace configuration and stops
before implementation.

## Implementation Follow-Up

User approval was granted after this spike. The implementation now uses the
recommended independent nested workspace:

- `app/fuzz/Cargo.toml` owns its own `[workspace]`.
- `app/fuzz/rust-toolchain.toml` selects nightly only under `app/fuzz/`.
- `app/fuzz` depends on `riteed` through `path = ".."` with the non-default
  `fuzzing` feature.
- The app workspace metadata still reports only `riteed@0.3.2` as a member
  when run from `app/`.
- The fuzz workspace carries the same local `sourceview5` patch as the app
  workspace so fuzz builds exercise the same dependency source.

Known maintenance burden:

- Fuzz targets need the app crate's non-default `fuzzing` feature to expose
  narrow pure-Rust harness entry points.
- The `sourceview5` patch is duplicated in `app/fuzz/Cargo.toml`; future app
  patch moves must be mirrored there.
- `app/fuzz/Cargo.lock` is independent. It must be checked after app dependency
  updates; one local run already had to align `gtk4-sys` back to the app
  workspace's `0.11.2` after the fuzz workspace resolved `0.11.3`.
