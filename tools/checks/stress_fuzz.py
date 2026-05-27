from __future__ import annotations

import json
import datetime as dt
import re
from fnmatch import fnmatch
from pathlib import Path
from typing import Any

from tools.checks import foundation, remediation
from tools.validation_tooling import contract_root, normalize_path, read_text


POLICY_FILE = "policy/stress-fuzz.policy.json"
POLICY_ID = "policy/stress_fuzz.v1.json"
REGISTRY_FINDING = "RIT-AUD-015"
STRESS_FINDING = "RIT-AUD-008"
PARSER_MARKER_RE = re.compile(r"PARSER-BOUNDARY:\s*id=([A-Za-z0-9_-]+)")
GIT_STATUS_REPOS = {
    "stress/git-repos/generated/many-untracked",
    "stress/git-repos/generated/many-modified",
    "stress/git-repos/generated/conflicted",
    "stress/git-repos/generated/non-utf8-paths",
    "stress/git-repos/generated/submodule-and-symlink",
    "stress/git-repos/generated/index-lock-present",
    "stress/git-repos/generated/huge-status",
}


def check_stress_fuzz(root: Path, errors: list[str]) -> None:
    repo = contract_root(root)
    policy_path = repo / POLICY_FILE
    if not policy_path.exists():
        foundation.add(errors, f"Missing stress/fuzz policy file: {POLICY_FILE}")
        return
    policy = foundation.stress_fuzz_policy(root)
    if policy.get("$id") != POLICY_ID:
        foundation.add(errors, f"{POLICY_FILE} must have $id {POLICY_ID}")
    _check_policy_stack(root, errors)
    active = remediation.validate_planned_remediation(policy, POLICY_FILE, errors)
    registry_entries = _check_parser_registry(repo, policy, active, errors)
    _check_fuzz_targets(repo, policy, active, registry_entries, errors)
    _check_stress_scripts(repo, policy, active, registry_entries, errors)
    _check_runner_fidelity(repo, active, errors)
    _check_ci_generated_inputs(repo, policy, active, errors)
    _check_artifact_uploads(repo, policy, errors)
    _check_lockfile_delegation(policy, errors)


def _check_policy_stack(root: Path, errors: list[str]) -> None:
    bundle = foundation.policy_bundle(root)
    validation = foundation.validation_policy(root)
    files = {str(item.get("file")): str(item.get("$id")) for item in bundle.get("bundle_contains", [])}
    if files.get("stress-fuzz.policy.json") != POLICY_ID:
        foundation.add(errors, "Bundle must include stress-fuzz.policy.json with policy/stress_fuzz.v1.json")
    if POLICY_ID not in [str(item) for item in bundle.get("overlaps_with", [])]:
        foundation.add(errors, "Bundle overlaps_with must include policy/stress_fuzz.v1.json")
    if POLICY_FILE not in [str(item) for item in validation.get("required_policy_files", [])]:
        foundation.add(errors, "validation-tooling required_policy_files must include policy/stress-fuzz.policy.json")
    if POLICY_ID not in [str(item) for item in validation.get("overlaps_with", [])]:
        foundation.add(errors, "validation-tooling overlaps_with must include policy/stress_fuzz.v1.json")


def _check_parser_registry(
    repo: Path,
    policy: dict[str, Any],
    active: set[str],
    errors: list[str],
) -> list[dict[str, Any]]:
    cfg = policy.get("parser_boundary_registry", {})
    registry_paths = _glob_paths(repo, [str(item) for item in cfg.get("globs", [])])
    if not registry_paths:
        if REGISTRY_FINDING not in active:
            foundation.add(errors, "Parser-boundary registry is required under app/build-aux/validation/")
        return []
    entries = _registry_entries(repo, registry_paths, errors)
    valid_entries: list[dict[str, Any]] = []
    seen: set[str] = set()
    entries_by_id: dict[str, dict[str, Any]] = {}
    required_fields = [str(item) for item in cfg.get("entry_shape", {}).get("required_fields", [])]
    coverage_fields = [str(item) for item in cfg.get("coverage_entry_required_fields", [])]
    exception_fields = [str(item) for item in cfg.get("reviewed_exception_required_fields", [])]
    for entry, label in entries:
        if not isinstance(entry, dict):
            foundation.add(errors, f"{label}: registry entry must be an object")
            continue
        missing = [field for field in required_fields if field not in entry]
        if missing:
            foundation.add(errors, f"{label}: missing required fields: {', '.join(missing)}")
        _check_registry_entry_shape(entry, label, coverage_fields, exception_fields, errors)
        entry_id = str(entry.get("id", "")).strip()
        if not entry_id:
            continue
        if entry_id in seen:
            foundation.add(errors, f"{label}: duplicate parser-boundary id {entry_id}")
        seen.add(entry_id)
        entries_by_id[entry_id] = entry
        _check_registry_paths(repo, entry, label, active, errors)
        valid_entries.append(entry)
    for boundary_id in [str(item) for item in cfg.get("minimum_boundary_ids", [])]:
        if boundary_id not in seen:
            foundation.add(errors, f"Parser-boundary registry missing minimum id {boundary_id}")
    if REGISTRY_FINDING not in active:
        _check_parser_boundary_markers(repo, entries_by_id, errors)
    return valid_entries


