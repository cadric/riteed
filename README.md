# GNOME Rust App Policy Kit

This repository contains a strict, machine-readable policy and validation package for building native GNOME applications in Rust.

It is intended for use by coding agents and by humans who want a predictable baseline for generation, review, and CI enforcement.

## What is in this repository

- `AGENTS.md` — root contract for agent behavior
- `policy/` — machine-readable policy files
- `policy/README.md` — scope mapping and review-artifact contract
- `tools/` — hard-fail validation tooling
- `VERSIONS.md` — versioning rules for this repository
- `CHANGELOG.md` — notable repository changes

## Current intent

The current policy stack is aimed at a strict GNOME application workflow built around:

- Rust
- GTK 4 bindings for Rust
- libadwaita
- GNOME HIG alignment
- gettext-based localization
- GSettings-backed preferences
- Flatpak-first packaging and sandboxing

## Validation

Typical validation entrypoints for a target application repository that vendors this pack:

```bash
python3 -m tools.policy_check --root /path/to/app-repo --strict
python3 -m tools.coverage_check --root /path/to/app-repo
```

## Notes

- This repository is a policy/tooling kit, not the application itself. The validators target an application repository that contains the expected Cargo, src/, data/, po/, and build-aux/ layout.
- The policy files are intended to be consumed together, with the bundle manifest as the primary entrypoint.
- Review-required evidence lives under `build-aux/validation/`; `.agent/CONTINUITY.md` is continuity only and never validator evidence.
- You will likely want to tailor this README further once the target application and repository conventions are settled.
