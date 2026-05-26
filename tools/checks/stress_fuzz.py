from __future__ import annotations

import json
import datetime as dt
from pathlib import Path
from typing import Any

from tools.checks import foundation, remediation
from tools.validation_tooling import contract_root, normalize_path, read_text


POLICY_FILE = "policy/stress-fuzz.policy.json"
POLICY_ID = "policy/stress_fuzz.v1.json"
REGISTRY_FINDING = "RIT-AUD-015"
STRESS_FINDING = "RIT-AUD-008"


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
    _check_parser_registry(repo, policy, active, errors)
    _check_fuzz_targets(repo, policy, active, errors)
    _check_stress_scripts(repo, policy, active, errors)
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


def _check_parser_registry(repo: Path, policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
    cfg = policy.get("parser_boundary_registry", {})
    registry_paths = _glob_paths(repo, [str(item) for item in cfg.get("globs", [])])
    if not registry_paths:
        if REGISTRY_FINDING not in active:
            foundation.add(errors, "Parser-boundary registry is required under app/build-aux/validation/")
        return
    entries = _registry_entries(repo, registry_paths, errors)
    seen: set[str] = set()
    required_fields = [str(item) for item in cfg.get("entry_shape", {}).get("required_fields", [])]
    for entry, label in entries:
        if not isinstance(entry, dict):
            foundation.add(errors, f"{label}: registry entry must be an object")
            continue
        missing = [field for field in required_fields if field not in entry]
        if missing:
            foundation.add(errors, f"{label}: missing required fields: {', '.join(missing)}")
        entry_id = str(entry.get("id", "")).strip()
        if not entry_id:
            continue
        if entry_id in seen:
            foundation.add(errors, f"{label}: duplicate parser-boundary id {entry_id}")
        seen.add(entry_id)
        _check_registry_paths(repo, entry, label, active, errors)
    for boundary_id in [str(item) for item in cfg.get("minimum_boundary_ids", [])]:
        if boundary_id not in seen:
            foundation.add(errors, f"Parser-boundary registry missing minimum id {boundary_id}")


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


def _check_fuzz_targets(repo: Path, policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
    cfg = policy.get("fuzz_targets", {})
    workspace = repo / str(cfg.get("workspace", "app/fuzz"))
    for target in [str(item) for item in cfg.get("targets_required", [])]:
        target_file = workspace / "fuzz_targets" / f"{target}.rs"
        if not target_file.exists():
            foundation.add(errors, f"{target_file.relative_to(repo)}: required fuzz target is missing")
        corpus = workspace / "corpus" / target
        if not corpus.exists() or not any(path.is_file() for path in corpus.rglob("*")):
            foundation.add(errors, f"{corpus.relative_to(repo)}: required fuzz seed corpus is missing")
    git_corpus = workspace / "corpus" / "git_status_parse"
    has_nul_seed = _corpus_has_nul_seed(repo, git_corpus, errors) if git_corpus.exists() else False
    if not has_nul_seed and REGISTRY_FINDING not in active:
        foundation.add(errors, "git_status_parse corpus must include NUL-delimited porcelain v2 -z seeds")


def _check_stress_scripts(repo: Path, policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
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
            _check_script_boundary_fidelity(rel, data, cfg, errors)


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
    tokens = _json_value_tokens({key: data.get(key) for key in ("fixtures", "actions", "assertions", "artifact_dir")})
    token_map = {
        "must_open_generated_corpus_file": ("stress/corpus/generated", "generated"),
        "must_perform_search_action": ("search",),
        "must_perform_save_or_save_as_boundary": ("save", "save_as", "save-as"),
        "must_assert_document_or_search_state": ("assert", "document", "search"),
        "must_start_compare_workflow": ("compare",),
        "must_assert_compare_pane_or_diff_state": ("assert", "compare", "diff"),
        "must_use_input_files_from_declared_fixtures": ("fixtures",),
        "must_open_generated_markdown_corpus": ("stress/corpus/generated", "markdown"),
        "must_toggle_or_render_preview_boundary": ("preview", "render"),
        "must_assert_preview_or_fallback_state": ("assert", "preview", "fallback"),
        "must_open_generated_git_repos": ("stress/git-repos/generated",),
        "must_include_many_untracked_many_modified_conflicted_non_utf8_submodule_and_lock_cases": (
            "many-untracked",
            "many-modified",
            "conflicted",
            "non-utf8",
            "submodule",
            "index-lock",
        ),
        "must_assert_source_control_or_degraded_state": ("assert", "source-control", "too-large", "degraded"),
    }
    for key, enabled in requirements.items():
        if not enabled:
            continue
        expected = token_map.get(str(key))
        if expected and not any(token in tokens for token in expected):
            foundation.add(errors, f"{rel}: boundary fidelity requirement {key} is not represented in script")


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


def _json_value_tokens(value: Any) -> str:
    if isinstance(value, dict):
        return " ".join(_json_value_tokens(item) for item in value.values())
    if isinstance(value, list):
        return " ".join(_json_value_tokens(item) for item in value)
    if value is None:
        return ""
    return str(value).lower()


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


def _corpus_has_nul_seed(repo: Path, corpus: Path, errors: list[str]) -> bool:
    max_seed_bytes = 8 * 1024 * 1024
    found = False
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
            with path.open("rb") as handle:
                for chunk in iter(lambda: handle.read(65536), b""):
                    if b"\0" in chunk:
                        found = True
                        break
        except OSError as exc:
            foundation.add(errors, f"{rel}: unable to read fuzz seed ({exc})")
        if found:
            return True
    return False


def _require_date(value: Any, label: str, field: str, errors: list[str]) -> None:
    if not isinstance(value, str):
        foundation.add(errors, f"{label}: {field} must be YYYY-MM-DD")
        return
    try:
        dt.date.fromisoformat(value)
    except ValueError:
        foundation.add(errors, f"{label}: {field} must be YYYY-MM-DD")
