from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import sys
from typing import Any, Callable

from tools.checks import github_api, release_evidence


FetchPages = Callable[[str, str, list[str], str], list[dict[str, Any]] | None]
FetchJson = Callable[[str, str, list[str], str], Any | None]


def collect(
    *,
    repository: str,
    head_sha: str,
    policy: dict[str, Any],
    token: str,
    errors: list[str],
    fetch_pages: FetchPages = github_api.fetch_pages,
    fetch_json: FetchJson = github_api.fetch_json,
) -> dict[str, Any] | None:
    requirement = release_evidence.live_requirement(policy, errors)
    if requirement is None:
        return None
    if repository != requirement.repository:
        errors.append("Release evidence repository must match release policy exactly.")
        return None
    if re.fullmatch(r"[0-9a-f]{40}", head_sha) is None:
        errors.append("Release evidence requires an exact 40-character commit SHA.")
        return None
    app_slug = (
        policy.get("signed_flatpak_publish", {})
        .get("hard_requirements", {})
        .get("required_check_app_slug")
    )
    if not isinstance(app_slug, str) or not app_slug.strip():
        errors.append("Release evidence policy requires an exact check app slug.")
        return None
    check_pages = fetch_pages(
        github_api.api_url(
            repository,
            f"/commits/{head_sha}/check-runs",
            "per_page=100&filter=all",
        ),
        token,
        errors,
        "release check runs",
    )
    if check_pages is None:
        return None
    checks = release_evidence.paged_items(
        check_pages, "check_runs", "check-run", errors
    )
    if errors:
        return None
    latest = release_evidence.latest_live_check(
        checks, requirement, head_sha, app_slug
    )
    if latest is None:
        errors.append(f"Required live governance check {requirement.context} is missing.")
        return None
    location = release_evidence.details_location(
        latest.get("details_url"), requirement.repository
    )
    if location is None:
        errors.append("Newest live governance check has an invalid GitHub Actions details_url.")
        return None
    run_id, _job_id = location
    run = fetch_json(
        github_api.api_url(repository, f"/actions/runs/{run_id}"),
        token,
        errors,
        "live governance workflow run",
    )
    job_pages = fetch_pages(
        github_api.api_url(
            repository,
            f"/actions/runs/{run_id}/jobs",
            "per_page=100&filter=all",
        ),
        token,
        errors,
        "live governance workflow jobs",
    )
    if run is None or job_pages is None:
        return None
    evidence = {
        "check_runs": check_pages,
        "workflow_run": run,
        "workflow_jobs": job_pages,
    }
    errors.extend(
        release_evidence.check_live_governance(
            evidence,
            policy=policy,
            head_sha=head_sha,
            app_slug=app_slug,
        )
    )
    return evidence if not errors else None


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Collect provenance-bound GitHub release-check evidence"
    )
    parser.add_argument("--repository", required=True)
    parser.add_argument("--head-sha", required=True)
    parser.add_argument("--policy", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    errors: list[str] = []
    try:
        policy = json.loads(args.policy.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        print(f"Release evidence policy could not be loaded: {exc}", file=sys.stderr)
        return 2
    if not isinstance(policy, dict):
        print("Release evidence policy must be an object.", file=sys.stderr)
        return 2
    token = github_api.github_token()
    if not token:
        print("Release evidence collection requires GITHUB_TOKEN or GH_TOKEN.", file=sys.stderr)
        return 2
    evidence = collect(
        repository=args.repository,
        head_sha=args.head_sha,
        policy=policy,
        token=token,
        errors=errors,
    )
    if evidence is None:
        print("\n".join(errors), file=sys.stderr)
        return 1
    try:
        args.output.write_text(json.dumps(evidence, sort_keys=True) + "\n", encoding="utf-8")
    except OSError as exc:
        print(f"Release evidence output could not be written: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
