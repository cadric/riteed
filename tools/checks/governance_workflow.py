from __future__ import annotations

import re
import shlex
from typing import Any

from tools.checks import foundation
from tools.checks._workflow_parser import Job, Step, Workflow


POLICY_FILE = "policy/release.policy.json"
WORKFLOW = ".github/workflows/validate.yml"
STATIC_IDENTITY = [
    ["set", "-euo", "pipefail"],
    ["actual_head=$(git rev-parse HEAD)"],
    ["test", "$actual_head", "=", "$GITHUB_SHA"],
]
LIVE_IDENTITY = [
    ["set", "-euo", "pipefail"],
    ["case", "$GITHUB_EVENT_NAME", "in"],
    ["push|schedule|workflow_dispatch)", ";;"],
    ["*)", "exit", "1", ";;"],
    ["esac"],
    ["test", "$GITHUB_REF", "=", "refs/heads/main"],
    ["test", "$GITHUB_REPOSITORY", "=", "cadric/riteed"],
    ["actual_head=$(git rev-parse HEAD)"],
    ["test", "$actual_head", "=", "$GITHUB_SHA"],
]


def check(policy: dict[str, Any], workflow: Workflow | None, errors: list[str]) -> None:
    config = _config(policy, errors)
    if workflow is None or config is None:
        return
    if not _triggers_are_reviewed(workflow):
        foundation.add(
            errors,
            f"{WORKFLOW}: governance producers must be PR plus main push, schedule and workflow_dispatch",
        )
    _check_static(workflow, config, errors)
    _check_live(workflow, config, errors)


def truthful_config(policy: dict[str, Any]) -> dict[str, Any] | None:
    raw = (
        policy.get("github_actions_release_safety", {})
        .get("repository_governance", {})
        .get("truthful_checks")
    )
    return raw if isinstance(raw, dict) else None


def _config(policy: dict[str, Any], errors: list[str]) -> dict[str, Any] | None:
    raw = truthful_config(policy)
    required = (
        policy.get("signed_flatpak_publish", {})
        .get("hard_requirements", {})
        .get("required_validate_check_contexts")
    )
    expected_required = [
        "dependency-preflight",
        "policy-pack",
        "native-tests",
        "governance-static",
        "flatpak-tests",
        "flatpak",
    ]
    expected = {
        "static_context": "governance-static",
        "static_command": "python3 -m tools.policy_check --release-static-check --root app --strict",
        "live_context": "governance-live",
        "live_environment": "ruleset-governance-live",
        "live_secret": "RULESET_GOVERNANCE_TOKEN",
        "live_allowed_events": ["push", "schedule", "workflow_dispatch"],
        "live_workflow_name": "Validate",
        "live_workflow_path": WORKFLOW,
        "live_job": "governance-live",
        "live_decisive_step": "Verify GitHub ruleset governance",
        "main_ref": "refs/heads/main",
        "main_branch": "main",
        "repository_secret_forbidden": True,
        "environment_branch_policy": {
            "protected_branches": False,
            "custom_branch_policies": True,
            "name": "main",
            "type": "branch",
        },
    }
    if raw != expected or required != expected_required:
        foundation.add(
            errors,
            f"{POLICY_FILE}: repository_governance.truthful_checks must match the reviewed static/live contract",
        )
        return None
    return expected


def _check_static(workflow: Workflow, config: dict[str, Any], errors: list[str]) -> None:
    job = workflow.jobs.get(config["static_context"])
    if job is None:
        foundation.add(errors, f"{WORKFLOW}: governance-static job is required")
        return
    checkouts = [step for step in job.steps if step.uses.startswith("actions/checkout@")]
    setups = [step for step in job.steps if step.uses.startswith("actions/setup-python@")]
    identity = [step for step in job.steps if step.name == "Verify candidate checkout"]
    gates = [step for step in job.steps if step.run.strip() == config["static_command"]]
    checkout_index = job.steps.index(checkouts[0]) if len(checkouts) == 1 else -1
    identity_index = job.steps.index(identity[0]) if len(identity) == 1 else -1
    gate_index = job.steps.index(gates[0]) if len(gates) == 1 else -1
    safe = (
        not _condition(job)
        and not _continues(job)
        and not job.environment
        and job.permissions == {"contents": "read"}
        and not _contains_privileged_secret(workflow.raw.get("env", {}))
        and not _contains_privileged_secret(job.raw.get("env", {}))
        and len(checkouts) == 1
        and checkouts[0].raw.get("with") == {"ref": "${{ github.sha }}"}
        and _active_action(checkouts[0])
        and len(setups) == 1
        and setups[0].raw.get("with") == {"python-version": "3.12"}
        and _active_action(setups[0])
        and len(identity) == 1
        and _active_root_bash(workflow, job, identity[0])
        and _commands(identity[0].run) == STATIC_IDENTITY
        and len(gates) == 1
        and _active_root_bash(workflow, job, gates[0])
        and checkout_index < identity_index < gate_index
        and job.steps == [checkouts[0], setups[0], identity[0], gates[0]]
        and _dependency_chain_is_unconditional(workflow, job)
        and all(not _contains_privileged_secret(step.raw) for step in job.steps)
    )
    if not safe:
        foundation.add(
            errors,
            f"{WORKFLOW}: governance-static must be unconditional, tokenless and check the event checkout with the exact static command",
        )


