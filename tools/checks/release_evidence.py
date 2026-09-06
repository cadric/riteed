from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any


SHA_RE = re.compile(r"[0-9a-f]{40}")


@dataclass(frozen=True)
class LiveRequirement:
    repository: str
    context: str
    events: tuple[str, ...]
    workflow_name: str
    workflow_path: str
    job_name: str
    decisive_step: str
    main_branch: str


def check_live_governance(
    evidence: Any,
    *,
    policy: dict[str, Any],
    head_sha: str,
    app_slug: str,
) -> list[str]:
    errors: list[str] = []
    requirement = live_requirement(policy, errors)
    if requirement is None:
        return errors
    if not isinstance(head_sha, str) or SHA_RE.fullmatch(head_sha) is None:
        return ["Live governance requires an exact 40-character commit SHA."]
    if not isinstance(app_slug, str) or not app_slug.strip():
        return ["Live governance requires a non-empty app slug."]
    if not isinstance(evidence, dict):
        return ["Live governance evidence must be an object."]
    checks = paged_items(evidence.get("check_runs"), "check_runs", "check-run", errors)
    jobs = paged_items(evidence.get("workflow_jobs"), "jobs", "workflow job", errors)
    run = evidence.get("workflow_run")
    if errors:
        return errors
    if not isinstance(run, dict):
        return ["Live governance evidence requires one workflow_run object."]
    latest = latest_live_check(checks, requirement, head_sha, app_slug)
    if latest is None:
        return [f"Required live governance check {requirement.context} for {head_sha} is missing."]
    if latest.get("status") != "completed" or latest.get("conclusion") != "success":
        return [
            f"Newest live governance check {requirement.context} for {head_sha} is "
            f"{latest.get('conclusion') or latest.get('status') or 'unknown'}, not completed success."
        ]
    location = details_location(latest.get("details_url"), requirement.repository)
    if location is None:
        return ["Newest live governance check has an invalid GitHub Actions details_url."]
    run_id, job_id = location
    _check_run(run, requirement, run_id, head_sha, errors)
    _check_job(jobs, latest, requirement, run_id, job_id, head_sha, errors)
    return errors


def latest_live_check(
    checks: list[dict[str, Any]],
    requirement: LiveRequirement,
    head_sha: str,
    app_slug: str,
) -> dict[str, Any] | None:
    matching = [
        check
        for check in checks
        if check.get("name") == requirement.context
        and check.get("head_sha") == head_sha
        and isinstance(check.get("app"), dict)
        and check["app"].get("slug") == app_slug
    ]
    return max(matching, key=lambda item: int(item["id"])) if matching else None


def live_requirement(
    policy: dict[str, Any], errors: list[str]
) -> LiveRequirement | None:
    repository = _nonempty_string(
        policy.get("release_identity", {}).get("repository_full_name")
    )
    config = (
        policy.get("github_actions_release_safety", {})
        .get("repository_governance", {})
        .get("truthful_checks")
    )
    if not isinstance(config, dict):
        errors.append("Release policy requires repository_governance.truthful_checks.")
        return None
    strings = {
        key: _nonempty_string(config.get(key))
        for key in (
            "live_context",
            "live_workflow_name",
            "live_workflow_path",
            "live_job",
            "live_decisive_step",
            "main_branch",
        )
    }
    events = config.get("live_allowed_events")
    if (
        repository is None
        or re.fullmatch(r"[^/\s]+/[^/\s]+", repository) is None
        or any(value is None for value in strings.values())
        or not isinstance(events, list)
        or events != ["push", "schedule", "workflow_dispatch"]
    ):
        errors.append("Release policy live governance producer contract is incomplete.")
        return None
    return LiveRequirement(
        repository=repository,
        context=strings["live_context"],
        events=tuple(events),
        workflow_name=strings["live_workflow_name"],
        workflow_path=strings["live_workflow_path"],
        job_name=strings["live_job"],
        decisive_step=strings["live_decisive_step"],
        main_branch=strings["main_branch"],
    )


