from __future__ import annotations

import ast
import re
import shlex
from typing import Any

from tools.checks import foundation
from tools.checks._workflow_parser import Job, Step, Workflow, WorkflowParseError, parse_workflow


WORKFLOW = ".github/workflows/publish-flatpak.yml"
VALIDATE_WORKFLOW = ".github/workflows/validate.yml"
POLICY_FILE = "policy/release.policy.json"
SIGNING_SECRETS = ("FLATPAK_GPG_PRIVATE_KEY", "FLATPAK_GPG_PASSPHRASE", "FLATPAK_GPG_KEY_ID")
CANDIDATE_REF_SOURCE = "validated_release_ref"


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


def check_monotonic_candidate_ref(
    policy: dict[str, Any],
    workflow: Workflow | None,
    errors: list[str],
) -> None:
    configured = (
        policy.get("signed_flatpak_publish", {})
        .get("monotonic_remote_update", {})
        .get("candidate_ref_source")
    )
    if configured != CANDIDATE_REF_SOURCE:
        foundation.add(
            errors,
            f"{POLICY_FILE}: monotonic_remote_update.candidate_ref_source must be {CANDIDATE_REF_SOURCE}",
        )
    if workflow is not None and not _monotonic_candidate_uses_release_ref(workflow):
        foundation.add(errors, f"{WORKFLOW}: monotonic rollback candidate ref must use validated release_ref")


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
    app_slug = required_check_app_slug(policy)
    if not required_checks or not app_slug:
        return False
    secret_jobs = [job for job in workflow.jobs.values() if _job_uses_secret(job)]
    if not secret_jobs:
        return False
    for secret_job in secret_jobs:
        ancestors: set[str] = set()
        _collect_needs(workflow, secret_job, ancestors)
        chain = [secret_job, *(workflow.jobs[key] for key in ancestors)]
        if any(_has_condition(job) or _continues_on_error(job) for job in chain):
            return False
        if not any(
            _step_is_strict_check_gate(workflow, workflow.jobs[key], step)
            for key in ancestors for step in workflow.jobs[key].steps
        ):
            return False
    return True


def check_required_validate_checks(
    policy: dict[str, Any],
    workflow: Workflow | None,
    active: set[str],
    errors: list[str],
) -> None:
    required = required_validate_check_contexts(policy)
    app_slug = required_check_app_slug(policy)
    has_invocation = bool(workflow) and any(
        _step_is_strict_check_gate(workflow, job, step) for job, step in _validation_steps_before_secret(workflow)
    )
    if required and app_slug and has_invocation:
        return
    if "RIT-AUD-001" in active:
        return
    if not required:
        foundation.add(errors, f"{POLICY_FILE}: required_validate_check_contexts must be a non-empty list")
    elif not app_slug:
        foundation.add(errors, f"{POLICY_FILE}: required_check_app_slug must be a non-empty string")
    else:
        foundation.add(errors, f"{WORKFLOW}: strict release-check-runs invocation is required before signing")


def check_ruleset_governance_wiring(
    policy: dict[str, Any],
    workflow: Workflow | None,
    errors: list[str],
) -> None:
    if workflow is None:
        return
    _check_required_validate_jobs(policy, workflow, errors)
    job = workflow.jobs.get("ruleset-governance")
    if job is None:
        foundation.add(errors, f"{VALIDATE_WORKFLOW}: ruleset-governance job is required")
        return
    governance_steps = [step for step in job.steps if _is_governance_step(step)]
    if len(governance_steps) != 1:
        foundation.add(errors, f"{VALIDATE_WORKFLOW}: ruleset-governance must run tools.ruleset_governance_check")
    else:
        expected = (
            policy.get("github_actions_release_safety", {})
            .get("repository_governance", {})
            .get("validation_step_condition")
        )
        if not _supported_run_context(workflow, job, governance_steps[0], require_root=True):
            foundation.add(errors, f"{VALIDATE_WORKFLOW}: governance must execute in the repository root with the supported shell")
        actual = governance_steps[0].raw.get("if")
        if not isinstance(expected, str) or not expected.strip() or actual != expected:
            foundation.add(errors, f"{VALIDATE_WORKFLOW}: ruleset-governance must use the approved execution condition")
    token_env = job.env | (governance_steps[0].env if len(governance_steps) == 1 else {})
    if not any(token_env.get(key) == "${{ secrets.RULESET_GOVERNANCE_TOKEN }}" for key in ("GITHUB_TOKEN", "GH_TOKEN")):
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