def _check_live(workflow: Workflow, config: dict[str, Any], errors: list[str]) -> None:
    job = workflow.jobs.get(config["live_job"])
    if job is None:
        foundation.add(errors, f"{WORKFLOW}: governance-live job is required")
        return
    expected_if = (
        "(github.event_name == 'push' || github.event_name == 'schedule' || "
        "github.event_name == 'workflow_dispatch') && github.ref == 'refs/heads/main'"
    )
    checkouts = [step for step in job.steps if step.uses.startswith("actions/checkout@")]
    setups = [step for step in job.steps if step.uses.startswith("actions/setup-python@")]
    identities = [step for step in job.steps if step.name == "Verify trusted main checkout"]
    decisive = [step for step in job.steps if step.name == config["live_decisive_step"]]
    secret_steps = [
        index for index, step in enumerate(job.steps) if _contains_privileged_secret(step.raw)
    ]
    decisive_index = job.steps.index(decisive[0]) if len(decisive) == 1 else -1
    safe = (
        str(job.raw.get("if", "")) == expected_if
        and not _continues(job)
        and job.environment == config["live_environment"]
        and job.permissions == {"contents": "read"}
        and not _contains_privileged_secret(workflow.raw.get("env", {}))
        and not _contains_privileged_secret(job.raw.get("env", {}))
        and len(checkouts) == 1
        and checkouts[0].raw.get("with") == {"ref": "${{ github.sha }}"}
        and _active_action(checkouts[0])
        and len(setups) == 1
        and setups[0].raw.get("with") == {"python-version": "3.12"}
        and _active_action(setups[0])
        and len(identities) == 1
        and _active_root_bash(workflow, job, identities[0])
        and _commands(identities[0].run) == LIVE_IDENTITY
        and len(decisive) == 1
        and _active_root_bash(workflow, job, decisive[0])
        and _commands(decisive[0].run) == [["python3", "-m", "tools.ruleset_governance_check"]]
        and decisive[0].env
        == {"GITHUB_TOKEN": "${{ secrets.RULESET_GOVERNANCE_TOKEN }}"}
        and secret_steps == [decisive_index]
        and job.steps.index(checkouts[0]) < job.steps.index(identities[0]) < decisive_index
        and job.steps == [checkouts[0], setups[0], identities[0], decisive[0]]
        and _dependency_chain_is_unconditional(workflow, job)
    )
    if not safe:
        foundation.add(
            errors,
            f"{WORKFLOW}: governance-live must be protected, main-only, SHA-bound and expose its environment secret only to the decisive step",
        )


def _active_root_bash(workflow: Workflow, job: Job, step: Step) -> bool:
    if _condition(step) or _continues(step):
        return False
    context: dict[str, Any] = {}
    for raw in (workflow.raw, job.raw):
        defaults = raw.get("defaults", {})
        if not isinstance(defaults, dict) or not isinstance(defaults.get("run", {}), dict):
            return False
        context.update(defaults.get("run", {}))
    context.update(
        {key: step.raw[key] for key in ("shell", "working-directory") if key in step.raw}
    )
    return context.get("shell", "bash") == "bash" and context.get("working-directory", ".") == "."


def _triggers_are_reviewed(workflow: Workflow) -> bool:
    if set(workflow.triggers) != {"push", "pull_request", "workflow_dispatch", "schedule"}:
        return False
    push = workflow.triggers.get("push")
    return isinstance(push, dict) and push.get("branches") == ["main"]


def _active_action(step: Step) -> bool:
    return not _condition(step) and not _continues(step)


def _dependency_chain_is_unconditional(workflow: Workflow, job: Job) -> bool:
    verified: set[str] = set()

    def visit(job_id: str, active: set[str]) -> bool:
        if job_id in verified:
            return True
        if job_id in active:
            return False
        dependency = workflow.jobs.get(job_id)
        if dependency is None or _condition(dependency) or _continues(dependency):
            return False
        nested = active | {job_id}
        if not all(visit(item, nested) for item in dependency.needs):
            return False
        verified.add(job_id)
        return True

    return all(visit(job_id, {job.job_id}) for job_id in job.needs)


def _commands(run: str) -> list[list[str]]:
    normalized = re.sub(r"\\[ \t]*\n[ \t]*", " ", run)
    commands: list[list[str]] = []
    for line in normalized.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        try:
            commands.append(shlex.split(stripped, posix=True))
        except ValueError:
            return []
    return commands


def _condition(value: Job | Step) -> bool:
    return bool(str(value.raw.get("if", "")).strip())


def _continues(value: Job | Step) -> bool:
    raw = value.raw.get("continue-on-error")
    return raw is not None and str(raw).strip().lower() != "false"


def _contains_governance_secret(value: Any) -> bool:
    if isinstance(value, dict):
        return any(
            _contains_governance_secret(key) or _contains_governance_secret(item)
            for key, item in value.items()
        )
    if isinstance(value, (list, tuple)):
        return any(_contains_governance_secret(item) for item in value)
    return "RULESET_GOVERNANCE_TOKEN" in str(value)


def _contains_privileged_secret(value: Any) -> bool:
    text = str(value)
    return (
        _contains_governance_secret(value)
        or "secrets." in text
        or "GH_TOKEN" in text
        or "GITHUB_TOKEN" in text
    )
