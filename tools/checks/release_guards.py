from __future__ import annotations

import re
import shlex
from dataclasses import dataclass
from typing import Any

from tools.checks import foundation, release_identity, release_workflow
from tools.checks._workflow_parser import Job, Step, Workflow

WORKFLOW = ".github/workflows/publish-flatpak.yml"
POLICY_FILE = "policy/release.policy.json"
APPSTREAM_COMMAND = (
    'TAG_COMMIT="$tag_commit" VERSION="$version" python3 - <<\'PY\''
)
FETCH_COMMAND = (
    'git fetch origin "+refs/tags/$release_ref:refs/tags/$release_ref" '
    "+refs/heads/main:refs/remotes/origin/main"
)
TAG_COMMIT_COMMAND = (
    'tag_commit="$(git rev-parse --verify "refs/tags/$release_ref^{commit}")"'
)
VERSION_COMMAND = (
    'version="$(git show "$tag_commit:app/Cargo.toml" | sed -n '
    '\'s/^version = "\\(.*\\)"/\\1/p\' | head -n 1)"'
)
CHECK_COLLECTOR_COMMAND = (
    'CHECK_RUNS_JSON="$checks_json" TAG_COMMIT="$tag_commit" python3 - <<\'PY\''
)
SHA_GUARD_BLOCK = (
    'if [[ ! "$tag_commit" =~ ^[0-9a-f]{40}$ ]]; then\n'
    '  echo "Release tag must resolve to one full commit SHA." >&2\n'
    "  exit 1\n"
    "fi"
)
RELEASE_REF_GUARD_BLOCK = (
    'if [[ ! "$release_ref" =~ ^v[0-9]+[.][0-9]+[.][0-9]+'
    '([-.+][A-Za-z0-9.-]+)?$ ]]; then\n'
    '  echo "Flatpak publish target must be a SemVer version tag." >&2\n'
    "  exit 1\n"
    "fi"
)
ANCESTRY_COMMANDS = (
    FETCH_COMMAND,
    TAG_COMMIT_COMMAND,
    'git merge-base --is-ancestor "$tag_commit" origin/main',
)
PRIVATE_IMPORT = "printf '%s' \"$FLATPAK_GPG_PRIVATE_KEY\" | gpg --batch --import"
GNUPG_SETUP = (
    'export GNUPGHOME="$(mktemp -d)"',
    'chmod 700 "$GNUPGHOME"',
    "cleanup() {",
    "trap cleanup EXIT",
    PRIVATE_IMPORT,
)
HOSTED_UBUNTU = re.compile(r"ubuntu-(?:latest|[0-9]{2}[.][0-9]{2}(?:-arm)?)")


@dataclass(frozen=True)
class GuardContext:
    preflight: Job | None
    release_step: Step | None
    build: Job | None
    signing_step: Step | None


@dataclass(frozen=True)
class ShellLine:
    index: int
    text: str
    controls: tuple[str, ...]
    substituted: bool
    heredoc_marker: str = ""
    heredoc_body: str = ""


def check(policy: dict[str, Any], workflow: Workflow | None, errors: list[str]) -> None:
    _check_policy_requirements(policy, errors)
    if workflow is None:
        return
    context = _guard_context(workflow)
    _check_ancestry(workflow, context, errors)
    _check_release_identity(workflow, context, errors)
    _check_appstream(workflow, context, errors)
    _check_signing_hygiene(workflow, context, errors)
    _check_hosted_runner(context, errors)


def ancestry_commands(workflow: Workflow) -> list[str]:
    context = _guard_context(workflow)
    if not _active_root_step(workflow, context.preflight, context.release_step):
        return []
    prefix = _prefix_before(context.release_step.run, 'remote_state="$(mktemp -d)"')
    commands = _top_level_lines(prefix)
    positions = _ordered_positions(commands, ANCESTRY_COMMANDS)
    version_assignment = next(
        (line for line in commands if line.startswith('version="$(git show "$tag_commit:')),
        "",
    )
    stable = (
        _stable_between(prefix, "release_ref", ANCESTRY_COMMANDS[0], ANCESTRY_COMMANDS[2])
        and _stable_between(prefix, "tag_commit", ANCESTRY_COMMANDS[1], ANCESTRY_COMMANDS[2])
        and _stable_between(prefix, "version", version_assignment, APPSTREAM_COMMAND)
    )
    return list(ANCESTRY_COMMANDS) if positions and stable else []


