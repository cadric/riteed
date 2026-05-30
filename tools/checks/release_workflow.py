from __future__ import annotations

import re
from typing import Any

from tools.checks import foundation
from tools.checks._workflow_parser import Workflow, WorkflowParseError, parse_workflow


WORKFLOW = ".github/workflows/publish-flatpak.yml"
VALIDATE_WORKFLOW = ".github/workflows/validate.yml"
POLICY_FILE = "policy/release.policy.json"
SIGNING_SECRETS = ("FLATPAK_GPG_PRIVATE_KEY", "FLATPAK_GPG_PASSPHRASE", "FLATPAK_GPG_KEY_ID")


def parse(label: str, workflow: str, errors: list[str]) -> Workflow | None:
    try:
        return parse_workflow(workflow, label)
    except WorkflowParseError as exc:
        foundation.add(errors, str(exc))
        return None


def check_publish_triggers(workflow: Workflow | None, errors: list[str]) -> None:
    if workflow is None:
        return
    if "workflow_dispatch" not in workflow.triggers:
        foundation.add(errors, f"{WORKFLOW}: workflow_dispatch trigger is required for reviewed manual release flow")
    tags = workflow.triggers.get("push", {}).get("tags", [])
    if "v*" not in [str(tag) for tag in tags if str(tag)]:
        foundation.add(errors, f"{WORKFLOW}: release tag trigger must include v*")
    if not _any_run_contains(workflow, "GITHUB_REF", "refs/tags/v*", "exit 1"):
        foundation.add(errors, f"{WORKFLOW}: workflow_dispatch must validate that GITHUB_REF is a version tag")
    if "workflow_dispatch" in workflow.triggers:
        if not _any_run_contains(workflow, "release_ref", "REQUESTED_RELEASE_REF", "GITHUB_EVENT_NAME", "workflow_dispatch"):
            foundation.add(errors, f"{WORKFLOW}: workflow_dispatch must validate an explicit release_ref version tag")
        if not _build_checkout_targets_release_ref(workflow):
            foundation.add(errors, f"{WORKFLOW}: build checkout must target needs.preflight.outputs.release_ref")


def check_secret_scope(workflow: Workflow | None, raw: str, errors: list[str]) -> None:
    if workflow is None:
        return
    uncommented = without_comment_lines(raw)
    if "pull_request" in workflow.triggers and any(secret in uncommented for secret in SIGNING_SECRETS):
        foundation.add(errors, f"{WORKFLOW}: pull_request workflows must not expose signing secret names")
    secret_jobs = [job for job in workflow.jobs.values() if _job_uses_secret(job)]
    for job in secret_jobs:
        if "flatpak-beta-signing" not in job.environment:
            foundation.add(errors, f"{WORKFLOW}: signing secrets must be scoped to flatpak-beta-signing environment")
    if any(secret in uncommented for secret in SIGNING_SECRETS) and not secret_jobs:
        foundation.add(errors, f"{WORKFLOW}: signing secrets must be scoped to flatpak-beta-signing environment")
    if workflow.permissions.get("contents") != "read":
        foundation.add(errors, f"{WORKFLOW}: release workflow must default to contents: read permissions")
    for permission in ("pages", "id-token"):
        if workflow.permissions.get(permission) == "write":
            foundation.add(errors, f"{WORKFLOW}: {permission}: write must stay scoped to the deploy job")
    pages_jobs = [job for job in workflow.jobs.values() if job.permissions.get("pages") == "write"]
    oidc_jobs = [job for job in workflow.jobs.values() if job.permissions.get("id-token") == "write"]
    for job in pages_jobs:
        if job.environment != "github-pages":
            foundation.add(errors, f"{WORKFLOW}: pages: write must stay scoped to the deploy job")
    for job in oidc_jobs:
        if job.environment != "github-pages":
            foundation.add(errors, f"{WORKFLOW}: id-token: write must stay scoped to the deploy job")
    if pages_jobs and not any(job.permissions.get("id-token") == "write" for job in pages_jobs):
        foundation.add(errors, f"{WORKFLOW}: deploy job must pair pages: write with id-token: write")


def check_validation_gate(
    policy: dict[str, Any],
    workflow: Workflow | None,
    active: set[str],
    errors: list[str],
) -> None:
    check_required_validate_checks(policy, workflow, active, errors)
    if has_validation_before_secret(policy, workflow):
        return
    if "RIT-AUD-001" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: signing secret import requires exact-commit validation gate before signing")


def has_validation_before_secret(policy: dict[str, Any], workflow: Workflow | str | None) -> bool:
    if isinstance(workflow, str):
        workflow = parse_workflow(workflow, WORKFLOW)
    if workflow is None:
        return False
    required_checks = required_validate_check_contexts(policy)
    runs = _validation_runs_before_secret(workflow)
    parsed_checks = workflow_required_checks("\n".join(runs))
    status_gate = (
        bool(required_checks)
        and parsed_checks == required_checks
        and any(_run_is_exact_check_gate(run) for run in runs)
    )
    suite = policy.get("signed_flatpak_publish", {}).get("hard_requirements", {}).get("release_critical_validation_suite", [])
    rerun_gate = bool(suite) and all(_suite_item_present("\n".join(runs), str(item)) for item in suite)
    return status_gate or rerun_gate


