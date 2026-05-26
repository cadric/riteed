from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path
from typing import Any

from tools.checks import foundation, remediation
from tools.validation_tooling import contract_root, normalize_path, read_text


POLICY_FILE = "policy/release.policy.json"
POLICY_ID = "policy/release.v1.json"
WORKFLOW = ".github/workflows/publish-flatpak.yml"
SIGNING_SECRETS = ("FLATPAK_GPG_PRIVATE_KEY", "FLATPAK_GPG_PASSPHRASE", "FLATPAK_GPG_KEY_ID")


def check_release(root: Path, errors: list[str]) -> None:
    repo = contract_root(root)
    policy_path = repo / POLICY_FILE
    if not policy_path.exists():
        foundation.add(errors, f"Missing release policy file: {POLICY_FILE}")
        return
    policy = foundation.release_policy(root)
    if policy.get("$id") != POLICY_ID:
        foundation.add(errors, f"{POLICY_FILE} must have $id {POLICY_ID}")
    _check_policy_stack(root, errors)
    active = remediation.validate_planned_remediation(policy, POLICY_FILE, errors)

    workflow_path = repo / WORKFLOW
    if not workflow_path.exists():
        foundation.add(errors, f"Missing release workflow: {WORKFLOW}")
        return
    workflow = read_text(workflow_path).replace("\r\n", "\n")
    _check_publish_triggers(workflow, errors)
    _check_secret_scope(workflow, active, errors)
    _check_validation_gate(policy, workflow, active, errors)
    _check_rollback_gate(workflow, active, errors)
    _check_key_governance(repo, policy, workflow, active, errors)
    _check_mutable_inputs(workflow, active, errors)
    _check_pages_artifact(workflow, errors)
    _check_ruleset_governance(active, errors)
    _check_local_patch_manifest(repo, policy, active, errors)


def _check_policy_stack(root: Path, errors: list[str]) -> None:
    bundle = foundation.policy_bundle(root)
    validation = foundation.validation_policy(root)
    files = {str(item.get("file")): str(item.get("$id")) for item in bundle.get("bundle_contains", [])}
    if files.get("release.policy.json") != POLICY_ID:
        foundation.add(errors, "Bundle must include release.policy.json with policy/release.v1.json")
    if POLICY_ID not in [str(item) for item in bundle.get("overlaps_with", [])]:
        foundation.add(errors, "Bundle overlaps_with must include policy/release.v1.json")
    if POLICY_FILE not in [str(item) for item in validation.get("required_policy_files", [])]:
        foundation.add(errors, "validation-tooling required_policy_files must include policy/release.policy.json")
    if POLICY_ID not in [str(item) for item in validation.get("overlaps_with", [])]:
        foundation.add(errors, "validation-tooling overlaps_with must include policy/release.v1.json")


def _check_publish_triggers(workflow: str, errors: list[str]) -> None:
    if "workflow_dispatch:" not in workflow:
        foundation.add(errors, f"{WORKFLOW}: workflow_dispatch trigger is required for reviewed manual release flow")
    if not re.search(r"(?m)^\s*-\s*[\"']?v\*[\"']?\s*$", workflow):
        foundation.add(errors, f"{WORKFLOW}: release tag trigger must include v*")
    if 'refs/tags/v*' not in workflow:
        foundation.add(errors, f"{WORKFLOW}: workflow_dispatch must validate that GITHUB_REF is a version tag")


def _check_secret_scope(workflow: str, active: set[str], errors: list[str]) -> None:
    if "pull_request:" in workflow and any(secret in workflow for secret in SIGNING_SECRETS):
        foundation.add(errors, f"{WORKFLOW}: pull_request workflows must not expose signing secret names")
    if any(secret in workflow for secret in SIGNING_SECRETS) and "environment: flatpak-beta-signing" not in workflow:
        foundation.add(errors, f"{WORKFLOW}: signing secrets must be scoped to flatpak-beta-signing environment")
    if "contents: read" not in workflow:
        foundation.add(errors, f"{WORKFLOW}: release workflow must default to contents: read permissions")
    if "pages: write" in workflow and not _has_github_pages_environment(workflow):
        foundation.add(errors, f"{WORKFLOW}: pages: write must stay scoped to the deploy job")
    if "id-token: write" in workflow and not _has_github_pages_environment(workflow):
        foundation.add(errors, f"{WORKFLOW}: id-token: write must stay scoped to the deploy job")


def _check_validation_gate(policy: dict[str, Any], workflow: str, active: set[str], errors: list[str]) -> None:
    if _has_validation_before_secret(policy, workflow):
        return
    if "RIT-AUD-001" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: signing secret import requires exact-commit validation gate before signing")