def appstream_python(workflow: Workflow) -> str | None:
    context = _guard_context(workflow)
    if not _active_root_step(workflow, context.preflight, context.release_step):
        return None
    prefix = _prefix_before(context.release_step.run, 'remote_state="$(mktemp -d)"')
    blocks = _top_level_heredocs(prefix)
    matching = [body for command, marker, body in blocks if command == APPSTREAM_COMMAND and marker == "PY"]
    return matching[0] if len(matching) == 1 else None


def release_identity_commands(workflow: Workflow) -> list[str]:
    context = _guard_context(workflow)
    if not _release_identity_is_guarded(workflow, context):
        return []
    return [
        TAG_COMMIT_COMMAND,
        VERSION_COMMAND,
    ]


def _check_policy_requirements(policy: dict[str, Any], errors: list[str]) -> None:
    requirements = (
        (
            policy.get("release_identity", {}).get("tag_commit_must_be_ancestor_of_main"),
            "release_identity.tag_commit_must_be_ancestor_of_main",
        ),
        (
            policy.get("release_identity", {}).get("appstream_top_release_must_match_tag"),
            "release_identity.appstream_top_release_must_match_tag",
        ),
        (
            policy.get("release_identity", {}).get("tag_commit_must_be_exact_sha"),
            "release_identity.tag_commit_must_be_exact_sha",
        ),
        (
            policy.get("release_identity", {}).get("release_content_must_use_tag_commit"),
            "release_identity.release_content_must_use_tag_commit",
        ),
        (
            policy.get("signed_flatpak_publish", {})
            .get("hard_requirements", {})
            .get("workflow_dispatch_build_must_checkout_tag_commit"),
            "signed_flatpak_publish.hard_requirements."
            "workflow_dispatch_build_must_checkout_tag_commit",
        ),
        (
            policy.get("signed_flatpak_publish", {})
            .get("hard_requirements", {})
            .get("checked_out_head_must_match_tag_commit_before_secrets"),
            "signed_flatpak_publish.hard_requirements."
            "checked_out_head_must_match_tag_commit_before_secrets",
        ),
        (
            policy.get("signing_key_governance", {})
            .get("private_key_import", {})
            .get("temporary_gnupg_home_required"),
            "signing_key_governance.private_key_import.temporary_gnupg_home_required",
        ),
        (
            policy.get("signing_key_governance", {})
            .get("private_key_import", {})
            .get("kill_agent_on_exit_required"),
            "signing_key_governance.private_key_import.kill_agent_on_exit_required",
        ),
        (
            policy.get("github_actions_release_safety", {})
            .get("mutable_inputs", {})
            .get("github_hosted_runner_required_until_self_hosted_policy_exists"),
            "github_actions_release_safety.mutable_inputs."
            "github_hosted_runner_required_until_self_hosted_policy_exists",
        ),
    )
    for value, path in requirements:
        if value is not True:
            foundation.add(errors, f"{POLICY_FILE}: {path} must be true")


def _guard_context(workflow: Workflow) -> GuardContext:
    preflight = workflow.jobs.get("preflight")
    release_steps = (
        [step for step in preflight.steps if step.raw.get("id") == "release"]
        if preflight is not None
        else []
    )
    build = workflow.jobs.get("build")
    signing_steps = (
        [
            step
            for step in build.steps
            if "FLATPAK_GPG_PRIVATE_KEY" in step.env
            or "FLATPAK_GPG_PRIVATE_KEY" in step.run
        ]
        if build is not None
        else []
    )
    return GuardContext(
        preflight,
        release_steps[0] if len(release_steps) == 1 else None,
        build,
        signing_steps[0] if len(signing_steps) == 1 else None,
    )


