# Policy Contract

This directory is the canonical contract for policy scope, validator behavior, and review-evidence structure.

## Scope Mapping

- `gnome-rust-app.bundle.json` is the entrypoint bundle.
- `validation-tooling.policy.json` owns shared thresholds, required tools, line limits, and review-artifact discovery.
- Domain policies own domain-specific hard-fail and `review_required` rules.
- `hard_fail_patterns[].exceptions` are narrow repo-relative globs applied before scanner regex matching; keep them path-scoped.
- `po/*.po` and `po/*.pot` are exempt only from generic line-count enforcement; gettext extraction, `msgfmt`, untranslated-catalog checks, and i18n review artifacts still apply.

## Review Evidence

The new `review_required` domains use machine-readable evidence only under `build-aux/validation/`.

- `ui-review*.json`
- `i18n-review*.json`
- `gsettings-review*.json`
- `runtime-review*.json`

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

Scanners must emit at most one hit per `(path, line, kind)`. If multiple reviewable patterns occur on one physical line, the scanner must refine `kind` so the identities stay unique.

## Evidence Boundaries

- `.agent/CONTINUITY.md` is continuity only and never counts as review evidence.
- `build-aux/permissions/flatpak-permissions.justifications.json` remains a separate Flatpak-permission contract and is not part of the new `review_required` artifact family.

## Maintainer Command

Use `python3 -m tools.policy_check --update-artifact-index` only in the root policy-pack repository. It is maintainer-only and must not be used in embedded app subtrees such as `app/` or in vendored target application repositories.

## Field Semantics

Review artifacts use fixed semantic tags where a field would otherwise be ambiguous:

- `ui.menus[].standard_items` is a list of lowercase semantic tags, not raw labels. Supported tags are `about`, `preferences`, `shortcuts`, `help`, `quit`, and `close`.
- `ui.surfaces[].smallest_width` is the reviewed narrowest supported window width in logical pixels.
- `gsettings.sites[].kind` must match the scanner kinds `gsettings-write` or `gsettings-bind`.
- GSettings schema keys satisfy the schema-type check with exactly one of `type`, `enum`, or `flags`, matching `glib-compile-schemas`.
- `runtime.sites[].kind` must match the scanner kinds emitted from policy, currently `runtime-strong-capture`, `runtime-shared-state`, and `runtime-git-subprocess`.

Template-source note:

- `.desktop.in.in` and `.metainfo.xml.in.in` are common GNOME/Meson source forms. Static validation in this pack only runs direct metadata validators on concrete `.desktop` and `.metainfo.xml` files present in the repository root passed to the checker. Template-only repositories still need build-system validation of generated outputs.