def check_required_validate_checks(
    policy: dict[str, Any],
    workflow: Workflow | None,
    active: set[str],
    errors: list[str],
) -> None:
    required = required_validate_check_contexts(policy)
    actual = workflow_required_checks("\n".join(_validation_runs_before_secret(workflow))) if workflow else []
    if required and actual == required:
        return
    if "RIT-AUD-001" in active:
        return
    if not required:
        foundation.add(errors, f"{POLICY_FILE}: required_validate_check_contexts must be a non-empty list")
    else:
        foundation.add(errors, f"{WORKFLOW}: required_checks must exactly match policy required_validate_check_contexts")


def check_ruleset_governance_wiring(workflow: Workflow | None, errors: list[str]) -> None:
    if workflow is None:
        return
    job = workflow.jobs.get("ruleset-governance")
    if job is None:
        foundation.add(errors, f"{VALIDATE_WORKFLOW}: ruleset-governance job is required")
        return
    if not any("python3 -m tools.ruleset_governance_check" in step.run for step in job.steps):
        foundation.add(errors, f"{VALIDATE_WORKFLOW}: ruleset-governance must run tools.ruleset_governance_check")
    token_env = job.env | {key: value for step in job.steps for key, value in step.env.items()}
    if not any(key in token_env and "secrets.RULESET_GOVERNANCE_TOKEN" in token_env[key] for key in ("GITHUB_TOKEN", "GH_TOKEN")):
        foundation.add(errors, f"{VALIDATE_WORKFLOW}: ruleset-governance must use RULESET_GOVERNANCE_TOKEN")
    native = workflow.jobs.get("native-tests")
    if native and _job_mentions_token(native):
        foundation.add(errors, f"{VALIDATE_WORKFLOW}: native-tests must not pass GitHub tokens into the container")


def required_validate_check_contexts(policy: dict[str, Any]) -> list[str]:
    raw = (
        policy.get("signed_flatpak_publish", {})
        .get("hard_requirements", {})
        .get("required_validate_check_contexts", [])
    )
    if not isinstance(raw, list) or not all(isinstance(item, str) and item.strip() for item in raw):
        return []
    return [str(item).strip() for item in raw]


def workflow_required_checks(text: str) -> list[str]:
    match = re.search(r"(?ms)^\s*required_checks=\(\s*(.*?)^\s*\)", text)
    if match is None:
        return []
    checks: list[str] = []
    for line in match.group(1).splitlines():
        value = line.split("#", 1)[0].strip().strip("\"'")
        if value:
            checks.append(value)
    return checks


def without_comment_lines(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))


def _validation_runs_before_secret(workflow: Workflow | None) -> list[str]:
    if workflow is None:
        return []
    secret_jobs = [job for job in workflow.jobs.values() if _job_uses_secret(job)]
    if not secret_jobs:
        return [step.run for job in workflow.jobs.values() for step in job.steps if step.run]
    allowed_job_ids: set[str] = set()
    for job in secret_jobs:
        _collect_needs(workflow, job, allowed_job_ids)
    runs = [step.run for job_id in allowed_job_ids for step in workflow.jobs[job_id].steps if step.run]
    return runs


def _collect_needs(workflow: Workflow, job: Any, found: set[str]) -> None:
    for needed in job.needs:
        if needed in found or needed not in workflow.jobs:
            continue
        found.add(needed)
        _collect_needs(workflow, workflow.jobs[needed], found)


def _run_is_exact_check_gate(run: str) -> bool:
    required_tokens = (
        "tag_commit",
        "check-runs",
        "head_sha",
        "github-actions",
        "conclusion",
        "success",
        "runs_by_name",
    )
    if "tail -n 1" in run:
        return False
    if not all(token in run for token in required_tokens):
        return False
    return "sys.exit(1)" in run or "exit 1" in run


def _any_run_contains(workflow: Workflow, *tokens: str) -> bool:
    return any(all(token in step.run for token in tokens) for job in workflow.jobs.values() for step in job.steps)


def _build_checkout_targets_release_ref(workflow: Workflow) -> bool:
    build = workflow.jobs.get("build")
    if build is None:
        return False
    for step in build.steps:
        if not step.uses.startswith("actions/checkout@"):
            continue
        with_value = step.raw.get("with")
        if isinstance(with_value, dict) and "needs.preflight.outputs.release_ref" in str(with_value.get("ref", "")):
            return True
    return False


def _job_uses_secret(job: Any) -> bool:
    values = [*job.env.keys(), *job.env.values()]
    for step in job.steps:
        values.extend(step.env.keys())
        values.extend(step.env.values())
        values.append(step.run)
    return any(secret in str(value) for secret in SIGNING_SECRETS for value in values)


def _job_mentions_token(job: Any) -> bool:
    values = [*job.env.keys(), *job.env.values()]
    for step in job.steps:
        values.extend(step.env.keys())
        values.extend(step.env.values())
        values.append(step.run)
    return any(token in str(value) for token in ("GITHUB_TOKEN", "GH_TOKEN") for value in values)


def _suite_item_present(text: str, item: str) -> bool:
    if item in text:
        return True
    lowered = item.lower()
    if "flatpak build" in lowered or "flatpak" in lowered and "smoke" in lowered:
        return "flatpak-builder" in text or "flatpak run" in text
    return False