def _check_registry_entry_shape(
    entry: dict[str, Any],
    label: str,
    coverage_fields: list[str],
    exception_fields: list[str],
    errors: list[str],
) -> None:
    if not str(entry.get("kind", "")).strip():
        foundation.add(errors, f"{label}: kind must be non-empty")
    for field in ("entrypoints", "gaps", "reviewed_exceptions"):
        if not isinstance(entry.get(field), list):
            foundation.add(errors, f"{label}: {field} must be an array")
    if not str(entry.get("real_input_shape", "")).strip():
        foundation.add(errors, f"{label}: real_input_shape must be non-empty")
    for index, coverage in enumerate(entry.get("coverage", []) if isinstance(entry.get("coverage"), list) else []):
        if not isinstance(coverage, dict):
            continue
        missing = [field for field in coverage_fields if not str(coverage.get(field, "")).strip()]
        if missing:
            foundation.add(errors, f"{label}: coverage[{index}] missing required fields: {', '.join(missing)}")
    for index, exception in enumerate(
        entry.get("reviewed_exceptions", []) if isinstance(entry.get("reviewed_exceptions"), list) else []
    ):
        if not isinstance(exception, dict):
            foundation.add(errors, f"{label}: reviewed_exceptions[{index}] must be an object")
            continue
        missing = [field for field in exception_fields if not str(exception.get(field, "")).strip()]
        if missing:
            foundation.add(errors, f"{label}: reviewed_exceptions[{index}] missing required fields: {', '.join(missing)}")
        _require_date(exception.get("last_reviewed"), label, f"reviewed_exceptions[{index}].last_reviewed", errors)


def _registry_entries(repo: Path, paths: list[Path], errors: list[str]) -> list[tuple[Any, str]]:
    entries: list[tuple[Any, str]] = []
    for path in paths:
        try:
            data = json.loads(read_text(path))
        except json.JSONDecodeError as exc:
            foundation.add(errors, f"{path.relative_to(repo)}: invalid JSON ({exc})")
            continue
        raw_entries = data.get("boundaries", data.get("entries", data)) if isinstance(data, dict) else data
        if not isinstance(raw_entries, list):
            foundation.add(errors, f"{path.relative_to(repo)}: expected boundary list or object with boundaries")
            continue
        for index, entry in enumerate(raw_entries):
            entries.append((entry, f"{path.relative_to(repo)}[{index}]"))
    return entries


def _check_registry_paths(repo: Path, entry: dict[str, Any], label: str, active: set[str], errors: list[str]) -> None:
    _require_date(entry.get("last_reviewed"), label, "last_reviewed", errors)
    for field in ("source_paths", "coverage"):
        if not isinstance(entry.get(field), list):
            foundation.add(errors, f"{label}: {field} must be an array")
    for source in entry.get("source_paths", []) if isinstance(entry.get("source_paths"), list) else []:
        if not _path_or_glob_exists(repo, str(source)):
            foundation.add(errors, f"{label}: source_path {source} does not match a repo file")
    for index, coverage in enumerate(entry.get("coverage", []) if isinstance(entry.get("coverage"), list) else []):
        if not isinstance(coverage, dict):
            foundation.add(errors, f"{label}: coverage[{index}] must be an object")
            continue
        for field in ("type", "path", "target", "input_shape_asserted"):
            if not str(coverage.get(field, "")).strip():
                foundation.add(errors, f"{label}: coverage[{index}] missing {field}")
        cov_path = str(coverage.get("path", "")).strip()
        if cov_path and not _path_or_glob_exists(repo, cov_path):
            foundation.add(errors, f"{label}: coverage path {cov_path} does not match a repo file")
    gaps = entry.get("gaps", [])
    exceptions = entry.get("reviewed_exceptions", [])
    if gaps and not exceptions and not _gaps_have_planned_remediation(gaps, active):
        foundation.add(errors, f"{label}: non-empty gaps require reviewed_exceptions or planned remediation")


