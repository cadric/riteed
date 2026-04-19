# Versioning Policy

This repository uses Semantic Versioning: `MAJOR.MINOR.PATCH`.

## Scope
- Repository version
- Policy bundle version
- Individual policy semantics
- Validation tooling behavior
- AGENTS contract behavior
- Optional machine-readable schema or contract versions when this repository exposes them

## Meaning
- `MAJOR`: breaking policy semantics, incompatible validation behavior, or contract changes that require coordinated adoption
- `MINOR`: backward-compatible policy additions, new validators, or new optional contract fields
- `PATCH`: bug fixes, wording clarifications, test updates, README or changelog maintenance, and internal refactors that preserve intended behavior

## Rules
1. Bump `MAJOR` when a policy meaning changes in a way that could fail previously valid code or materially change expected agent behavior.
2. Bump `MINOR` when adding new checks that are backward compatible, or when adding new policy files or optional contract fields without changing existing semantics.
3. Bump `PATCH` for documentation, test, or implementation fixes that preserve intended policy behavior.
4. If a target application repository has its own runtime version files or release manifests, that repository remains the source of truth for app/runtime versions.
5. Version changes should be reflected in `CHANGELOG.md` whenever policy semantics, validation behavior, or the root agent contract changes materially.

## Notes
- This repository is a policy and validation kit for strict native GNOME applications written in Rust.
- Repository versioning is not a substitute for the target application's own release versioning.
- Version decisions should be explained in the change summary when policy semantics move materially.