def required_check_app_slug(policy: dict[str, Any]) -> str:
    raw = (
        policy.get("signed_flatpak_publish", {})
        .get("hard_requirements", {})
        .get("required_check_app_slug")
    )
    return raw.strip() if isinstance(raw, str) else ""


def without_comment_lines(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))


def _validation_steps_before_secret(workflow: Workflow | None) -> list[tuple[Job, Step]]:
    if workflow is None:
        return []
    secret_jobs = [job for job in workflow.jobs.values() if _job_uses_secret(job)]
    if not secret_jobs:
        return [(job, step) for job in workflow.jobs.values() for step in job.steps if step.run]
    allowed_job_ids: set[str] = set()
    for job in secret_jobs:
        _collect_needs(workflow, job, allowed_job_ids)
    return [
        (workflow.jobs[job_id], step)
        for job_id in sorted(allowed_job_ids)
        for step in workflow.jobs[job_id].steps
        if step.run
    ]


def _collect_needs(workflow: Workflow, job: Any, found: set[str]) -> None:
    for needed in job.needs:
        if needed in found or needed not in workflow.jobs:
            continue
        found.add(needed)
        _collect_needs(workflow, workflow.jobs[needed], found)


def _step_is_strict_check_gate(workflow: Workflow, job: Job, step: Step) -> bool:
    expected = [
        "python3",
        "-m",
        "tools.release_check_runs",
        "--input",
        "$CHECK_RUNS_JSON",
        "--policy",
        "policy/release.policy.json",
        "--head-sha",
        "$TAG_COMMIT",
    ]
    if _has_condition(job) or _has_condition(step) or _continues_on_error(job) or _continues_on_error(step):
        return False
    return (
        _supported_run_context(workflow, job, step, require_root=True)
        and step.env.get("CHECK_RUNS_JSON") == "${{ runner.temp }}/release-check-runs.json"
        and step.env.get("TAG_COMMIT") == "${{ steps.release.outputs.tag_commit }}"
        and _shell_commands(step.run) == [["set", "-euo", "pipefail"], expected]
    )


def _supported_run_context(workflow: Workflow, job: Job, step: Step, *, require_root: bool = False) -> bool:
    # Only the normal Linux bash execution contract is supported. Custom shell
    # templates can return success without executing the inspected run block.
    context: dict[str, Any] = {}
    for raw in (workflow.raw, job.raw):
        defaults = raw.get("defaults", {})
        if not isinstance(defaults, dict) or not isinstance(defaults.get("run", {}), dict):
            return False
        context.update(defaults.get("run", {}))
    context.update({key: step.raw[key] for key in ("shell", "working-directory") if key in step.raw})
    return context.get("shell", "bash") == "bash" and (
        not require_root or context.get("working-directory", ".") == "."
    )


def _shell_commands(run: str) -> list[list[str]]:
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


def _any_run_contains(workflow: Workflow, *tokens: str) -> bool:
    return any(all(token in step.run for token in tokens) for job in workflow.jobs.values() for step in job.steps)


def _build_checkout_targets_release_ref(workflow: Workflow) -> bool:
    build = workflow.jobs.get("build")
    if build is None:
        return False
    checkouts = [step for step in build.steps if step.uses.startswith("actions/checkout@")]
    if len(checkouts) != 1:
        return False
    with_value = checkouts[0].raw.get("with")
    return isinstance(with_value, dict) and with_value.get("ref") == "${{ needs.preflight.outputs.release_ref }}"