def _has_validation_before_secret(policy: dict[str, Any], workflow: str) -> bool:
    secret_pos = _first_secret_usage(workflow)
    before_secret = workflow[:secret_pos]
    uncommented = _without_comment_lines(before_secret)
    status_gate = "tag_commit" in uncommented and any(token in uncommented for token in ("check-runs", "workflow-runs", "gh api"))
    suite = policy.get("signed_flatpak_publish", {}).get("hard_requirements", {}).get("release_critical_validation_suite", [])
    rerun_gate = bool(suite) and all(_suite_item_present(uncommented, str(item)) for item in suite)
    return status_gate or rerun_gate


def _check_rollback_gate(workflow: str, active: set[str], errors: list[str]) -> None:
    lower = _without_comment_lines(workflow).lower()
    has_remote_check = any(token in lower for token in ("flatpak remote-ls", "flatpak remote-info", "ostree refs", "ostree log"))
    has_rollback = "emergency_rollback" in lower and any(token in lower for token in ("rollback_ref", "rollback_tag", "rollback_version"))
    if has_remote_check and has_rollback:
        return
    if "RIT-AUD-002" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: beta publish requires monotonic remote check and explicit rollback path")


def _check_key_governance(repo: Path, policy: dict[str, Any], workflow: str, active: set[str], errors: list[str]) -> None:
    rel_key = str(policy.get("release_identity", {}).get("committed_beta_public_key", ""))
    if not rel_key or not (repo / rel_key).exists():
        foundation.add(errors, f"{POLICY_FILE}: committed beta public key must exist")
    uncommented = _without_comment_lines(workflow)
    key_ref = bool(rel_key) and (rel_key in uncommented or Path(rel_key).name in uncommented)
    has_pin = key_ref and any(token in uncommented for token in ("--export", "--fingerprint", "cmp", "diff", "sha256sum"))
    readme = repo / "app" / "build-aux" / "flatpak" / "README.md"
    docs_ok = False
    if readme.exists():
        text = read_text(readme).lower()
        docs_ok = all(token in text for token in ("rotation", "revocation", "compromise", "emergency")) and "tbd" not in text
    if has_pin and docs_ok:
        return
    if "RIT-AUD-010" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: signing key must be pinned to committed public key and governance docs cannot contain TBD")


def _check_mutable_inputs(workflow: str, active: set[str], errors: list[str]) -> None:
    mutable_uses = re.findall(r"(?m)^\s*uses:\s*[^@\s]+@v[0-9]+(?:\s|$)", workflow)
    curl_pipe = re.search(r"curl\b.*\|\s*(?:sh|bash)", workflow)
    if not mutable_uses and not curl_pipe:
        return
    if "RIT-AUD-011" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: release-critical actions/tool installers must be pinned or reviewed")


def _check_pages_artifact(workflow: str, errors: list[str]) -> None:
    required = [
        ("no symlink check", lambda text: "find site -type l" in text),
        ("no hardlink check", lambda text: "st_nlink > 1" in text),
        ("summary exists", lambda text: "site/flatpak/repo/summary" in text),
        ("summary signature exists", lambda text: "site/flatpak/repo/summary.sig" in text),
        ("upload path site", lambda text: re.search(r"(?m)^\s*path:\s*site\s*(?:#.*)?$", text) is not None),
    ]
    for label, present in required:
        if not present(workflow):
            foundation.add(errors, f"{WORKFLOW}: missing Pages artifact safety check for {label}")


def _check_ruleset_governance(active: set[str], errors: list[str]) -> None:
    if "RIT-AUD-017" in active:
        return
    foundation.add(errors, "GitHub main/tag ruleset governance must be enabled or explicitly documented")


