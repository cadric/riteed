from __future__ import annotations

import re
import shlex
import tomllib
from pathlib import Path
from typing import Any

from tools.checks import foundation
from tools.checks._workflow_parser import WorkflowParseError, parse_workflow
from tools.validation_tooling import normalize_path, read_text


FUZZ_LOOP_RE = re.compile(r"^\s*for\s+target\s+in\s+(.+?)\s*;\s*do\s*$")
FUZZ_CALL_RE = re.compile(
    r'^\s*\(cd\s+fuzz\s+&&\s+cargo\s+\+nightly\s+fuzz\s+run\s+["\']?\$target["\']?'
    r"(?:\s+--\s+[^)]*)?\)\s*$"
)
FUZZ_GUARD = "if [ -d fuzz ]; then"


def check_target_contracts(repo: Path, policy: dict[str, Any], errors: list[str]) -> None:
    config = policy.get("fuzz_targets", {})
    workspace = repo / str(config.get("workspace", "app/fuzz"))
    targets = [str(item) for item in config.get("targets_required", [])]
    _check_cargo_bins(repo, workspace, targets, errors)
    if "page_text_decode" in targets:
        seed_config = config.get("seed_contract", {}).get("page_text_decode", {})
        _check_page_text_seeds(repo, workspace / "corpus" / "page_text_decode", seed_config, errors)
    _check_ci_execution(repo, policy, targets, errors)


def _check_cargo_bins(repo: Path, workspace: Path, targets: list[str], errors: list[str]) -> None:
    manifest = workspace / "Cargo.toml"
    try:
        with manifest.open("rb") as handle:
            data = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as exc:
        foundation.add(errors, f"{manifest.relative_to(repo)}: unable to parse fuzz Cargo manifest ({exc})")
        return
    bins = data.get("bin", [])
    if not isinstance(bins, list):
        foundation.add(errors, f"{manifest.relative_to(repo)}: [[bin]] entries must be an array")
        return
    for target in targets:
        expected = f"fuzz_targets/{target}.rs"
        exact = [
            entry
            for entry in bins
            if isinstance(entry, dict)
            and str(entry.get("name", "")) == target
            and normalize_path(str(entry.get("path", ""))) == expected
        ]
        if len(exact) != 1:
            foundation.add(
                errors,
                f"{manifest.relative_to(repo)}: required fuzz target {target} must have exactly one "
                f"Cargo [[bin]] registration at {expected}",
            )


def _check_page_text_seeds(repo: Path, corpus: Path, config: Any, errors: list[str]) -> None:
    label = "fuzz_targets.seed_contract.page_text_decode"
    if not isinstance(config, dict):
        foundation.add(errors, f"{label} must be an object")
        return
    prefix_bytes = _positive_int(config.get("offset_prefix_bytes"))
    scan_limit = _positive_int(config.get("max_seed_scan_bytes"))
    near_max_distance = _positive_int(config.get("near_max_distance"))
    byte_order = str(config.get("byte_order", ""))
    required = config.get("required_seed_classes", [])
    if prefix_bytes is None:
        foundation.add(errors, f"{label}.offset_prefix_bytes must be a positive integer")
    if scan_limit is None:
        foundation.add(errors, f"{label}.max_seed_scan_bytes must be a positive integer")
    if near_max_distance is None:
        foundation.add(errors, f"{label}.near_max_distance must be a positive integer")
    if byte_order != "little":
        foundation.add(errors, f"{label}.byte_order must be little")
    if not isinstance(required, list) or not required or not all(isinstance(item, str) and item for item in required):
        foundation.add(errors, f"{label}.required_seed_classes must be a non-empty string array")
        return
    supported = {
        "offset_zero",
        "offset_near_max",
        "offset_max",
        "empty_payload",
        "incomplete_utf8",
        "continuation_only",
    }
    unknown = sorted(set(required) - supported)
    if unknown:
        foundation.add(errors, f"{label}.required_seed_classes contains unsupported values: {', '.join(unknown)}")
    if prefix_bytes is None or scan_limit is None or near_max_distance is None or byte_order != "little":
        return

    found: set[str] = set()
    for path in sorted(corpus.rglob("*")) if corpus.exists() else []:
        if not path.is_file():
            continue
        rel = path.relative_to(repo)
        try:
            size = path.stat().st_size
        except OSError as exc:
            foundation.add(errors, f"{rel}: unable to stat fuzz seed ({exc})")
            continue
        if size > scan_limit:
            foundation.add(errors, f"{rel}: fuzz seed exceeds policy scan limit of {scan_limit} bytes")
            continue
        try:
            data = path.read_bytes()
        except OSError as exc:
            foundation.add(errors, f"{rel}: unable to read fuzz seed ({exc})")
            continue
        found.update(_page_text_seed_classes(data, prefix_bytes, near_max_distance))
    missing = [item for item in required if item not in found]
    if missing:
        foundation.add(errors, f"page_text_decode corpus missing required seed classes: {', '.join(missing)}")