def _check_fuzz_targets(
    repo: Path,
    policy: dict[str, Any],
    active: set[str],
    registry_entries: list[dict[str, Any]],
    errors: list[str],
) -> None:
    cfg = policy.get("fuzz_targets", {})
    workspace = repo / str(cfg.get("workspace", "app/fuzz"))
    for target in [str(item) for item in cfg.get("targets_required", [])]:
        target_file = workspace / "fuzz_targets" / f"{target}.rs"
        if not target_file.exists():
            foundation.add(errors, f"{target_file.relative_to(repo)}: required fuzz target is missing")
        corpus = workspace / "corpus" / target
        if not corpus.exists() or not any(path.is_file() for path in corpus.rglob("*")):
            foundation.add(errors, f"{corpus.relative_to(repo)}: required fuzz seed corpus is missing")
        if REGISTRY_FINDING not in active and not _registry_covers(
            registry_entries,
            "fuzz",
            target,
            target_file.relative_to(repo).as_posix(),
        ):
            foundation.add(errors, f"Parser-boundary registry coverage missing required fuzz target {target}")
    git_corpus = workspace / "corpus" / "git_status_parse"
    has_git_seed_shape = _corpus_has_git_status_shape(repo, git_corpus, errors) if git_corpus.exists() else False
    if not has_git_seed_shape and REGISTRY_FINDING not in active:
        foundation.add(
            errors,
            "git_status_parse corpus must include valid NUL-delimited porcelain v2 -z seeds "
            "covering ordinary, unmerged, control-character, and non-UTF-8 paths",
        )


def _check_stress_scripts(
    repo: Path,
    policy: dict[str, Any],
    active: set[str],
    registry_entries: list[dict[str, Any]],
    errors: list[str],
) -> None:
    cfg = policy.get("stress_scripts", {})
    required_fields = [str(item) for item in cfg.get("script_schema", {}).get("required_fields", [])]
    for rel in [str(item) for item in cfg.get("required_scripts", [])]:
        path = repo / rel
        if not path.exists():
            foundation.add(errors, f"{rel}: required stress script is missing")
            continue
        try:
            data = json.loads(read_text(path))
        except json.JSONDecodeError as exc:
            foundation.add(errors, f"{rel}: invalid JSON ({exc})")
            continue
        missing = [field for field in required_fields if field not in data]
        if missing and STRESS_FINDING not in active:
            foundation.add(errors, f"{rel}: stress script missing required fields: {', '.join(missing)}")
        if STRESS_FINDING not in active:
            if not _registry_covers(registry_entries, "stress", str(data.get("flow", "")), rel):
                foundation.add(errors, f"Parser-boundary registry coverage missing required stress script {rel}")
            _check_script_schema_values(rel, data, errors)
            _check_script_boundary_fidelity(rel, data, cfg, errors)


def _check_script_schema_values(rel: str, data: dict[str, Any], errors: list[str]) -> None:
    for field in ("fixtures", "actions", "assertions"):
        if not isinstance(data.get(field), list) or not data.get(field):
            foundation.add(errors, f"{rel}: {field} must be a non-empty array")
    artifact_dir = str(data.get("artifact_dir", "")).strip()
    if _safe_stress_artifact_dir(artifact_dir) is None:
        foundation.add(errors, f"{rel}: artifact_dir must be under stress/artifacts/")
    _check_script_object_array(rel, data, "fixtures", ("role", "path"), errors)
    _check_script_object_array(rel, data, "actions", ("type",), errors)
    _check_script_object_array(rel, data, "assertions", ("type",), errors)