def _check_ancestry(
    workflow: Workflow, context: GuardContext, errors: list[str]
) -> None:
    if ancestry_commands(workflow):
        return
    foundation.add(
        errors,
        f"{WORKFLOW}: tag commit must be an ancestor of origin/main in the active release preflight",
    )


def _check_release_identity(
    workflow: Workflow, context: GuardContext, errors: list[str]
) -> None:
    if _release_identity_is_guarded(workflow, context):
        return
    foundation.add(
        errors,
        f"{WORKFLOW}: release metadata, check collection and outputs must use one stable peeled tag commit SHA",
    )


def _release_identity_is_guarded(workflow: Workflow, context: GuardContext) -> bool:
    if not _active_root_step(workflow, context.preflight, context.release_step):
        return False
    run = context.release_step.run
    scanned = _scan_shell(run)
    if scanned is None:
        return False
    top_level = [line.text for line in scanned if not line.controls and not line.substituted]
    required = (
        FETCH_COMMAND,
        TAG_COMMIT_COMMAND,
        VERSION_COMMAND,
        ANCESTRY_COMMANDS[2],
        CHECK_COLLECTOR_COMMAND,
        APPSTREAM_COMMAND,
        'CANDIDATE_COMMIT="$tag_commit" \\',
        'echo "release_ref=$release_ref" >> "$GITHUB_OUTPUT"',
        'echo "tag_commit=$tag_commit" >> "$GITHUB_OUTPUT"',
    )
    if not _ordered_positions(top_level, required):
        return False
    preflight = context.preflight
    outputs = preflight.raw.get("outputs", {}) if preflight is not None else {}
    expected_outputs = {
        name: f"${{{{ steps.release.outputs.{name} }}}}"
        for name in ("version", "release_ref", "tag_commit")
    }
    if not isinstance(outputs, dict) or any(
        outputs.get(name) != value for name, value in expected_outputs.items()
    ):
        return False
    ref_guard = release_identity.exact_guard_owner(scanned, RELEASE_REF_GUARD_BLOCK)
    sha_guard = release_identity.exact_guard_owner(scanned, SHA_GUARD_BLOCK)
    active_indexes = {
        line.text: line.index
        for line in scanned
        if not line.controls and not line.substituted
    }
    return (
        ref_guard is not None
        and sha_guard is not None
        and ref_guard < active_indexes[FETCH_COMMAND]
        and active_indexes[TAG_COMMIT_COMMAND] < sha_guard
        and sha_guard < active_indexes[VERSION_COMMAND]
        and release_identity.identity_outputs_are_unique(scanned)
        and _stable_after(run, "release_ref", RELEASE_REF_GUARD_BLOCK)
        and _stable_after(run, "tag_commit", TAG_COMMIT_COMMAND)
        and _stable_after(run, "version", VERSION_COMMAND)
    )


def _check_appstream(
    workflow: Workflow, context: GuardContext, errors: list[str]
) -> None:
    source = appstream_python(workflow)
    if source is not None and release_identity.appstream_ast_is_guard(source):
        return
    foundation.add(
        errors,
        f"{WORKFLOW}: AppStream top release must match the release tag in the active release preflight",
    )


def _check_signing_hygiene(
    workflow: Workflow, context: GuardContext, errors: list[str]
) -> None:
    if _signing_hygiene_is_guarded(workflow, context):
        return
    foundation.add(
        errors,
        f"{WORKFLOW}: private key import requires temporary GNUPGHOME cleanup in the active signing step",
    )


def _check_hosted_runner(context: GuardContext, errors: list[str]) -> None:
    build = context.build
    runner = build.raw.get("runs-on") if build is not None else None
    if (
        isinstance(runner, str)
        and HOSTED_UBUNTU.fullmatch(runner)
        and _build_is_active(context)
    ):
        return
    foundation.add(
        errors,
        f"{WORKFLOW}: build signing job must use a GitHub-hosted Ubuntu runner after preflight",
    )