def _monotonic_candidate_uses_release_ref(workflow: Workflow) -> bool:
    preflight = workflow.jobs.get("preflight")
    if preflight is None:
        return False
    release_steps = [step for step in preflight.steps if step.raw.get("id") == "release"]
    if len(release_steps) != 1:
        return False
    step = release_steps[0]
    if (
        _has_condition(preflight)
        or _has_condition(step)
        or _continues_on_error(preflight)
        or _continues_on_error(step)
        or not _supported_run_context(workflow, preflight, step, require_root=True)
    ):
        return False
    blocks = [
        block
        for block in _top_level_python_blocks(step.run)
        if _is_monotonic_body(block[1])
    ]
    if len(blocks) != 1:
        return False
    assignments: list[tuple[str, str]] = []
    for line in blocks[0][0]:
        match = re.fullmatch(r'([A-Z][A-Z0-9_]*)=(\S.*)[ \t]+\\', line)
        if match is None:
            return False
        assignments.append((match.group(1), match.group(2)))
    candidate_values = [value for name, value in assignments if name == "CANDIDATE_REF"]
    return candidate_values == ['"$release_ref"']


def _is_monotonic_body(body: str) -> bool:
    """Recognise the actual input and comparison statements, never comment tokens."""
    try:
        module = ast.parse(body)
    except SyntaxError:
        return False
    expected_ref = ast.dump(ast.parse('candidate_ref = os.environ["CANDIDATE_REF"]').body[0])
    ref_writes = [
        node for node in ast.walk(module)
        if isinstance(node, ast.Name) and node.id == "candidate_ref" and isinstance(node.ctx, ast.Store)
    ]
    if len(ref_writes) != 1 or expected_ref not in [ast.dump(node) for node in module.body]:
        return False
    expected_key = ast.dump(ast.parse("candidate_key = version_key(candidate)").body[0])
    if expected_key not in [ast.dump(node) for node in module.body]:
        return False
    expected_test = ast.dump(ast.parse("candidate_key == published_key", mode="eval").body)
    comparisons = [
        node for node in module.body
        if isinstance(node, ast.If) and ast.dump(node.test) == expected_test
    ]
    expected_source = ast.dump(ast.parse(
        "same_source = published_ref == candidate_ref and published_commit == candidate_commit"
    ).body[0])
    return len(comparisons) == 1 and expected_source in [ast.dump(node) for node in comparisons[0].body]


def _top_level_python_blocks(run: str) -> list[tuple[list[str], str]]:
    lines = run.splitlines()
    blocks: list[tuple[list[str], str]] = []
    controls: list[str] = []
    substitutions: list[tuple[str, int]] = []
    quote = ""
    index = 0
    while index < len(lines):
        stripped = lines[index].strip()
        quote = _update_substitution_scope(lines[index], substitutions, quote)
        if "<<'PY'" in stripped:
            end = index + 1
            while end < len(lines) and lines[end].strip() != "PY":
                end += 1
            if end >= len(lines):
                return []
            prefix_start = index
            while prefix_start > 0 and lines[prefix_start - 1].rstrip().endswith("\\"):
                prefix_start -= 1
            if (
                stripped == "python3 - <<'PY'"
                and not controls
                and not substitutions
                and not quote
                and all(lines[item] == lines[item].lstrip() for item in range(prefix_start, index + 1))
            ):
                blocks.append((lines[prefix_start:index], "\n".join(lines[index + 1 : end])))
            index = end + 1
            continue
        if not _update_control_stack(stripped, controls):
            return []
        index += 1
    return blocks if not controls and not substitutions and not quote else []


def _update_substitution_scope(line: str, stack: list[tuple[str, int]], quote: str) -> str:
    """Track multiline $(...) owners while preserving their surrounding quotes."""
    index = 0
    while index < len(line):
        char = line[index]
        if char == "\\" and quote != "'":
            index += 2
            continue
        if quote == "'":
            if char == "'":
                quote = ""
        elif line[index:index + 2] == "$(":
            stack.append((quote, 1))
            quote = ""
            index += 1
        elif quote:
            if char == quote:
                quote = ""
        elif char in {"'", '"'}:
            quote = char
        elif char == "#" and (index == 0 or line[index - 1].isspace()):
            break
        elif stack and char in {"(", ")"}:
            parent_quote, depth = stack[-1]
            depth += 1 if char == "(" else -1
            if depth:
                stack[-1] = parent_quote, depth
            else:
                stack.pop()
                quote = parent_quote
        index += 1
    return quote


