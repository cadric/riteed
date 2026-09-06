from __future__ import annotations

from pathlib import Path
from typing import Any, TypeAlias

from tools.checks.foundation import release_policy, rust_policy, validation_policy
from tools.scanners.rust import RUST_GLOBS
from tools.validation_tooling import iter_files, match_any, normalize_path, relpath


CHECK_OWNERS = frozenset(
    {
        "cargo-workspace",
        "coverage",
        "dependency-preflight",
        "line-limits",
        "release-local-patches",
        "runtime-review",
        "source-patterns",
        "stress-fuzz",
    }
)
_SOURCE_SCOPE_KEYS = frozenset({"categories"})
_CATEGORY_KEYS = frozenset({"id", "paths", "checks"})
_PATCH_PREFIX = "build-aux/cargo-patches/"

InventoryEntry: TypeAlias = dict[str, tuple[str, ...]]
SourceInventory: TypeAlias = dict[str, InventoryEntry]


def check_source_scope(root: Path, errors: list[str]) -> SourceInventory:
    """Inventory every Rust source and record the configured validation owners."""
    policy = validation_policy(root)
    config = policy.get("source_scope")
    inventory = build_source_inventory(root, config, errors)
    if inventory:
        _validate_owner_scopes(root, inventory, policy, errors)
    return inventory


def build_source_inventory(root: Path, config: Any, errors: list[str]) -> SourceInventory:
    """Build a deterministic, fail-closed inventory from source_scope policy."""
    categories = _validated_categories(config, errors)
    if categories is None:
        return {}

    matches_by_category: dict[str, list[str]] = {
        category["id"]: [] for category in categories
    }
    rust_sources = sorted(
        relpath(path, root) for path in iter_files(root) if path.suffix == ".rs"
    )
    for rel in rust_sources:
        matches = [
            category
            for category in categories
            if match_any(rel, category["paths"])
        ]
        for category in matches:
            matches_by_category[category["id"]].append(rel)
        if not matches:
            errors.append(f"Unclassified Rust source: {rel}")
        elif len(matches) > 1:
            category_ids = ", ".join(sorted(category["id"] for category in matches))
            errors.append(
                f"Rust source matches multiple source_scope categories ({category_ids}): {rel}"
            )

    inventory: SourceInventory = {}
    for category in categories:
        category_id = category["id"]
        files = tuple(matches_by_category[category_id])
        if not files:
            errors.append(
                f"source_scope category {category_id!r} does not match any Rust source"
            )
        _validate_file_owners(category_id, files, category["checks"], errors)
        inventory[category_id] = {
            "checks": tuple(category["checks"]),
            "files": files,
        }
    return inventory


def _validated_categories(
    config: Any, errors: list[str]
) -> list[dict[str, Any]] | None:
    start_error_count = len(errors)
    if not isinstance(config, dict):
        errors.append("validation-tooling source_scope must be an object")
        return None
    unknown = sorted(set(config) - _SOURCE_SCOPE_KEYS)
    if unknown:
        errors.append(f"source_scope has unknown fields: {', '.join(unknown)}")
    raw_categories = config.get("categories")
    if not isinstance(raw_categories, list) or not raw_categories:
        errors.append("source_scope.categories must be a non-empty array")
        return None

    categories: list[dict[str, Any]] = []
    seen_ids: set[str] = set()
    for index, raw in enumerate(raw_categories):
        label = f"source_scope.categories[{index}]"
        if not isinstance(raw, dict):
            errors.append(f"{label} must be an object")
            continue
        unknown = sorted(set(raw) - _CATEGORY_KEYS)
        if unknown:
            errors.append(f"{label} has unknown fields: {', '.join(unknown)}")
        missing = sorted(_CATEGORY_KEYS - set(raw))
        if missing:
            errors.append(f"{label} missing required fields: {', '.join(missing)}")

        category_id = _nonempty_string(raw.get("id"), f"{label}.id", errors)
        paths = _string_list(raw.get("paths"), f"{label}.paths", errors)
        checks = _string_list(raw.get("checks"), f"{label}.checks", errors)
        if category_id is not None:
            if category_id in seen_ids:
                errors.append(f"{label}: duplicate category id {category_id!r}")
            seen_ids.add(category_id)
        if paths is not None:
            _validate_paths(paths, label, checks or [], errors)
        if checks is not None:
            for check in checks:
                if check not in CHECK_OWNERS:
                    errors.append(f"{label}.checks: unknown checker owner {check!r}")
        if category_id is not None and paths is not None and checks is not None:
            categories.append({"id": category_id, "paths": paths, "checks": checks})

    if len(errors) != start_error_count:
        return None
    return categories


def _nonempty_string(value: Any, label: str, errors: list[str]) -> str | None:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label} must be a non-empty string")
        return None
    return value.strip()


def _string_list(value: Any, label: str, errors: list[str]) -> list[str] | None:
    if not isinstance(value, list) or not value:
        errors.append(f"{label} must be a non-empty array")
        return None
    result: list[str] = []
    for index, item in enumerate(value):
        parsed = _nonempty_string(item, f"{label}[{index}]", errors)
        if parsed is not None:
            result.append(parsed)
    duplicates = sorted(item for item in set(result) if result.count(item) > 1)
    if duplicates:
        errors.append(f"{label} contains duplicate values: {', '.join(duplicates)}")
    return result