def _signing_hygiene_is_guarded(
    workflow: Workflow, context: GuardContext
) -> bool:
    if not _active_root_step(workflow, context.build, context.signing_step):
        return False
    if not _build_is_active(context):
        return False
    run = context.signing_step.run
    prefix = _prefix_through(run, PRIVATE_IMPORT)
    commands = _top_level_lines(prefix)
    positions = _ordered_positions(commands, GNUPG_SETUP)
    if not positions:
        return False
    cleanup = _function_body(prefix, "cleanup")
    return (
        'gpgconf --homedir "$GNUPGHOME" --kill gpg-agent || true' in cleanup
        and 'rm -rf "$GNUPGHOME"' in cleanup
        and _stable_after(run, "GNUPGHOME", GNUPG_SETUP[0])
        and _cleanup_binding_is_preserved(run)
    )


def _cleanup_binding_is_preserved(run: str) -> bool:
    _, separator, suffix = run.partition(PRIVATE_IMPORT)
    if not separator:
        return False
    for raw in suffix.splitlines()[1:]:
        text = _strip_shell_comment(raw).strip()
        if (
            re.match(r"^cleanup\s*\(\)\s*\{", text)
            or re.match(r"^unset(?:\s+-f)?\s+cleanup(?:\s|$)", text)
            or _trap_targets_exit(text)
        ):
            return False
    return True


def _trap_targets_exit(text: str) -> bool:
    if not text.startswith("trap "):
        return False
    try:
        tokens = shlex.split(text)
    except ValueError:
        return True
    if len(tokens) > 1 and tokens[1] in {"-l", "-p"}:
        return False
    signals = tokens[3:] if len(tokens) > 1 and tokens[1] == "--" else tokens[2:]
    return any(signal in {"0", "EXIT"} for signal in signals)


def _stable_between(run: str, name: str, start: str, end: str) -> bool:
    scanned = _scan_shell(run)
    if scanned is None or not start:
        return False
    owners = [
        [line for line in scanned if line.text == target and not line.controls and not line.substituted]
        for target in (start, end)
    ]
    if any(len(owner) != 1 for owner in owners):
        return False
    first, last = owners[0][0].index, owners[1][0].index
    return first < last and not any(
        first < line.index < last and not line.substituted and _rebinds(line.text, name)
        for line in scanned
    )


def _stable_after(run: str, name: str, declaration: str) -> bool:
    _, separator, suffix = run.partition(declaration)
    return bool(separator) and not any(
        _rebinds(_strip_shell_comment(line).strip(), name)
        for line in suffix.splitlines()[1:]
    )


def _rebinds(text: str, name: str) -> bool:
    return bool(
        re.match(rf"^(?:export\s+)?{name}=", text)
        or re.match(rf"^unset(?:\s+-v)?\s+{name}(?:\s|$)", text)
    )


def _build_is_active(context: GuardContext) -> bool:
    build = context.build
    step = context.signing_step
    return (
        build is not None
        and step is not None
        and "preflight" in build.needs
        and not _has_condition(build)
        and not _has_condition(step)
        and not _continues_on_error(build)
        and not _continues_on_error(step)
    )


def _active_root_step(
    workflow: Workflow, job: Job | None, step: Step | None
) -> bool:
    if job is None or step is None:
        return False
    if (
        _has_condition(job)
        or _has_condition(step)
        or _continues_on_error(job)
        or _continues_on_error(step)
    ):
        return False
    context: dict[str, Any] = {}
    for raw in (workflow.raw, job.raw):
        defaults = raw.get("defaults", {})
        if not isinstance(defaults, dict):
            return False
        run = defaults.get("run", {})
        if not isinstance(run, dict):
            return False
        context.update(run)
    context.update(
        {key: step.raw[key] for key in ("shell", "working-directory") if key in step.raw}
    )
    return context.get("shell", "bash") == "bash" and context.get("working-directory", ".") == "."