def _page_text_seed_classes(data: bytes, prefix_bytes: int, near_max_distance: int) -> set[str]:
    if len(data) < prefix_bytes:
        return set()
    offset = int.from_bytes(data[:prefix_bytes], "little")
    payload = data[prefix_bytes:]
    maximum = (1 << (prefix_bytes * 8)) - 1
    found = set()
    if offset == 0:
        found.add("offset_zero")
    if offset == maximum:
        found.add("offset_max")
    if maximum - min(maximum, near_max_distance) <= offset < maximum:
        found.add("offset_near_max")
    if not payload:
        found.add("empty_payload")
    if _ends_with_incomplete_utf8(payload):
        found.add("incomplete_utf8")
    if payload and all(0x80 <= byte <= 0xBF for byte in payload):
        found.add("continuation_only")
    return found


def _ends_with_incomplete_utf8(payload: bytes) -> bool:
    expected_lengths = {**{byte: 2 for byte in range(0xC2, 0xE0)}, **{byte: 3 for byte in range(0xE0, 0xF0)}, **{byte: 4 for byte in range(0xF0, 0xF5)}}
    for start in range(max(0, len(payload) - 4), len(payload)):
        expected = expected_lengths.get(payload[start])
        trailing = payload[start:]
        if expected is not None and len(trailing) < expected and all(0x80 <= byte <= 0xBF for byte in trailing[1:]):
            return True
    return False


def _check_ci_execution(repo: Path, policy: dict[str, Any], targets: list[str], errors: list[str]) -> None:
    fuzz_config = policy.get("fuzz_targets", {})
    config = fuzz_config.get("ci_execution", {})
    label = "fuzz_targets.ci_execution"
    if not isinstance(config, dict):
        foundation.add(errors, f"{label} must be an object")
        return
    job_name = str(config.get("job", "")).strip()
    step_name = str(config.get("step", "")).strip()
    required_condition = str(config.get("required_condition", "")).strip()
    container_image_prefix = str(config.get("container_image_prefix", "")).strip()
    required_triggers = config.get("required_triggers", [])
    if (
        not job_name
        or not step_name
        or not required_condition
        or not container_image_prefix
        or not isinstance(required_triggers, list)
        or not required_triggers
        or not all(isinstance(item, str) and item for item in required_triggers)
    ):
        foundation.add(
            errors,
            f"{label} must declare job, step, required_condition, container_image_prefix, and required_triggers",
        )
        return
    workflow = repo / str(policy.get("ci_artifacts", {}).get("workflow", ".github/workflows/validate.yml"))
    if not workflow.exists():
        return
    raw_workflow = read_text(workflow)
    try:
        model = parse_workflow(raw_workflow, workflow.relative_to(repo).as_posix())
    except WorkflowParseError as exc:
        foundation.add(errors, str(exc))
        return
    _check_workflow_triggers(
        workflow.relative_to(repo).as_posix(),
        model.triggers,
        required_triggers,
        raw_workflow,
        errors,
    )
    job = model.jobs.get(job_name)
    if job is None:
        foundation.add(errors, f"{workflow.relative_to(repo)}: required CI fuzz job {job_name} is missing")
        return
    if str(job.raw.get("if", "")).strip() != required_condition:
        foundation.add(errors, f"{workflow.relative_to(repo)}: CI fuzz job must use the policy-owned event condition")
    if _continues_on_error(job.raw) or not _uses_supported_shell(model.raw, job.raw, {}):
        foundation.add(errors, f"{workflow.relative_to(repo)}: CI fuzz job must use fail-closed bash execution")
    steps = [step for step in job.steps if step.name == step_name]
    if len(steps) != 1:
        foundation.add(errors, f"{workflow.relative_to(repo)}: required CI fuzz step {step_name} is missing")
        return
    step = steps[0]
    if str(step.raw.get("if", "")).strip() or _continues_on_error(step.raw):
        foundation.add(errors, f"{workflow.relative_to(repo)}: CI fuzz step {step_name} must not be disabled or conditional")
        return
    if not _uses_supported_shell(model.raw, job.raw, step.raw):
        foundation.add(errors, f"{workflow.relative_to(repo)}: CI fuzz step must use the supported bash shell")
        return
    executed_targets = _executed_fuzz_targets(step.run, container_image_prefix)
    if executed_targets is None:
        foundation.add(errors, f"{workflow.relative_to(repo)}: CI fuzz loop must execute cargo-fuzz for its target variable")
        return
    missing_targets = [target for target in targets if target not in executed_targets]
    if missing_targets:
        foundation.add(errors, f"{workflow.relative_to(repo)}: CI fuzz loop missing required targets: {', '.join(missing_targets)}")


