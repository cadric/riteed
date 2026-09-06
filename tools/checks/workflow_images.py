from __future__ import annotations

import re
import shlex
from typing import Any

from tools.checks import foundation
from tools.checks._workflow_parser import Workflow


POLICY_FILE = "policy/release.policy.json"
PINNING_POLICY = "pin_to_sha256_digest"
PINNED_IMAGE_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:/-]*@sha256:[0-9a-f]{64}$")
ASSIGNMENT_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=.*$", re.DOTALL)
STATIC_DURATION_RE = re.compile(r"^(?:[0-9]+(?:\.[0-9]+)?[smhd]?|inf)$")
CONTROL_CHARS = frozenset(";&|()\n")

TIMEOUT_FLAGS = {"--foreground", "--preserve-status", "--verbose", "-v"}
TIMEOUT_VALUE_OPTIONS = {"--kill-after", "--signal", "-k", "-s"}
DOCKER_GLOBAL_FLAGS = {"--debug", "--tls", "--tlsverify", "-D"}
DOCKER_GLOBAL_VALUE_OPTIONS = {
    "--config",
    "--context",
    "--host",
    "--log-level",
    "--tlscacert",
    "--tlscert",
    "--tlskey",
    "-H",
    "-l",
}
PULL_FLAGS = {"--all-tags", "--disable-content-trust", "--quiet", "-a", "-q"}
PULL_VALUE_OPTIONS = {"--platform"}
RUN_FLAGS = {
    "--detach",
    "--init",
    "--interactive",
    "--privileged",
    "--read-only",
    "--rm",
    "--tty",
    "-d",
    "-i",
    "-t",
}
RUN_VALUE_OPTIONS = {
    "--entrypoint",
    "--env",
    "--env-file",
    "--mount",
    "--name",
    "--network",
    "--platform",
    "--pull",
    "--user",
    "--volume",
    "--workdir",
    "-e",
    "-u",
    "-v",
    "-w",
}
DECOY_COMMANDS = {"echo", "printf"}
DOCKER_EXECUTABLES = {"docker", "/usr/bin/docker"}
DOCKER_EXECUTABLE_RE = re.compile(
    r"(?<![A-Za-z0-9_.-])(?:/[A-Za-z0-9._/-]*/)?docker(?=$|[\s;&|()<>`'\"\\])"
)
DATA_HEREDOC_RE = re.compile(
    r"^[ \t]*(?:cat|tee|/bin/cat|/usr/bin/cat|/usr/bin/tee)[ \t]+"
    r"<<-?[ \t]*(?:'[^'\r\n]+'|\"[^\"\r\n]+\")[ \t]*(?:#[^\r\n]*)?\r?\n?$"
)


def check_workflow_images(
    policy: dict[str, Any],
    workflows: list[Workflow | None],
    errors: list[str],
) -> None:
    configured = (
        policy.get("github_actions_release_safety", {})
        .get("pinning", {})
        .get("release_critical_container_images")
    )
    if configured != PINNING_POLICY:
        foundation.add(
            errors,
            f"{POLICY_FILE}: release_critical_container_images must be {PINNING_POLICY}",
        )
    for workflow in workflows:
        if workflow is None:
            continue
        for job in workflow.jobs.values():
            _check_job_container(workflow, job.job_id, job.raw.get("container"), errors)
            for step in job.steps:
                if step.run:
                    _check_run_step(workflow, job.job_id, step.name, step.run, errors)


def _check_job_container(workflow: Workflow, job_id: str, container: Any, errors: list[str]) -> None:
    if container is None:
        return
    image: Any
    if isinstance(container, str):
        image = container
    elif isinstance(container, dict):
        image = container.get("image")
    else:
        foundation.add(errors, f"{workflow.label}: job {job_id} container must declare a static image")
        return
    if not isinstance(image, str) or not image.strip():
        foundation.add(errors, f"{workflow.label}: job {job_id} container must declare a static image")
        return
    _require_pinned_image(workflow.label, f"job {job_id} container image", image.strip(), errors)