def paged_items(
    pages: Any,
    key: str,
    label: str,
    errors: list[str],
) -> list[dict[str, Any]]:
    if not isinstance(pages, list) or not pages:
        errors.append(f"{label} evidence requires non-empty paginated payloads.")
        return []
    expected: int | None = None
    values: list[dict[str, Any]] = []
    seen: set[int] = set()
    for page in pages:
        if not isinstance(page, dict):
            errors.append(f"{label} pages must be objects.")
            return []
        total = page.get("total_count")
        items = page.get(key)
        if type(total) is not int or total < 0 or not isinstance(items, list):
            errors.append(f"{label} pages require total_count and {key}.")
            return []
        if expected is not None and expected != total:
            errors.append(f"{label} total_count changed during pagination.")
            return []
        expected = total
        for item in items:
            item_id = item.get("id") if isinstance(item, dict) else None
            if (
                not isinstance(item, dict)
                or type(item_id) is not int
                or item_id <= 0
                or item_id in seen
            ):
                errors.append(f"{label} IDs must be positive unique integers.")
                return []
            seen.add(item_id)
            values.append(item)
    if expected is None or len(values) != expected:
        errors.append(f"Incomplete {label} pagination.")
        return []
    return values


def details_location(value: Any, repository: str) -> tuple[int, int] | None:
    pattern = re.compile(
        rf"https://github[.]com/{re.escape(repository)}/actions/runs/([1-9][0-9]*)/job/([1-9][0-9]*)"
    )
    match = pattern.fullmatch(value) if isinstance(value, str) else None
    return (int(match.group(1)), int(match.group(2))) if match else None


def _check_run(
    run: dict[str, Any],
    requirement: LiveRequirement,
    run_id: int,
    head_sha: str,
    errors: list[str],
) -> None:
    expected = {
        "id": run_id,
        "name": requirement.workflow_name,
        "path": requirement.workflow_path,
        "head_sha": head_sha,
        "head_branch": requirement.main_branch,
    }
    for field, value in expected.items():
        actual = run.get(field)
        if (field == "id" and type(actual) is not int) or actual != value:
            errors.append(f"Live governance workflow run {field} must be {value!r}.")
    if run.get("event") not in requirement.events:
        errors.append("Live governance workflow run event is not policy-owned.")
    if run.get("status") != "completed":
        errors.append("Live governance workflow run must be completed.")
    for field in ("repository", "head_repository"):
        owner = run.get(field)
        if not isinstance(owner, dict) or owner.get("full_name") != requirement.repository:
            errors.append(f"Live governance workflow run {field} must be the policy repository.")


def _check_job(
    jobs: list[dict[str, Any]],
    check: dict[str, Any],
    requirement: LiveRequirement,
    run_id: int,
    job_id: int,
    head_sha: str,
    errors: list[str],
) -> None:
    check_id = check["id"]
    api_url = f"https://api.github.com/repos/{requirement.repository}/check-runs/{check_id}"
    details_url = check.get("details_url")
    matching = [
        job
        for job in jobs
        if job.get("id") == job_id
        and job.get("check_run_url") == api_url
        and job.get("html_url") == details_url
    ]
    if len(matching) != 1:
        errors.append("Live governance check must map to one exact Actions job URL pair.")
        return
    job = matching[0]
    expected = {
        "run_id": run_id,
        "name": requirement.job_name,
        "head_sha": head_sha,
        "status": "completed",
        "conclusion": "success",
    }
    for field, value in expected.items():
        actual = job.get(field)
        if (field == "run_id" and type(actual) is not int) or actual != value:
            errors.append(f"Live governance Actions job {field} must be {value!r}.")
    steps = job.get("steps")
    if not isinstance(steps, list) or not all(isinstance(step, dict) for step in steps):
        errors.append("Live governance Actions job requires step payloads.")
        return
    decisive = [step for step in steps if step.get("name") == requirement.decisive_step]
    if len(decisive) != 1:
        errors.append("Live governance decisive step must be present exactly once.")
    elif decisive[0].get("status") != "completed" or decisive[0].get("conclusion") != "success":
        errors.append("Live governance decisive step must be completed success.")


def _nonempty_string(value: Any) -> str | None:
    return value.strip() if isinstance(value, str) and value.strip() else None