def _check_local_patch_manifest(repo: Path, policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
    for item in policy.get("local_patch_policy", {}).get("release_critical_local_patches", []):
        if not isinstance(item, dict):
            continue
        manifest_rel = str(item.get("manifest", ""))
        manifest = repo / manifest_rel
        if not manifest.exists():
            if "RIT-AUD-009" not in active:
                foundation.add(errors, f"{manifest_rel}: sourceview5 patch manifest is required")
            continue
        _validate_patch_manifest(manifest, item, errors)


def _validate_patch_manifest(manifest: Path, policy_entry: dict[str, Any], errors: list[str]) -> None:
    try:
        data = json.loads(read_text(manifest))
    except json.JSONDecodeError as exc:
        foundation.add(errors, f"{manifest}: invalid JSON ({exc})")
        return
    required = [str(field) for field in policy_entry.get("required_manifest_fields", [])]
    for field in required:
        if field not in data:
            foundation.add(errors, f"{manifest}: missing required field {field}")
    patch_dir = manifest.parent
    _check_allowed_patch_files(manifest, patch_dir, data, errors)
    _check_patch_tree_checksum(manifest, patch_dir, data, errors)
    _check_unsafe_baseline(manifest, patch_dir, data, errors)


def _first_secret_usage(workflow: str) -> int:
    offset = 0
    for line in workflow.splitlines(keepends=True):
        stripped = line.lstrip()
        if not stripped.startswith("#") and any(secret in line for secret in SIGNING_SECRETS):
            return offset + min(line.find(secret) for secret in SIGNING_SECRETS if secret in line)
        offset += len(line)
    return len(workflow)


def _without_comment_lines(text: str) -> str:
    return "\n".join(line for line in text.splitlines() if not line.lstrip().startswith("#"))


def _has_github_pages_environment(workflow: str) -> bool:
    patterns = (
        r"(?m)^\s*environment:\s*github-pages\s*$",
        r"(?ms)^\s*environment:\s*\n\s*name:\s*github-pages\b",
        r"(?m)^\s*environment:\s*\{[^}\n]*name\s*:\s*github-pages\b[^}\n]*\}",
    )
    return any(re.search(pattern, workflow) for pattern in patterns)


def _suite_item_present(text: str, item: str) -> bool:
    if item in text:
        return True
    lowered = item.lower()
    if "flatpak build" in lowered or "flatpak" in lowered and "smoke" in lowered:
        return "flatpak-builder" in text or "flatpak run" in text
    return False


def _check_allowed_patch_files(manifest: Path, patch_dir: Path, data: dict[str, Any], errors: list[str]) -> None:
    raw_allowed = data.get("allowed_changed_files")
    if not isinstance(raw_allowed, list) or not all(isinstance(item, str) and item.strip() for item in raw_allowed):
        foundation.add(errors, f"{manifest}: allowed_changed_files must be a non-empty string array")
        return
    allowed = {normalize_path(item) for item in raw_allowed}
    for path in patch_dir.rglob("*"):
        if not path.is_file() or _ignored_patch_file(path):
            continue
        rel = path.relative_to(patch_dir).as_posix()
        if rel == manifest.name:
            continue
        if rel not in allowed:
            foundation.add(errors, f"{manifest}: unlisted local patch file {rel}")


def _check_patch_tree_checksum(manifest: Path, patch_dir: Path, data: dict[str, Any], errors: list[str]) -> None:
    expected = data.get("diff_checksum_sha256")
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        foundation.add(errors, f"{manifest}: diff_checksum_sha256 must be a lowercase sha256 hex digest")
        return
    actual = _patch_tree_checksum(patch_dir, manifest.name)
    if actual != expected:
        foundation.add(errors, f"{manifest}: diff_checksum_sha256 mismatch, expected {actual}")


def _patch_tree_checksum(patch_dir: Path, manifest_name: str) -> str:
    digest = hashlib.sha256()
    for path in sorted(path for path in patch_dir.rglob("*") if path.is_file() and not _ignored_patch_file(path)):
        rel = path.relative_to(patch_dir).as_posix()
        if rel == manifest_name:
            continue
        digest.update(rel.encode("utf-8") + b"\0")
        digest.update(path.read_bytes())
        digest.update(b"\0")
    return digest.hexdigest()


def _check_unsafe_baseline(manifest: Path, patch_dir: Path, data: dict[str, Any], errors: list[str]) -> None:
    baseline = _baseline_count(data.get("unsafe_ffi_baseline"))
    if baseline is None:
        foundation.add(errors, f"{manifest}: unsafe_ffi_baseline must include audited_total_matches")
        return
    current = _unsafe_ffi_count(patch_dir, errors)
    if current != baseline:
        foundation.add(errors, f"{manifest}: unsafe_ffi_baseline mismatch, expected current count {current}")


def _baseline_count(value: Any) -> int | None:
    if isinstance(value, int) and not isinstance(value, bool):
        return value
    if isinstance(value, dict):
        for key in ("audited_total_matches", "total_matches", "count"):
            nested = value.get(key)
            if isinstance(nested, int) and not isinstance(nested, bool):
                return nested
    return None


def _unsafe_ffi_count(patch_dir: Path, errors: list[str]) -> int:
    pattern = re.compile(r'\bunsafe\b|\bextern\s+"C"|\btransmute\b')
    total = 0
    for path in sorted((patch_dir / "src").rglob("*.rs")):
        try:
            total += len(pattern.findall(read_text(path)))
        except SystemExit:
            foundation.add(errors, f"{path}: unable to read sourceview5 patch source")
    return total


def _ignored_patch_file(path: Path) -> bool:
    return any(part in {".git", "target", "__pycache__"} for part in path.parts)