def _check_run_step(workflow: Workflow, job_id: str, step_name: str, run: str, errors: list[str]) -> None:
    label = f"{workflow.label}: job {job_id} step {step_name or '<unnamed>'}"
    script, heredoc_error = _without_heredoc_bodies(run)
    if heredoc_error:
        foundation.add(errors, f"{label}: cannot determine Docker image safely: {heredoc_error}")
        return
    try:
        commands = _shell_commands(script)
    except ValueError as exc:
        if "docker" in script:
            foundation.add(errors, f"{label}: cannot determine Docker image safely: {exc}")
        return
    if any(_strip_assignments(command) == ["$"] for command in commands) and any(
        _contains_docker_invocation(command) for command in commands
    ):
        foundation.add(errors, f"{label}: cannot determine Docker image safely: unsupported dynamic Docker wrapper")
        return
    for command in commands:
        _check_command(label, command, errors)


def _shell_commands(script: str) -> list[list[str]]:
    normalized = re.sub(r"\\[ \t]*\n[ \t]*", " ", script)
    lexer = shlex.shlex(normalized, posix=True, punctuation_chars=";&|()\n")
    lexer.whitespace = " \t\r"
    lexer.whitespace_split = True
    lexer.commenters = "#"
    commands: list[list[str]] = []
    current: list[str] = []
    for token in lexer:
        if token and all(char in CONTROL_CHARS for char in token):
            if current:
                commands.append(current)
                current = []
        else:
            current.append(token)
    if current:
        commands.append(current)
    return commands


def _check_command(label: str, command: list[str], errors: list[str]) -> None:
    stripped = _strip_assignments(command)
    if not stripped:
        return
    if stripped[0] in DECOY_COMMANDS:
        if _contains_executable_docker_substitution(stripped[1:]):
            foundation.add(errors, f"{label}: cannot determine Docker image safely: executable command substitution")
        return
    if stripped[0] == "timeout":
        docker = _after_timeout(stripped)
        if docker is None:
            if _contains_docker_invocation(stripped) or _contains_dynamic_pull_or_run(stripped):
                foundation.add(errors, f"{label}: cannot determine Docker image safely: unsupported timeout command")
            return
    elif stripped[0] in DOCKER_EXECUTABLES:
        docker = stripped
    else:
        if _contains_docker_invocation(stripped) or _contains_dynamic_pull_or_run(stripped):
            foundation.add(errors, f"{label}: cannot determine Docker image safely: unsupported Docker wrapper")
        return
    parsed = _docker_image_operand(docker)
    if parsed is None:
        return
    subcommand, image, problem = parsed
    if problem:
        foundation.add(errors, f"{label}: cannot determine Docker image safely: {problem}")
        return
    _require_pinned_image(label, f"docker {subcommand} image", image, errors)


def _strip_assignments(command: list[str]) -> list[str]:
    index = 0
    while index < len(command) and ASSIGNMENT_RE.fullmatch(command[index]):
        index += 1
    return command[index:]


def _contains_docker_invocation(command: list[str]) -> bool:
    joined = " ".join(command)
    return DOCKER_EXECUTABLE_RE.search(joined) is not None


def _contains_executable_docker_substitution(command: list[str]) -> bool:
    joined = " ".join(command)
    return ("$(" in joined or "`" in joined) and DOCKER_EXECUTABLE_RE.search(joined) is not None


def _contains_dynamic_pull_or_run(command: list[str]) -> bool:
    return any(
        any(char in command[index] for char in "$`{}") and command[index + 1] in {"pull", "run"}
        for index in range(len(command) - 1)
    )


def _after_timeout(command: list[str]) -> list[str] | None:
    index = 1
    while index < len(command) and command[index].startswith("-"):
        next_index = _consume_option(command, index, TIMEOUT_FLAGS, TIMEOUT_VALUE_OPTIONS)
        if next_index is None:
            return None
        index = next_index
    if index >= len(command) or STATIC_DURATION_RE.fullmatch(command[index]) is None:
        return None
    index += 1
    if index >= len(command) or command[index] not in DOCKER_EXECUTABLES:
        return None
    return command[index:]