def _check_runner_fidelity(repo: Path, active: set[str], errors: list[str]) -> None:
    runner = repo / "app" / "src" / "bin" / "riteed_stress.rs"
    if not runner.exists():
        foundation.add(errors, "app/src/bin/riteed_stress.rs: stress runner is required")
        return
    text = read_text(runner)
    temp_only = "std::env::temp_dir()" in text and "stress/corpus/generated" not in text and "stress/git-repos/generated" not in text
    if temp_only and STRESS_FINDING not in active:
        foundation.add(errors, "riteed-stress must consume declared/generated fixtures for named boundary flows")


def _check_script_boundary_fidelity(rel: str, data: dict[str, Any], cfg: dict[str, Any], errors: list[str]) -> None:
    flow = str(data.get("flow", "")).replace("-", "_")
    requirements = cfg.get("boundary_fidelity", {}).get(flow, {})
    if not isinstance(requirements, dict):
        return
    fixtures = _path_values(data.get("fixtures"))
    actions = _typed_objects(data.get("actions"))
    assertions = _typed_objects(data.get("assertions"))
    checks = {
        "must_open_generated_corpus_file": lambda: _has_action(actions, "open", "stress/corpus/generated/open-save-search.txt"),
        "must_perform_search_action": lambda: _has_action_type(actions, "search"),
        "must_perform_save_or_save_as_boundary": lambda: _has_action_type(actions, "save") or _has_action_type(actions, "save-as"),
        "must_assert_document_or_search_state": lambda: _has_assertion_type(assertions, "document-state")
        and _has_assertion_type(assertions, "search-state"),
        "must_start_compare_workflow": lambda: _has_compare_workflow(actions, fixtures),
        "must_assert_compare_pane_or_diff_state": lambda: _has_assertion_type(assertions, "compare-pane-diff-state"),
        "must_use_input_files_from_declared_fixtures": lambda: {
            "stress/corpus/generated/compare-reference.txt",
            "stress/corpus/generated/compare-current.txt",
        }.issubset(fixtures),
        "must_open_generated_markdown_corpus": lambda: _has_action(actions, "open", "stress/corpus/generated/markdown-stress.md"),
        "must_toggle_or_render_preview_boundary": lambda: _has_action_type(actions, "toggle-preview-render"),
        "must_assert_preview_or_fallback_state": lambda: _has_assertion_type(assertions, "preview-or-fallback-state"),
        "must_open_generated_git_repos": lambda: all(_has_any_action_for_path(actions, path) for path in GIT_STATUS_REPOS),
        "must_include_many_untracked_many_modified_conflicted_non_utf8_submodule_and_lock_cases": lambda: GIT_STATUS_REPOS.issubset(fixtures),
        "must_assert_source_control_or_degraded_state": lambda: _has_assertion_type(
            assertions,
            "source-control-or-degraded-state",
        ),
    }
    for key, enabled in requirements.items():
        if not enabled:
            continue
        checker = checks.get(str(key))
        if checker is None:
            continue
        if not checker():
            foundation.add(errors, f"{rel}: boundary fidelity requirement {key} is not executable in script")