def _check_workflow_triggers(
    workflow: str,
    triggers: dict[str, Any],
    required: list[str],
    raw_workflow: str,
    errors: list[str],
) -> None:
    missing = [trigger for trigger in required if trigger not in triggers]
    if missing:
        foundation.add(errors, f"{workflow}: CI fuzz workflow missing required triggers: {', '.join(missing)}")
    if "schedule" not in required:
        return
    schedule = triggers.get("schedule")
    trigger_block = _raw_mapping_block(raw_workflow.splitlines(), 0, "on:")
    schedule_block = _raw_mapping_block(trigger_block or [], 2, "schedule:")
    quoted_crons = [
        match.groups()
        for line in schedule_block or []
        if (match := re.match(r'^    - cron:\s*(?:"([^"]+)"|\'([^\']+)\')\s*$', line)) is not None
    ]
    has_cron = isinstance(schedule, list) and any(
        isinstance(item, dict) and isinstance(item.get("cron"), str) and item["cron"].strip() for item in schedule
    )
    has_typed_cron = any((double_quoted or single_quoted).strip() for double_quoted, single_quoted in quoted_crons)
    if not has_cron or not has_typed_cron:
        foundation.add(errors, f"{workflow}: CI fuzz schedule trigger must include a non-empty cron entry")


def _raw_mapping_block(lines: list[str], indent: int, marker: str) -> list[str] | None:
    prefix = " " * indent
    for index, line in enumerate(lines):
        if line != f"{prefix}{marker}":
            continue
        end = index + 1
        while end < len(lines):
            candidate = lines[end]
            if candidate.strip() and len(candidate) - len(candidate.lstrip()) <= indent:
                break
            end += 1
        return lines[index + 1 : end]
    return None


def _executed_fuzz_targets(run: str, container_image_prefix: str) -> set[str] | None:
    lines = _supported_docker_script(run, container_image_prefix)
    if lines is None or any(_unsupported_script_construct(line) for line in lines):
        return None
    guard_indexes = [index for index, line in enumerate(lines) if line.strip() == FUZZ_GUARD]
    if len(guard_indexes) != 1:
        return None
    guard_start = guard_indexes[0]
    if _control_depth(lines[:guard_start]) != 0:
        return None
    guard_end = _matching_control_end(lines, guard_start, "if ", "fi")
    if guard_end is None or any(line.strip() for line in lines[guard_end + 1 :]):
        return None
    guarded = lines[guard_start + 1 : guard_end]
    if any(_unsupported_nested_control(line) for line in guarded):
        return None
    loops = [(index, FUZZ_LOOP_RE.match(line)) for index, line in enumerate(guarded)]
    loops = [(index, match) for index, match in loops if match is not None]
    if len(loops) != 1:
        return None
    loop_start, match = loops[0]
    loop_end = _matching_control_end(guarded, loop_start, "for ", "done")
    if loop_end is None or match is None:
        return None
    body = [line for line in guarded[loop_start + 1 : loop_end] if line.strip() and not line.lstrip().startswith("#")]
    if len(body) != 1 or FUZZ_CALL_RE.match(body[0]) is None:
        return None
    try:
        return set(shlex.split(match.group(1)))
    except ValueError:
        return None