def _has_condition(value: Job | Step) -> bool:
    return bool(str(value.raw.get("if", "")).strip())


def _continues_on_error(value: Job | Step) -> bool:
    raw = value.raw.get("continue-on-error")
    return raw is not None and str(raw).strip().lower() != "false"


def _ordered_positions(lines: list[str], required: tuple[str, ...]) -> list[int]:
    positions: list[int] = []
    start = 0
    for command in required:
        try:
            position = lines.index(command, start)
        except ValueError:
            return []
        positions.append(position)
        start = position + 1
    return positions


def _prefix_before(run: str, terminal: str) -> str:
    matches = [index for index, line in enumerate(run.splitlines()) if line.strip() == terminal]
    if len(matches) != 1:
        return ""
    return "\n".join(run.splitlines()[: matches[0]]) + "\n"


def _prefix_through(run: str, terminal: str) -> str:
    matches = [index for index, line in enumerate(run.splitlines()) if line.strip() == terminal]
    if len(matches) != 1:
        return ""
    return "\n".join(run.splitlines()[: matches[0] + 1]) + "\n"


def _top_level_lines(run: str) -> list[str]:
    scanned = _scan_shell(run)
    if scanned is None:
        return []
    return [
        line.text
        for line in scanned
        if not line.controls and not line.substituted
    ]


def _top_level_heredocs(run: str) -> list[tuple[str, str, str]]:
    scanned = _scan_shell(run)
    if scanned is None:
        return []
    return [
        (line.text, line.heredoc_marker, line.heredoc_body)
        for line in scanned
        if not line.controls
        and not line.substituted
        and line.heredoc_marker
    ]


def _function_body(run: str, name: str) -> list[str]:
    scanned = _scan_shell(run)
    if scanned is None:
        return []
    opening = f"{name}() {{"
    owners = [
        line
        for line in scanned
        if line.text == opening and not line.controls and not line.substituted
    ]
    if len(owners) != 1:
        return []
    return [
        line.text
        for line in scanned
        if line.index > owners[0].index
        and line.controls == ("brace",)
        and not line.substituted
        and line.text != "}"
    ]


def _scan_shell(run: str) -> list[ShellLine] | None:
    raw_lines = run.splitlines()
    scanned: list[ShellLine] = []
    controls: list[str] = []
    substitutions: list[tuple[str, int]] = []
    quote = ""
    index = 0
    while index < len(raw_lines):
        raw = raw_lines[index]
        text = _strip_shell_comment(raw).strip()
        was_substituted = bool(substitutions or quote)
        quote = release_workflow._update_substitution_scope(raw, substitutions, quote)
        marker = _heredoc_marker(text)
        body = ""
        if marker:
            end = index + 1
            while end < len(raw_lines) and raw_lines[end].strip() != marker:
                end += 1
            if end >= len(raw_lines):
                return None
            body = "\n".join(raw_lines[index + 1 : end]) + "\n"
        if text:
            scanned.append(
                ShellLine(index, text, tuple(controls), bool(substitutions or quote), marker, body)
            )
        if not marker and not was_substituted and not substitutions and not quote:
            if not release_workflow._update_control_stack(text, controls):
                return None
        index = end + 1 if marker else index + 1
    return scanned if not controls and not substitutions and not quote else None


def _heredoc_marker(text: str) -> str:
    match = re.search(r"<<-?\s*['\"]?([A-Za-z_][A-Za-z0-9_]*)['\"]?", text)
    return match.group(1) if match is not None else ""


def _strip_shell_comment(text: str) -> str:
    quote = ""
    escaped = False
    for index, char in enumerate(text):
        if escaped:
            escaped = False
            continue
        if quote == '"' and char == "\\":
            escaped = True
            continue
        if char in {"'", '"'}:
            quote = "" if quote == char else char if not quote else quote
            continue
        if char == "#" and not quote and (index == 0 or text[index - 1].isspace()):
            return text[:index]
    return text