def _check_ci_generated_inputs(repo: Path, policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
    workflow = repo / str(policy.get("ci_artifacts", {}).get("workflow", ".github/workflows/validate.yml"))
    if not workflow.exists():
        foundation.add(errors, f"{workflow.relative_to(repo)}: validation workflow is required")
        return
    text = read_text(workflow)
    for token in ("python3 stress/make_corpus.py", "stress/git-repos/make_repos.sh"):
        if token not in text and STRESS_FINDING not in active:
            foundation.add(errors, f"{workflow.relative_to(repo)}: scheduled stress must run {token}")
    for token in policy.get("generated_inputs", {}).get("generated_paths", []):
        if str(token) not in text and STRESS_FINDING not in active:
            foundation.add(errors, f"{workflow.relative_to(repo)}: scheduled stress must consume {token}")


def _check_artifact_uploads(repo: Path, policy: dict[str, Any], errors: list[str]) -> None:
    workflow = repo / str(policy.get("ci_artifacts", {}).get("workflow", ".github/workflows/validate.yml"))
    if not workflow.exists():
        return
    text = read_text(workflow)
    for rel in [str(item) for item in policy.get("ci_artifacts", {}).get("required_failure_artifact_paths", [])]:
        if rel not in text:
            foundation.add(errors, f"{workflow.relative_to(repo)}: failure artifact upload must include {rel}")


def _check_lockfile_delegation(policy: dict[str, Any], errors: list[str]) -> None:
    lockfile = policy.get("fuzz_targets", {}).get("lockfile_sync", {})
    if not lockfile.get("dependency_preflight_is_authoritative_for_version_comparison"):
        foundation.add(errors, f"{POLICY_FILE}: lockfile sync must delegate version comparison to dependency preflight")


def _check_parser_boundary_markers(repo: Path, entries: dict[str, dict[str, Any]], errors: list[str]) -> None:
    markers = _parser_boundary_markers(repo, errors)
    for boundary_id, entry in sorted(entries.items()):
        marked_paths = markers.get(boundary_id, [])
        if not marked_paths:
            foundation.add(errors, f"Parser-boundary id {boundary_id} must have PARSER-BOUNDARY marker in source_paths")
            continue
        sources = [normalize_path(str(item)) for item in entry.get("source_paths", [])]
        for path in marked_paths:
            if not any(_path_matches_pattern(path, source) for source in sources):
                foundation.add(errors, f"Parser-boundary marker {boundary_id} is outside registered source_paths")
    for boundary_id in sorted(markers):
        if boundary_id not in entries:
            foundation.add(errors, f"Parser-boundary marker {boundary_id} is not registered")


def _parser_boundary_markers(repo: Path, errors: list[str]) -> dict[str, list[str]]:
    markers: dict[str, list[str]] = {}
    for path in sorted(repo.glob("app/src/**/*.rs")):
        if not path.is_file():
            continue
        rel = path.relative_to(repo).as_posix()
        try:
            text = read_text(path)
        except SystemExit:
            foundation.add(errors, f"{rel}: unable to read parser-boundary marker source")
            continue
        for match in PARSER_MARKER_RE.finditer(text):
            markers.setdefault(match.group(1), []).append(rel)
    return markers


def _path_matches_pattern(path: str, pattern: str) -> bool:
    return path == pattern or fnmatch(path, pattern)


def _check_script_object_array(
    rel: str,
    data: dict[str, Any],
    field: str,
    required_fields: tuple[str, ...],
    errors: list[str],
) -> None:
    items = data.get(field)
    if not isinstance(items, list):
        return
    for index, item in enumerate(items):
        if not isinstance(item, dict):
            foundation.add(errors, f"{rel}: {field}[{index}] must be an object")
            continue
        missing = [name for name in required_fields if not str(item.get(name, "")).strip()]
        if missing:
            foundation.add(errors, f"{rel}: {field}[{index}] missing required fields: {', '.join(missing)}")


def _typed_objects(value: Any) -> list[dict[str, Any]]:
    return [item for item in value if isinstance(item, dict) and str(item.get("type", "")).strip()] if isinstance(value, list) else []


def _path_values(value: Any) -> set[str]:
    paths: set[str] = set()
    if not isinstance(value, list):
        return paths
    for item in value:
        if isinstance(item, dict):
            path = normalize_path(str(item.get("path", "")).strip())
            if path:
                paths.add(path)
    return paths


def _has_action(actions: list[dict[str, Any]], action_type: str, path: str) -> bool:
    return any(
        str(action.get("type")) == action_type and normalize_path(str(action.get("path", ""))) == path
        for action in actions
    )


def _has_action_type(actions: list[dict[str, Any]], action_type: str) -> bool:
    return any(str(action.get("type")) == action_type for action in actions)


def _has_assertion_type(assertions: list[dict[str, Any]], assertion_type: str) -> bool:
    return any(str(assertion.get("type")) == assertion_type for assertion in assertions)


def _has_any_action_for_path(actions: list[dict[str, Any]], path: str) -> bool:
    return any(
        str(action.get("type")) in {"open-source-control", "refresh-source-control"}
        and normalize_path(str(action.get("path", ""))) == path
        for action in actions
    )


def _has_compare_workflow(actions: list[dict[str, Any]], fixtures: set[str]) -> bool:
    for action in actions:
        if str(action.get("type")) != "start-compare-workflow":
            continue
        reference = normalize_path(str(action.get("reference", "")))
        current = normalize_path(str(action.get("current", "")))
        if reference in fixtures and current in fixtures and reference != current:
            return True
    return False


def _safe_stress_artifact_dir(value: str) -> Path | None:
    normalized = normalize_path(value)
    if not normalized.startswith("stress/artifacts/"):
        return None
    path = Path(normalized)
    if path.is_absolute() or any(part in {"", ".."} for part in path.parts):
        return None
    return path


def _registry_covers(entries: list[dict[str, Any]], coverage_type: str, target: str, path: str) -> bool:
    expected_type = coverage_type.strip().lower()
    expected_target = target.strip()
    expected_path = normalize_path(path)
    if not expected_type or not expected_target or not expected_path:
        return False
    for entry in entries:
        for coverage in entry.get("coverage", []) if isinstance(entry.get("coverage"), list) else []:
            if not isinstance(coverage, dict):
                continue
            if str(coverage.get("type", "")).strip().lower() != expected_type:
                continue
            if str(coverage.get("target", "")).strip() != expected_target:
                continue
            if normalize_path(str(coverage.get("path", ""))) == expected_path:
                return True
    return False


def _glob_paths(root: Path, patterns: list[str]) -> list[Path]:
    found: list[Path] = []
    for pattern in patterns:
        found.extend(path for path in root.glob(pattern) if path.is_file())
    return sorted(set(found))


def _path_or_glob_exists(root: Path, pattern: str) -> bool:
    normalized = normalize_path(pattern)
    if not normalized or normalized.startswith("/") or normalized.startswith("../") or "/../" in normalized:
        return False
    path = root / normalized
    if path.exists():
        return True
    try:
        return any(candidate.exists() for candidate in root.glob(normalized))
    except (NotImplementedError, ValueError):
        return False


def _gaps_have_planned_remediation(gaps: Any, active: set[str]) -> bool:
    if not active:
        return False
    if not isinstance(gaps, list):
        return False
    for gap in gaps:
        if isinstance(gap, dict):
            candidates = [
                gap.get("finding_id"),
                gap.get("planned_remediation"),
                gap.get("remediation"),
            ]
            if any(str(item) in active for item in candidates if item):
                return True
        elif isinstance(gap, str) and any(item in gap for item in active):
            return True
    return False


def _corpus_has_git_status_shape(repo: Path, corpus: Path, errors: list[str]) -> bool:
    max_seed_bytes = 8 * 1024 * 1024
    shape = {
        "nul": False,
        "ordinary": False,
        "unmerged": False,
        "control": False,
        "non_utf8": False,
    }
    for path in corpus.rglob("*"):
        if not path.is_file():
            continue
        rel = path.relative_to(repo)
        try:
            size = path.stat().st_size
        except OSError as exc:
            foundation.add(errors, f"{rel}: unable to stat fuzz seed ({exc})")
            continue
        if size > max_seed_bytes:
            foundation.add(errors, f"{rel}: fuzz seed is too large for policy shape scan ({size} bytes)")
            continue
        try:
            data = path.read_bytes()
        except OSError as exc:
            foundation.add(errors, f"{rel}: unable to read fuzz seed ({exc})")
            continue
        _merge_git_status_seed_shape(shape, data)
        if all(shape.values()):
            return True
    return all(shape.values())


def _merge_git_status_seed_shape(shape: dict[str, bool], data: bytes) -> None:
    if b"\0" in data:
        shape["nul"] = True
    for record in data.split(b"\0"):
        if not record:
            continue
        if record.startswith(b"1 "):
            shape["ordinary"] = True
            _scan_path_shape(shape, _field_at(record, 8))
        elif record.startswith(b"u "):
            shape["unmerged"] = True
            _scan_path_shape(shape, _field_at(record, 10))
        elif record.startswith(b"? "):
            _scan_path_shape(shape, record[2:])


def _field_at(record: bytes, index: int) -> bytes:
    fields = record.split(b" ", index)
    if len(fields) <= index:
        return b""
    return fields[index]


def _scan_path_shape(shape: dict[str, bool], path: bytes) -> None:
    if any(byte < 0x20 or byte == 0x7f for byte in path):
        shape["control"] = True
    try:
        path.decode("utf-8")
    except UnicodeDecodeError:
        shape["non_utf8"] = True


def _require_date(value: Any, label: str, field: str, errors: list[str]) -> None:
    if not isinstance(value, str):
        foundation.add(errors, f"{label}: {field} must be YYYY-MM-DD")
        return
    try:
        reviewed = dt.date.fromisoformat(value)
    except ValueError:
        foundation.add(errors, f"{label}: {field} must be YYYY-MM-DD")
        return
    today = dt.datetime.now(dt.UTC).date()
    if reviewed > today:
        foundation.add(errors, f"{label}: {field} must not be after {today}")
