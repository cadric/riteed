from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
import sys
from typing import Any


class CheckRunInputError(ValueError):
    pass


def check_required_runs(
    payloads: list[dict[str, Any]],
    *,
    required_checks: list[str],
    head_sha: str,
    app_slug: str,
) -> list[str]:
    problem = _input_error(payloads, required_checks, head_sha, app_slug)
    if problem:
        return [problem]
    errors: list[str] = []
    for name in required_checks:
        matching = [
            run
            for payload in payloads
            for run in payload.get("check_runs", [])
            if run.get("name") == name
            and run.get("head_sha") == head_sha
            and run.get("app", {}).get("slug") == app_slug
        ]
        if not matching:
            errors.append(f"Required Validate check {name} for {head_sha} is missing.")
            continue
        latest = max(matching, key=_run_id)
        if latest.get("status") != "completed":
            errors.append(
                f"Required Validate check {name} for {head_sha} is "
                f"{latest.get('status') or 'unknown'}, not completed successfully."
            )
            continue
        if latest.get("conclusion") != "success":
            errors.append(
                f"Required Validate check {name} for {head_sha} is "
                f"{latest.get('conclusion') or 'unknown'}, not success."
            )
    return errors


def _input_error(payloads: Any, required: Any, sha: Any, app_slug: Any) -> str | None:
    if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
        return "Release checks require an exact 40-character commit SHA."
    if (not isinstance(required, list) or not required
            or not all(isinstance(item, str) and item.strip() == item and item for item in required)
            or len(set(required)) != len(required)):
        return "Release checks require unique non-empty check contexts."
    if not isinstance(app_slug, str) or not app_slug.strip():
        return "Release checks require a non-empty app slug."
    if not isinstance(payloads, list) or not payloads:
        return "Release checks require complete paginated check-runs payloads."
    expected_count: int | None = None
    seen: set[int] = set()
    for page in payloads:
        if not isinstance(page, dict):
            return "Check-runs pages must be objects."
        total = page.get("total_count")
        runs = page.get("check_runs")
        if type(total) is not int or total < 0 or not isinstance(runs, list):
            return "Check-runs page must contain total_count and check_runs."
        if expected_count is not None and total != expected_count:
            return "Check-runs changed during pagination; rerun validation."
        expected_count = total
        for run in runs:
            if not isinstance(run, dict) or not isinstance(run.get("app"), dict):
                return "Malformed check-run object or app identity."
            run_id = run.get("id")
            if type(run_id) is not int or run_id <= 0 or run_id in seen:
                return "Check-run IDs must be positive, unique integers."
            seen.add(run_id)
    if len(seen) != expected_count:
        return "Incomplete check-runs pagination; rerun validation."
    return None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Require successful GitHub Actions checks for one commit")
    parser.add_argument("--input", required=True, type=Path, help="JSON check-runs payload or list of paginated payloads")
    parser.add_argument("--policy", required=True, type=Path, help="Release policy JSON")
    parser.add_argument("--head-sha", required=True, help="Exact release commit SHA")
    args = parser.parse_args(argv)
    try:
        payloads = _load_payloads(args.input)
        required, app_slug = _load_policy(args.policy)
    except (CheckRunInputError, OSError) as exc:
        print(str(exc), file=sys.stderr)
        return 2
    errors = check_required_runs(
        payloads,
        required_checks=required,
        head_sha=args.head_sha,
        app_slug=app_slug,
    )
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    return 0


def _load_payloads(path: Path) -> list[dict[str, Any]]:
    raw = _load_json(path)
    values = raw if isinstance(raw, list) else [raw]
    if not values or not all(isinstance(value, dict) for value in values):
        raise CheckRunInputError(f"{path}: expected a check-runs object or non-empty list of objects")
    for value in values:
        runs = value.get("check_runs")
        if not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs):
            raise CheckRunInputError(f"{path}: each payload must contain a check_runs object list")
    return values


def _load_policy(path: Path) -> tuple[list[str], str]:
    raw = _load_json(path)
    if not isinstance(raw, dict):
        raise CheckRunInputError(f"{path}: release policy must be an object")
    requirements = raw.get("signed_flatpak_publish", {}).get("hard_requirements", {})
    required = requirements.get("required_validate_check_contexts")
    app_slug = requirements.get("required_check_app_slug")
    if (
        not isinstance(required, list)
        or not required
        or not all(isinstance(item, str) and item.strip() for item in required)
        or len(set(required)) != len(required)
    ):
        raise CheckRunInputError(f"{path}: required_validate_check_contexts must be unique non-empty strings")
    if not isinstance(app_slug, str) or not app_slug.strip():
        raise CheckRunInputError(f"{path}: required_check_app_slug must be a non-empty string")
    return [item.strip() for item in required], app_slug.strip()


def _load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise CheckRunInputError(f"{path}: invalid JSON ({exc})") from exc


def _run_id(run: dict[str, Any]) -> int:
    value = run.get("id")
    return value if isinstance(value, int) and not isinstance(value, bool) else -1


if __name__ == "__main__":
    sys.exit(main())
