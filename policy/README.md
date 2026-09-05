# Policy Contract

This directory is the canonical contract for policy scope, validator behavior, and review-evidence structure.

## Scope Mapping

- `gnome-rust-app.bundle.json` is the entrypoint bundle.
- `validation-tooling.policy.json` owns shared thresholds, required tools, line limits, and review-artifact discovery.
- `release.policy.json` owns signed Flatpak publishing, GitHub Actions release gates, GitHub repository ruleset governance, GPG/OSTree beta remote signing, GitHub Pages artifact safety, rollback behavior, signing-key governance, and local release-critical patch manifests.
- `stress-fuzz.policy.json` owns parser-boundary registry requirements, fuzz seed fidelity, stress-script boundary fidelity, generated corpus/repo consumption, and stress/fuzz artifact preservation.
- Domain policies own domain-specific hard-fail and `review_required` rules.
- `hard_fail_patterns[].exceptions` are narrow repo-relative globs applied before scanner regex matching; keep them path-scoped.
- CSS review and resource scanning covers `data/**/*.css`, including CSS stored beside UI resources under `data/ui/`.
- `po/*.po` and `po/*.pot` are exempt only from generic line-count enforcement; gettext extraction, `msgfmt`, untranslated-catalog checks, and i18n review artifacts still apply.
- Generic line-count enforcement keeps production files at 600 lines by default, allows configured test files up to 800 lines, and permits production files up to the 720 waiver cap only through explicit `line_limit_waivers` with scope-relative paths and frozen per-file caps.

## Enforcement Boundaries

See [the enforcement map](../docs/policy-validation.md) for what each gate
executes and which architectural properties require human review. A green
policy-pack self-check establishes bundle integrity and scoped line limits;
CI runs validator unit tests separately.

- `source_scope.categories` assigns all discovered Rust sources to exactly
  one reviewed category with named checker owners; unknown files, overlapping
  categories and owner claims outside the actual scanner scopes fail.
- `cargo_source_inventory` owns the permitted generated archive/inline field
  sets and the exact parsed vendor configuration. Unknown sources, extra
  source options, orphaned checksums and non-lockfile archives fail.
- `rust.targets.rust.target_rust_family` and `rust.toolchain.required_components`
  drive toolchain checks directly; their values are not duplicated in Python.
- UI XML uses structural parsing. Runtime review uses lexical comment/literal
  masking and brace scopes; these checks do not perform Rust type inference,
  macro expansion or prove human ownership arguments.

## Review Evidence

The new `review_required` domains use machine-readable evidence only under `build-aux/validation/`.

- `ui-review*.json`
- `i18n-review*.json`
- `gsettings-review*.json`
- `runtime-review*.json`
- `parser-boundaries*.json`

These files are merged by sorted path order.

Every review entry is anchored to source with:

- `path`: repo-root-relative, normalized with forward slashes
- `line`: 1-based line number computed with `splitlines()`
- `match`: literal text that must still appear on that exact line
- `kind`: explicit kind unless provided by the artifact section contract

The validator hard-fails when:

- a scanner hit has no matching review entry
- a review entry matches no scanner hit
- multiple review entries claim the same `(path, line, kind)`
- the anchored line no longer exists
- the anchored line no longer contains `match`
- required fields violate `review_field_types` (including empty/whitespace
  evidence, boolean line numbers, and non-string justifications)
- `version` is not the integer `review_artifact_version`

Scanners must emit at most one hit per `(path, line, kind)`. If multiple reviewable patterns occur on one physical line, the scanner must refine `kind` so the identities stay unique.

## Evidence Boundaries

- `.agent/CONTINUITY.md` is continuity only and never counts as review evidence.
- `build-aux/permissions/flatpak-permissions.justifications.json` remains a separate Flatpak-permission contract and is not part of the new `review_required` artifact family.

## Maintainer Command

Use `python3 -m tools.policy_check --update-artifact-index` only in the root policy-pack repository. It is maintainer-only and must not be used in embedded app subtrees such as `app/` or in vendored target application repositories.

## Field Semantics

Review artifacts use fixed semantic tags where a field would otherwise be ambiguous:

- `ui.menus[].standard_items` is a list of lowercase semantic tags, not raw labels. Supported tags are `about`, `preferences`, `shortcuts`, `help`, `quit`, and `close`.
- AppStream gettext extraction covers app identity, description, and
  screenshot copy, but release descriptions must use `translate="no"` and stay
  out of POT/PO catalogs. `CHANGELOG.md` and GitHub Releases remain the full
  release-note sources; AppStream release text exists only for software-center
  version history.