def _supported_docker_script(run: str, container_image_prefix: str) -> list[str] | None:
    lines = run.splitlines()
    nonempty = [index for index, line in enumerate(lines) if line.strip()]
    if not nonempty or lines[nonempty[0]].strip() != "docker run --rm \\":
        return None
    bash_indexes = [index for index, line in enumerate(lines) if line.strip() == "bash -lc '"]
    if len(bash_indexes) != 1:
        return None
    bash_index = bash_indexes[0]
    header = lines[nonempty[0] : bash_index + 1]
    header[-1] = header[-1].rstrip().removesuffix("'").rstrip()
    normalized = re.sub(r"\\[ \t]*\n[ \t]*", " ", "\n".join(header))
    try:
        command = shlex.split(normalized)
    except ValueError:
        return None
    if not _supported_docker_command(command, container_image_prefix):
        return None
    if nonempty[-1] <= bash_index or lines[nonempty[-1]].strip() != "'":
        return None
    if any(line.strip() for line in lines[nonempty[-1] + 1 :]):
        return None
    return lines[bash_index + 1 : nonempty[-1]]


def _supported_docker_command(command: list[str], container_image_prefix: str) -> bool:
    if command[:3] != ["docker", "run", "--rm"] or len(command) < 6:
        return False
    if command[-2:] != ["bash", "-lc"] or not command[-3].startswith(container_image_prefix):
        return False
    options = command[3:-3]
    index = 0
    while index < len(options):
        option = options[index]
        if option == "--privileged":
            index += 1
        elif option in {"-e", "-v", "-w"} and index + 1 < len(options):
            index += 2
        else:
            return False
    return True


def _control_depth(lines: list[str]) -> int:
    depth = 0
    for line in lines:
        stripped = line.strip()
        if stripped.startswith(("if ", "for ", "while ", "until ", "case ")):
            depth += 1
        elif stripped in {"fi", "done", "esac"}:
            depth -= 1
        if depth < 0:
            return -1
    return depth


def _unsupported_script_construct(line: str) -> bool:
    stripped = line.strip()
    return "<<" in stripped or re.match(r"^[A-Za-z_][A-Za-z0-9_]*\s*\(\)\s*\{", stripped) is not None


def _matching_control_end(lines: list[str], start: int, opening: str, closing: str) -> int | None:
    depth = 0
    for index in range(start, len(lines)):
        stripped = lines[index].strip()
        if stripped.startswith(opening):
            depth += 1
        elif stripped == closing:
            depth -= 1
            if depth == 0:
                return index
    return None


def _unsupported_nested_control(line: str) -> bool:
    stripped = line.strip()
    return bool(
        stripped.startswith(("if ", "while ", "until ", "case ", "select "))
        or (stripped.startswith("for ") and FUZZ_LOOP_RE.match(line) is None)
        or "<<" in stripped
        or stripped in {"fi", "esac"}
    )


def _uses_supported_shell(workflow: dict[str, Any], job: dict[str, Any], step: dict[str, Any]) -> bool:
    context: dict[str, Any] = {}
    for raw in (workflow, job):
        defaults = raw.get("defaults", {})
        if defaults and (not isinstance(defaults, dict) or not isinstance(defaults.get("run", {}), dict)):
            return False
        if isinstance(defaults, dict):
            context.update(defaults.get("run", {}))
    context.update({key: step[key] for key in ("shell", "working-directory") if key in step})
    return context.get("shell", "bash") == "bash"


def _continues_on_error(raw: dict[str, Any]) -> bool:
    value = raw.get("continue-on-error")
    return value is not None and str(value).strip().lower() != "false"


def _positive_int(value: Any) -> int | None:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        return None
    return value