def _validate_paths(
    paths: list[str], label: str, checks: list[str], errors: list[str]
) -> None:
    for path in paths:
        normalized = normalize_path(path)
        parts = Path(normalized).parts
        if Path(normalized).is_absolute() or ".." in parts:
            errors.append(f"{label}.paths: path glob must be target-relative: {path!r}")
        if not normalized.endswith(".rs"):
            errors.append(f"{label}.paths: path glob must select Rust sources: {path!r}")
        if normalized.startswith(_PATCH_PREFIX):
            subtree = normalized[len(_PATCH_PREFIX) :].split("/", 1)[0]
            if not subtree or "*" in subtree or "?" in subtree:
                errors.append(
                    f"{label}.paths: local patch glob must name one exact patch subtree"
                )
    if "release-local-patches" in checks:
        for path in paths:
            normalized = normalize_path(path)
            if not normalized.startswith(_PATCH_PREFIX):
                errors.append(
                    f"{label}.paths: release-local-patches must select an exact patch subtree"
                )


def _validate_file_owners(
    category_id: str, files: tuple[str, ...], checks: list[str], errors: list[str]
) -> None:
    if not any(path.startswith(_PATCH_PREFIX) for path in files):
        return
    if "release-local-patches" not in checks:
        errors.append(
            f"source_scope category {category_id!r}: vendored local patches require "
            "release-local-patches ownership"
        )
    if "source-patterns" in checks:
        errors.append(
            f"source_scope category {category_id!r}: vendored local patches must not use "
            "source-patterns"
        )


def _validate_owner_scopes(
    root: Path,
    inventory: SourceInventory,
    policy: dict[str, Any],
    errors: list[str],
) -> None:
    source_rules = [
        item
        for key in ("forbidden_source_patterns", "required_source_patterns")
        for item in policy.get(key, [])
        if isinstance(item, dict)
    ]
    line_limit_globs = _strings(policy.get("line_limit_globs"))
    needs_runtime = any(
        "runtime-review" in entry["checks"] for entry in inventory.values()
    )
    runtime_rules: list[dict[str, Any]] = []
    if needs_runtime:
        enforcement = rust_policy(root).get("enforcement", {})
        if isinstance(enforcement, dict):
            runtime_rules = [
                item
                for key in ("hard_fail_patterns", "review_required_patterns")
                for item in enforcement.get(key, [])
                if isinstance(item, dict)
            ]
    needs_patches = any(
        "release-local-patches" in entry["checks"] for entry in inventory.values()
    )
    patch_roots = _release_patch_roots(root) if needs_patches else ()

    for category_id, entry in inventory.items():
        checks = entry["checks"]
        for rel in entry["files"]:
            if "source-patterns" in checks and not any(
                _rule_covers(rel, rule) for rule in source_rules
            ):
                errors.append(
                    f"{rel}: source_scope category {category_id!r} claims source-patterns "
                    "but no configured source-pattern rule covers it"
                )
            if "line-limits" in checks and not match_any(rel, line_limit_globs):
                errors.append(
                    f"{rel}: source_scope category {category_id!r} claims line-limits "
                    "but line_limit_globs does not cover it"
                )
            if "runtime-review" in checks and not (
                match_any(rel, RUST_GLOBS)
                or any(_rule_covers(rel, rule) for rule in runtime_rules)
            ):
                errors.append(
                    f"{rel}: source_scope category {category_id!r} claims runtime-review "
                    "but no runtime scanner scope covers it"
                )
            if "release-local-patches" in checks and not any(
                rel == patch_root or rel.startswith(f"{patch_root}/")
                for patch_root in patch_roots
            ):
                errors.append(
                    f"{rel}: source_scope category {category_id!r} claims "
                    "release-local-patches but release policy does not list its patch tree"
                )


def _strings(value: Any) -> list[str]:
    if not isinstance(value, list):
        return []
    return [normalize_path(item) for item in value if isinstance(item, str)]


def _rule_covers(rel: str, rule: dict[str, Any]) -> bool:
    paths = _strings(rule.get("paths"))
    exceptions = _strings(rule.get("exceptions"))
    return match_any(rel, paths) and not match_any(rel, exceptions)


def _release_patch_roots(root: Path) -> tuple[str, ...]:
    local_patch_policy = release_policy(root).get("local_patch_policy", {})
    if not isinstance(local_patch_policy, dict):
        return ()
    entries = local_patch_policy.get("release_critical_local_patches", [])
    if not isinstance(entries, list):
        return ()
    target = root.resolve()
    contract = root
    for candidate in (root, *root.parents):
        if (candidate / "policy").is_dir():
            contract = candidate
            break
    patch_roots: set[str] = set()
    for entry in entries:
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
            continue
        configured = normalize_path(entry["path"])
        for base in (contract, root):
            candidate = (base / configured).resolve()
            try:
                relative = candidate.relative_to(target).as_posix()
            except ValueError:
                continue
            if candidate.exists():
                patch_roots.add(normalize_path(relative))
                break
    return tuple(sorted(patch_roots))