- `ui.surfaces[].smallest_width` is the reviewed narrowest supported window width in logical pixels.
- `workflow_safety.local_flatpak_test_builds` defines the local preflight and
  wrapper scripts that must guard user-test Flatpak builds against split local
  feature branches. `allowed_build_branches` names normal build candidates,
  while `feature_only_override` is the explicit opt-in for partial feature-only
  builds.
- `gsettings.sites[].kind` must match the scanner kinds `gsettings-write` or `gsettings-bind`.
- GSettings schema keys satisfy the schema-type check with exactly one of `type`, `enum`, or `flags`, matching `glib-compile-schemas`.
- `runtime.sites[].kind` must match the scanner kinds emitted from policy, currently `runtime-strong-capture`, `runtime-shared-state`, `runtime-git-subprocess`, and `runtime-sync-fs`.
- `runtime-sync-fs` covers synchronous runtime filesystem probes in `src/**/*.rs`; test-only files and `#[cfg(test)]` ranges are ignored, and reviewed entries must explain the native-only guard.
- `parser_boundaries` entries use `{id, kind, source_paths, entrypoints, real_input_shape, coverage, gaps, reviewed_exceptions, last_reviewed}`. `reviewed_exceptions` means input shapes or trust-boundary cases intentionally not fuzzed or stressed with review evidence; it is not reviewer approval metadata.
- Parser-boundary implementation files must include `PARSER-BOUNDARY: id=<registry id>` markers in registered `source_paths`; the validator treats missing markers and unregistered markers as bidirectional registry drift.
- `planned_remediation` entries in release and stress/fuzz policies use `{finding_id, target_milestone, review_artifact, created, max_age_days, reason, removal_condition, approval_required?}`. `created` and reviewed exception dates must not be in the future; `created + max_age_days` is evaluated against the current UTC date at validator runtime, so stale policy debt expires even when no new commits are made.
- `release_identity.repository_full_name` is the `owner/name` repository used for read-only GitHub ruleset API verification after repository-governance remediation entries clear.
- `github_actions_release_safety.repository_governance.main_pull_request_policy` is the expected `Protect main` pull-request rule shape. The live governance job verifies that the branch ruleset requires pull requests, matches the reviewed approving-review count, requires review-thread resolution, and matches the reviewed last-push approval setting.
- `github_actions_release_safety.repository_governance.reviewed_bypass_actors` is the exact allowlist for GitHub ruleset bypass identities; live branch bypass actors must match `(ruleset, actor_type, actor_id, bypass_mode)`, `bypass_mode` must be `pull_request`, and tag rulesets must have no bypass actors.
- `signed_flatpak_publish.hard_requirements.required_validate_check_contexts`
  and `required_check_app_slug` are consumed by `tools.release_check_runs`
  before signing. The helper rejects incomplete pagination and requires the
  newest matching check-run ID to be completed with conclusion `success`.
- Required Validate jobs and gate steps cannot conditionally skip or continue
  on error. The governance step alone uses the exact policy-owned
  `repository_governance.validation_step_condition` for token-eligible events;
  conditional artifact uploads are allowed only for failure/cancellation.
- Every signing job must depend on the validated, unconditional success chain.
  The helper and governance run in the repository root with normal Bash;
  workflow/job/step shell overrides cannot turn execution into syntax checking.
  The workflow parser rejects folded YAML scalars; executable blocks use `|`.
- Offline release validation requires the dedicated check-runs invocation;
  inline status logic or presence of success-related words is not evidence.
- `workflow_dispatch` release publishes may target an explicit `release_ref`, but
  that ref must be a validated `v*` tag, the build job must checkout that tag,
  and legacy Pages metadata without source-ref/source-commit is accepted only
  when publishing a newer version than the current beta page.
- `github_actions_release_safety.rollback_environment.reviewed_required_reviewers` is the exact allowlist for the emergency rollback environment's required reviewer identities; the live governance job must match `(actor_type, actor_id)` and missing or extra reviewers fail validation.
- Release-critical local patch manifests pin an upstream `.crate` archive with its official checksum, list only reviewed changed files, store a canonical diff checksum, and record the unsafe/FFI baseline. The validator extracts the archive with tar-safety checks, and tracked `.crate` anchors must be marked as binary artifacts in `.gitattributes`.
- Each release-critical local patch listed in `release.policy.json` carries its own dependency document and unsafe/FFI baseline command; do not reuse another patched crate's baseline or review artifact.

Template-source note:

- `.desktop.in.in` and `.metainfo.xml.in.in` are common GNOME/Meson source forms. Static validation in this pack only runs direct metadata validators on concrete `.desktop` and `.metainfo.xml` files present in the repository root passed to the checker. Template-only repositories still need build-system validation of generated outputs.