def _update_control_stack(line: str, controls: list[str]) -> bool:
    command = line.split("#", maxsplit=1)[0].strip()
    if not command:
        return True
    closing = {"fi": "if", "done": "loop", "esac": "case", "}": "brace"}
    if re.fullmatch(r'}\s+>>?\s+"\$GITHUB_OUTPUT"', command):
        return bool(controls) and controls.pop() == "brace"
    if command in closing:
        return bool(controls) and controls.pop() == closing[command]
    if re.match(r"^if\b.*;\s*then$", command):
        controls.append("if")
    elif re.match(r"^(?:for|while|until)\b.*;\s*do$", command):
        controls.append("loop")
    elif re.match(r"^case\b.*\bin$", command):
        controls.append("case")
    elif command == "{" or re.match(r"^[A-Za-z_][A-Za-z0-9_]*\(\)\s*\{$", command):
        controls.append("brace")
    elif re.match(r"^(?:if|then|for|while|until|select|do|case|function)\b", command):
        return False  # Unsupported shell control cannot establish an active owner.
    elif command in {"(", ")"} or re.search(r"(?:&&|\|\|)\s*\($", command):
        return False
    return True


def _check_required_validate_jobs(policy: dict[str, Any], workflow: Workflow, errors: list[str]) -> None:
    requirements = policy.get("signed_flatpak_publish", {}).get("hard_requirements", {})
    required = required_validate_check_contexts(policy)
    if requirements.get("required_validate_jobs_must_run") is not True:
        foundation.add(errors, f"{POLICY_FILE}: required_validate_jobs_must_run must be true")
    if requirements.get("required_validate_checks_must_not_continue_on_error") is not True:
        foundation.add(errors, f"{POLICY_FILE}: required_validate_checks_must_not_continue_on_error must be true")
    for job_id in required:
        job = workflow.jobs.get(job_id)
        if job is None:
            foundation.add(errors, f"{VALIDATE_WORKFLOW}: required Validate job {job_id} is missing")
            continue
        if _has_condition(job):
            foundation.add(errors, f"{VALIDATE_WORKFLOW}: {job_id} job must run unconditionally")
        if _continues_on_error(job):
            foundation.add(errors, f"{VALIDATE_WORKFLOW}: {job_id} job must not continue on error")
        for step in job.steps:
            if step.run and not _supported_run_context(workflow, job, step):
                foundation.add(errors, f"{VALIDATE_WORKFLOW}: {job_id} gate step must use the supported execution shell")
            if _continues_on_error(step):
                foundation.add(errors, f"{VALIDATE_WORKFLOW}: {job_id} step must not continue on error")
            if _has_condition(step) and not _allowed_conditional_step(job_id, step):
                foundation.add(errors, f"{VALIDATE_WORKFLOW}: {job_id} gate step must run unconditionally")


def _allowed_conditional_step(job_id: str, step: Step) -> bool:
    if job_id == "ruleset-governance" and _is_governance_step(step):
        return True
    condition = str(step.raw.get("if", "")).strip()
    return step.uses.startswith("actions/upload-artifact@") and condition in {
        "failure()",
        "failure() || cancelled()",
        "cancelled() || failure()",
    }


def _is_governance_step(step: Step) -> bool:
    return _shell_commands(step.run) == [["python3", "-m", "tools.ruleset_governance_check"]]


def _has_condition(value: Job | Step) -> bool:
    return bool(str(value.raw.get("if", "")).strip())


def _continues_on_error(value: Job | Step) -> bool:
    raw = value.raw.get("continue-on-error")
    return raw is not None and str(raw).strip().lower() != "false"


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