def _docker_image_operand(command: list[str]) -> tuple[str, str, str] | None:
    index = 1
    while index < len(command) and command[index].startswith("-"):
        next_index = _consume_option(command, index, DOCKER_GLOBAL_FLAGS, DOCKER_GLOBAL_VALUE_OPTIONS)
        if next_index is None:
            return "command", "", f"unsupported Docker option {command[index]}"
        index = next_index
    if index >= len(command) or command[index] not in {"pull", "run"}:
        if index < len(command) and any(char in command[index] for char in "$`{}"):
            return "command", "", "dynamic Docker subcommand"
        if any(token in {"pull", "run"} for token in command[index:]):
            return "command", "", "unsupported Docker command shape"
        return None
    subcommand = command[index]
    index += 1
    flags = PULL_FLAGS if subcommand == "pull" else RUN_FLAGS
    value_options = PULL_VALUE_OPTIONS if subcommand == "pull" else RUN_VALUE_OPTIONS
    while index < len(command) and command[index].startswith("-"):
        next_index = _consume_option(command, index, flags, value_options)
        if next_index is None:
            return subcommand, "", f"unsupported docker {subcommand} option {command[index]}"
        index = next_index
    if index >= len(command):
        return subcommand, "", f"docker {subcommand} image is missing"
    return subcommand, command[index], ""


def _consume_option(
    command: list[str],
    index: int,
    flags: set[str],
    value_options: set[str],
) -> int | None:
    option = command[index]
    if option in flags:
        return index + 1
    if option in value_options:
        return index + 2 if index + 1 < len(command) else None
    if option.startswith("--") and "=" in option and option.split("=", maxsplit=1)[0] in value_options:
        return index + 1 if option.split("=", maxsplit=1)[1] else None
    return None


def _require_pinned_image(label: str, source: str, image: str, errors: list[str]) -> None:
    if PINNED_IMAGE_RE.fullmatch(image):
        return
    foundation.add(errors, f"{label}: {source} must use an exact sha256 digest")


def _without_heredoc_bodies(script: str) -> tuple[str, str]:
    output: list[str] = []
    pending: list[tuple[str, bool, bool]] = []
    quote = ""
    for line in script.splitlines(keepends=True):
        if pending:
            delimiter, strip_tabs, data_only = pending[0]
            candidate = line.rstrip("\r\n")
            if strip_tabs:
                candidate = candidate.lstrip("\t")
            if candidate == delimiter:
                pending.pop(0)
            elif not data_only and DOCKER_EXECUTABLE_RE.search(line):
                return "".join(output), "Docker command in executable heredoc"
            output.append("\n" if line.endswith("\n") else "")
            continue
        output.append(line)
        delimiters, quote, problem, _operator_index = _heredoc_delimiters(line, quote)
        if problem:
            return "".join(output), problem
        data_only = bool(delimiters) and DATA_HEREDOC_RE.fullmatch(line) is not None
        pending.extend((delimiter, strip_tabs, data_only) for delimiter, strip_tabs in delimiters)
    if pending:
        return "".join(output), "unterminated heredoc"
    return "".join(output), ""


def _heredoc_delimiters(
    line: str,
    quote: str,
) -> tuple[list[tuple[str, bool]], str, str, int | None]:
    delimiters: list[tuple[str, bool]] = []
    index = 0
    first_operator: int | None = None
    while index < len(line):
        char = line[index]
        if quote:
            if char == quote:
                quote = ""
            elif char == "\\" and quote == '"':
                index += 1
            index += 1
            continue
        if char in {"'", '"'}:
            quote = char
            index += 1
            continue
        if char == "#" and (index == 0 or line[index - 1].isspace()):
            break
        if line[index : index + 2] != "<<" or line[index : index + 3] == "<<<":
            index += 1
            continue
        if first_operator is None:
            first_operator = index
        index += 2
        strip_tabs = index < len(line) and line[index] == "-"
        if strip_tabs:
            index += 1
        while index < len(line) and line[index] in " \t":
            index += 1
        if index >= len(line) or line[index] in "\r\n":
            return [], quote, "heredoc delimiter is missing", first_operator
        delimiter_quote = line[index] if line[index] in {"'", '"'} else ""
        if delimiter_quote:
            index += 1
            start = index
            while index < len(line) and line[index] != delimiter_quote:
                index += 1
            if index >= len(line):
                return [], quote, "unterminated heredoc delimiter", first_operator
            delimiter = line[start:index]
            index += 1
        else:
            start = index
            while index < len(line) and not line[index].isspace() and line[index] not in ";&|<>()":
                index += 1
            delimiter = line[start:index]
        if not delimiter or any(char in delimiter for char in "$`\\"):
            return [], quote, "dynamic heredoc delimiter", first_operator
        delimiters.append((delimiter, strip_tabs))
    return delimiters, quote, "", first_operator
