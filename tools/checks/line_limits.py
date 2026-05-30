from __future__ import annotations

from pathlib import Path
from typing import Any

from tools.validation_tooling import (
    contract_root,
    count_lines,
    iso_date_not_future_status,
    load_json,
    match_any,
    normalize_path,
    relpath,
    scoped_files,
)


VALID_SCOPES = {"app", "policy-pack"}


def check_line_limits(root: Path, errors: list[str], scope: str = "app") -> None:
    policy = _validation_policy(root)
    thresholds = policy["thresholds"]
    default_limit = int(thresholds["max_file_lines"])
    test_limit = int(thresholds["max_file_lines_test"])
    waiver_cap = int(thresholds["max_file_lines_waiver_cap"])
    line_globs = [str(item) for item in policy["line_limit_globs"]]
    test_globs = [str(item) for item in policy.get("test_file_globs", [])]
    files = scoped_files(root, line_globs)
    scoped_by_rel = {relpath(path, root): path for path in files}
    base_valid = _valid_base_waivers(policy, default_limit, waiver_cap, errors)
    if scope not in VALID_SCOPES:
        errors.append(f"line-limit scope {scope!r} is unsupported")
        return
    scoped_waivers = _scoped_waivers(
        root,
        scope,
        base_valid,
        scoped_by_rel,
        test_globs,
        default_limit,
        errors,
    )
    for path in files:
        rel = relpath(path, root)
        lines = count_lines(path)
        if match_any(rel, test_globs):
            limit = test_limit
            message = "test LOC limit"
        elif rel in scoped_waivers:
            limit = int(scoped_waivers[rel]["max_total_lines"])
            message = "waivered LOC limit"
        else:
            limit = default_limit
            message = "hard LOC limit"
        if lines > limit:
            errors.append(f"{rel} exceeds {message} {limit}: {lines}")


def _validation_policy(root: Path) -> dict[str, Any]:
    return load_json(contract_root(root) / "policy" / "validation-tooling.policy.json")


def _valid_base_waivers(
    policy: dict[str, Any],
    default_limit: int,
    waiver_cap: int,
    errors: list[str],
) -> list[dict[str, Any]]:
    waivers = policy.get("line_limit_waivers", [])
    if not isinstance(waivers, list):
        errors.append("line_limit_waivers must be an array")
        return []
    required = [str(item) for item in policy.get("line_limit_waiver_required_fields", [])]
    valid_entries: list[dict[str, Any]] = []
    for index, waiver in enumerate(waivers):
        label = f"line_limit_waivers[{index}]"
        if not isinstance(waiver, dict):
            errors.append(f"{label} must be an object")
            continue
        valid = True
        missing = [field for field in required if field not in waiver]
        if missing:
            errors.append(f"{label} missing required fields: {', '.join(missing)}")
            valid = False
        if not _valid_scope(waiver.get("scope"), label, errors):
            valid = False
        if not _valid_string(waiver.get("path"), label, "path", errors):
            valid = False
        elif not _safe_rel_path(str(waiver["path"])):
            errors.append(f"{label}: path must be relative and must not contain parent segments")
            valid = False
        for field in ("reason", "finding_id"):
            if not _valid_string(waiver.get(field), label, field, errors):
                valid = False
        max_total = waiver.get("max_total_lines")
        if isinstance(max_total, bool) or not isinstance(max_total, int):
            errors.append(f"{label}: max_total_lines must be an integer")
            valid = False
        elif max_total <= default_limit:
            errors.append(f"{label}: max_total_lines must exceed hard LOC limit {default_limit}")
            valid = False
        elif max_total > waiver_cap:
            errors.append(f"{label}: max_total_lines exceeds cap {waiver_cap}")
            valid = False
        status, today = iso_date_not_future_status(waiver.get("last_reviewed"))
        if status == "invalid":
            errors.append(f"{label}: last_reviewed must be YYYY-MM-DD")
            valid = False
        elif status == "future":
            errors.append(f"{label}: last_reviewed must not be after {today}")
            valid = False
        if valid:
            valid_entries.append(waiver)
    return valid_entries


def _scoped_waivers(
    root: Path,
    scope: str,
    waivers: list[dict[str, Any]],
    scoped_by_rel: dict[str, Path],
    test_globs: list[str],
    default_limit: int,
    errors: list[str],
) -> dict[str, dict[str, Any]]:
    active: dict[str, dict[str, Any]] = {}
    seen: set[str] = set()
    for waiver in waivers:
        if waiver.get("scope") != scope:
            continue
        rel = normalize_path(str(waiver["path"]))
        if rel in seen:
            errors.append(f"line_limit_waivers: duplicate path for scope {scope}: {rel}")
            continue
        seen.add(rel)
        path = scoped_by_rel.get(rel)
        if path is None:
            errors.append(f"{rel}: line-limit waiver path is outside scoped files for {scope}")
            continue
        if match_any(rel, test_globs):
            errors.append(f"{rel}: line-limit waiver is not allowed for test files")
            continue
        lines = count_lines(path)
        if lines <= default_limit:
            errors.append(f"{rel}: stale line-limit waiver for {lines} lines at hard LOC limit {default_limit}")
            continue
        active[rel] = waiver
    return active


def _valid_scope(value: Any, label: str, errors: list[str]) -> bool:
    if not isinstance(value, str) or value not in VALID_SCOPES:
        errors.append(f"{label}: scope must be one of app, policy-pack")
        return False
    return True


def _valid_string(value: Any, label: str, field: str, errors: list[str]) -> bool:
    if not isinstance(value, str) or not value.strip():
        errors.append(f"{label}: {field} must be a non-empty string")
        return False
    return True


def _safe_rel_path(value: str) -> bool:
    normalized = normalize_path(value)
    path = Path(normalized)
    return bool(path.parts) and not path.is_absolute() and ".." not in path.parts
