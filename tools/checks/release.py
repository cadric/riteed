from __future__ import annotations

import hashlib
import json
import re
import tarfile
import tempfile
from pathlib import Path
from typing import Any

from tools.checks import foundation, release_workflow, remediation
from tools.validation_tooling import contract_root, normalize_path, read_text


POLICY_FILE = "policy/release.policy.json"
POLICY_ID = "policy/release.v1.json"
WORKFLOW = ".github/workflows/publish-flatpak.yml"
SIGNING_SECRETS = ("FLATPAK_GPG_PRIVATE_KEY", "FLATPAK_GPG_PASSPHRASE", "FLATPAK_GPG_KEY_ID")
CRATE_EXTRACT_CHUNK_BYTES = 1024 * 1024
CRATE_MEMBER_MAX_BYTES = 16 * 1024 * 1024
CRATE_TOTAL_MAX_BYTES = 64 * 1024 * 1024


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
    workflow_model = release_workflow.parse(WORKFLOW, workflow, errors)
    validation_workflow_path = repo / ".github" / "workflows" / "validate.yml"
    validation_workflow = ""
    validation_workflow_model = None
    if validation_workflow_path.exists():
        validation_workflow = read_text(validation_workflow_path).replace("\r\n", "\n")
        validation_workflow_model = release_workflow.parse(".github/workflows/validate.yml", validation_workflow, errors)
    else:
        foundation.add(errors, "Missing validation workflow: .github/workflows/validate.yml")
    release_workflow.check_publish_triggers(workflow_model, errors)
    release_workflow.check_secret_scope(workflow_model, workflow, errors)
    release_workflow.check_validation_gate(policy, workflow_model, active, errors)
    _check_rollback_gate(workflow, active, errors)
    _check_rollback_environment_policy(policy, errors)
    _check_key_governance(repo, policy, workflow, active, errors)
    _check_mutable_inputs({WORKFLOW: workflow, ".github/workflows/validate.yml": validation_workflow}, active, errors)
    _check_pages_artifact(workflow, errors)
    release_workflow.check_ruleset_governance_wiring(validation_workflow_model, errors)
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


def _check_rollback_gate(workflow: str, active: set[str], errors: list[str]) -> None:
    lower = release_workflow.without_comment_lines(workflow).lower()
    has_remote_check = any(token in lower for token in ("flatpak remote-ls", "flatpak remote-info", "ostree refs", "ostree log"))
    has_version_gate = all(token in lower for token in ("candidate_version", "published_version", "version_key"))
    has_ref_gate = all(
        token in lower
        for token in (
            "candidate_ref",
            "candidate_commit",
            "published_source_ref",
            "published_source_commit",
        )
    )
    has_rollback = "emergency_rollback" in lower and all(token in lower for token in ("rollback_ref", "rollback_reason"))
    has_environment_route = all(
        token in lower
        for token in (
            "flatpak-beta-signing",
            "flatpak-beta-rollback",
            "needs.preflight.outputs.emergency_rollback",
        )
    )
    has_release_metadata = "--rollback-ref" in lower and "--rollback-reason" in lower
    has_fail_closed_metadata = (
        "published beta metadata is malformed" in lower
        and "published beta version metadata is required" in lower
    )
    if (
        has_remote_check
        and has_version_gate
        and has_ref_gate
        and has_rollback
        and has_environment_route
        and has_release_metadata
        and has_fail_closed_metadata
    ):
        return
    if "RIT-AUD-002" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: beta publish requires monotonic version/ref check and explicit rollback path")


def _check_rollback_environment_policy(policy: dict[str, Any], errors: list[str]) -> None:
    rollback = policy.get("github_actions_release_safety", {}).get("rollback_environment", {})
    if rollback.get("name") != "flatpak-beta-rollback":
        foundation.add(errors, f"{POLICY_FILE}: rollback_environment.name must be flatpak-beta-rollback")
    if rollback.get("normal_signing_environment") != "flatpak-beta-signing":
        foundation.add(errors, f"{POLICY_FILE}: rollback_environment.normal_signing_environment must be flatpak-beta-signing")
    if not _reviewed_rollback_reviewer_keys(policy):
        foundation.add(errors, f"{POLICY_FILE}: rollback_environment.reviewed_required_reviewers is required")


def _check_key_governance(repo: Path, policy: dict[str, Any], workflow: str, active: set[str], errors: list[str]) -> None:
    rel_key = str(policy.get("release_identity", {}).get("committed_beta_public_key", ""))
    key_path = _safe_repo_relative_path(repo, rel_key)
    if key_path is None or not key_path.exists():
        foundation.add(errors, f"{POLICY_FILE}: committed beta public key must exist")
    uncommented = release_workflow.without_comment_lines(workflow)
    key_ref = bool(rel_key) and (rel_key in uncommented or Path(rel_key).name in uncommented)
    has_pin = key_ref and any(token in uncommented for token in ("--export", "--fingerprint", "cmp", "diff", "sha256sum"))
    readme = _safe_repo_relative_path(repo, "app/build-aux/flatpak/README.md")
    docs_ok = False
    if readme is not None and readme.exists():
        text = read_text(readme).lower()
        docs_ok = all(token in text for token in ("rotation", "revocation", "compromise", "emergency")) and "tbd" not in text
    if has_pin and docs_ok:
        return
    if "RIT-AUD-010" in active:
        return
    foundation.add(errors, f"{WORKFLOW}: signing key must be pinned to committed public key and governance docs cannot contain TBD")


def _safe_repo_relative_path(repo: Path, value: str) -> Path | None:
    normalized = normalize_path(value)
    if not normalized or normalized.startswith("/") or normalized.startswith("../") or "/../" in normalized:
        return None
    candidate = (repo / normalized).resolve()
    try:
        candidate.relative_to(repo.resolve())
    except ValueError:
        return None
    return candidate


def _check_mutable_inputs(workflows: dict[str, str], active: set[str], errors: list[str]) -> None:
    mutable_hits: list[str] = []
    for label, workflow in workflows.items():
        scan_text = _normalize_shell_continuations(release_workflow.without_comment_lines(workflow))
        mutable_uses = re.findall(r"(?m)^\s*uses:\s*[^@\s]+@v[0-9]+(?:\s|$)", scan_text)
        curl_pipe = re.search(r"\bcurl\b[^\n]*\|\s*(?:sh|bash)\b", scan_text)
        unpinned_cargo = re.findall(r"(?m)^\s*cargo\s+install\s+\S+(?![^\n]*\s--version\s+\S+)[^\n]*$", scan_text)
        if mutable_uses or curl_pipe or unpinned_cargo:
            mutable_hits.append(label)
    if not mutable_hits:
        return
    if "RIT-AUD-011" in active:
        return
    foundation.add(errors, f"{', '.join(sorted(mutable_hits))}: release-critical actions/tool installers must be pinned or reviewed")


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


def _check_local_patch_manifest(repo: Path, policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
    for item in policy.get("local_patch_policy", {}).get("release_critical_local_patches", []):
        if not isinstance(item, dict):
            continue
        manifest_rel = str(item.get("manifest", ""))
        manifest = repo / manifest_rel
        if not manifest.exists():
            if "RIT-AUD-009" not in active:
                crate = str(item.get("crate") or "release-critical")
                foundation.add(errors, f"{manifest_rel}: {crate} patch manifest is required")
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
    upstream_data = _load_upstream_crate(manifest, patch_dir, data, errors)
    _check_patch_identity(manifest, data, policy_entry, errors)
    if upstream_data is not None:
        upstream, tmp = upstream_data
        try:
            _check_allowed_patch_files(manifest, patch_dir, upstream, data, errors)
            _check_patch_tree_checksum(manifest, patch_dir, upstream, data, errors)
        finally:
            tmp.cleanup()
    _check_unsafe_baseline(manifest, patch_dir, data, errors)


def _reviewed_rollback_reviewer_keys(policy: dict[str, Any]) -> set[tuple[str, int]]:
    rollback = policy.get("github_actions_release_safety", {}).get("rollback_environment", {})
    reviewers = rollback.get("reviewed_required_reviewers", [])
    keys: set[tuple[str, int]] = set()
    for reviewer in reviewers if isinstance(reviewers, list) else []:
        if not isinstance(reviewer, dict):
            continue
        actor_type = str(reviewer.get("actor_type", "")).strip()
        actor_id = reviewer.get("actor_id")
        if actor_type and isinstance(actor_id, int):
            keys.add((actor_type, actor_id))
    return keys


def _normalize_shell_continuations(text: str) -> str:
    previous = None
    normalized = text
    while previous != normalized:
        previous = normalized
        normalized = re.sub(r"\\[ \t]*\n[ \t]*", " ", normalized)
    return normalized


def _check_allowed_patch_files(
    manifest: Path,
    patch_dir: Path,
    upstream: Path,
    data: dict[str, Any],
    errors: list[str],
) -> None:
    raw_allowed = data.get("allowed_changed_files")
    if not isinstance(raw_allowed, list) or not all(isinstance(item, str) and item.strip() for item in raw_allowed):
        foundation.add(errors, f"{manifest}: allowed_changed_files must be a non-empty string array")
        return
    allowed = {normalize_path(item) for item in raw_allowed}
    changed = {entry[1] for entry in _patch_diff_entries(patch_dir, upstream, manifest.name)}
    for rel in sorted(changed - allowed):
        foundation.add(errors, f"{manifest}: unlisted local patch file {rel}")
    for rel in sorted(allowed - changed):
        foundation.add(errors, f"{manifest}: allowed_changed_files includes unchanged file {rel}")


def _check_patch_tree_checksum(
    manifest: Path,
    patch_dir: Path,
    upstream: Path,
    data: dict[str, Any],
    errors: list[str],
) -> None:
    expected = data.get("diff_checksum_sha256")
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-f]{64}", expected):
        foundation.add(errors, f"{manifest}: diff_checksum_sha256 must be a lowercase sha256 hex digest")
        return
    actual = _patch_diff_checksum(patch_dir, upstream, manifest.name)
    if actual != expected:
        foundation.add(errors, f"{manifest}: diff_checksum_sha256 mismatch, expected {actual}")


def _patch_diff_checksum(patch_dir: Path, upstream: Path, manifest_name: str) -> str:
    digest = hashlib.sha256()
    for kind, rel, local, original in _patch_diff_entries(patch_dir, upstream, manifest_name):
        digest.update(kind.encode("utf-8") + b"\0")
        digest.update(rel.encode("utf-8") + b"\0")
        digest.update(original + b"\0")
        digest.update(local + b"\0")
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
            foundation.add(errors, f"{path}: unable to read local patch source")
    return total


def _ignored_patch_file(path: Path) -> bool:
    return any(part in {".git", "target", "__pycache__", "upstream"} for part in path.parts)


def _check_patch_identity(
    manifest: Path,
    data: dict[str, Any],
    policy_entry: dict[str, Any],
    errors: list[str],
) -> None:
    if data.get("crate") != policy_entry.get("crate"):
        foundation.add(errors, f"{manifest}: crate must be {policy_entry.get('crate')!r}")
    if not str(data.get("version", "")).strip():
        foundation.add(errors, f"{manifest}: version must be set")
    if not data.get("review_evidence"):
        foundation.add(errors, f"{manifest}: review_evidence must be set")
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", str(data.get("last_reviewed", ""))):
        foundation.add(errors, f"{manifest}: last_reviewed must be YYYY-MM-DD")


def _load_upstream_crate(
    manifest: Path,
    patch_dir: Path,
    data: dict[str, Any],
    errors: list[str],
) -> tuple[Path, tempfile.TemporaryDirectory[str]] | None:
    source = data.get("upstream_source")
    archive_rel = ""
    if isinstance(source, dict):
        archive_rel = str(source.get("crate_archive", "")).strip()
    elif isinstance(source, str):
        archive_rel = source.strip()
    if not archive_rel:
        foundation.add(errors, f"{manifest}: upstream_source must include crate_archive")
        return None
    archive_path = _safe_patch_relative_path(archive_rel)
    if archive_path is None:
        foundation.add(errors, f"{manifest}: upstream crate archive path must be relative and stay under patch directory")
        return None
    archive = patch_dir / archive_path
    if not archive.exists():
        foundation.add(errors, f"{manifest}: upstream crate archive is missing: {archive_rel}")
        return None
    expected = data.get("upstream_crate_checksum")
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    if expected != actual:
        foundation.add(errors, f"{manifest}: upstream_crate_checksum mismatch, expected {actual}")
        return None
    tmp = tempfile.TemporaryDirectory(prefix="crate-upstream-")
    tmp_path = Path(tmp.name)
    if not _extract_crate_safely(archive, tmp_path, errors):
        tmp.cleanup()
        return None
    roots = [path for path in tmp_path.iterdir() if path.is_dir()]
    if len(roots) != 1:
        foundation.add(errors, f"{manifest}: upstream crate archive must contain one root directory")
        tmp.cleanup()
        return None
    return roots[0], tmp


def _safe_patch_relative_path(value: str) -> Path | None:
    normalized = normalize_path(value)
    if not normalized or normalized.startswith("/") or normalized.startswith("../") or "/../" in normalized:
        return None
    path = Path(normalized)
    if path.is_absolute() or any(part in {"..", ""} for part in path.parts):
        return None
    return path


def _extract_crate_safely(archive: Path, dest: Path, errors: list[str]) -> bool:
    try:
        with tarfile.open(archive, "r:gz") as tar:
            total_bytes = 0
            for member in tar.getmembers():
                name = member.name
                parts = Path(name).parts
                if Path(name).is_absolute() or ".." in parts:
                    foundation.add(errors, f"{archive}: unsafe crate archive path {name}")
                    return False
                target = (dest / name).resolve()
                if not str(target).startswith(str(dest.resolve()) + "/"):
                    foundation.add(errors, f"{archive}: crate archive path escapes extraction root {name}")
                    return False
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                if not member.isfile():
                    foundation.add(errors, f"{archive}: crate archive contains unsupported member {name}")
                    return False
                if member.size > CRATE_MEMBER_MAX_BYTES:
                    foundation.add(errors, f"{archive}: crate archive member {name} exceeds {CRATE_MEMBER_MAX_BYTES} bytes")
                    return False
                if total_bytes + member.size > CRATE_TOTAL_MAX_BYTES:
                    foundation.add(errors, f"{archive}: crate archive exceeds {CRATE_TOTAL_MAX_BYTES} extracted bytes")
                    return False
                target.parent.mkdir(parents=True, exist_ok=True)
                source = tar.extractfile(member)
                if source is None:
                    foundation.add(errors, f"{archive}: unable to read crate member {name}")
                    return False
                with target.open("wb") as handle:
                    written = 0
                    while True:
                        chunk = source.read(CRATE_EXTRACT_CHUNK_BYTES)
                        if not chunk:
                            break
                        written += len(chunk)
                        if written > CRATE_MEMBER_MAX_BYTES:
                            foundation.add(errors, f"{archive}: crate archive member {name} exceeds {CRATE_MEMBER_MAX_BYTES} bytes")
                            return False
                        handle.write(chunk)
                total_bytes += written
    except (OSError, tarfile.TarError) as exc:
        foundation.add(errors, f"{archive}: unable to extract upstream crate ({exc})")
        return False
    return True


def _patch_diff_entries(
    patch_dir: Path,
    upstream: Path,
    manifest_name: str,
) -> list[tuple[str, str, bytes, bytes]]:
    local_files = _patch_file_map(patch_dir, patch_dir, manifest_name)
    upstream_files = _patch_file_map(upstream, upstream, manifest_name)
    entries: list[tuple[str, str, bytes, bytes]] = []
    for rel in sorted(local_files.keys() | upstream_files.keys()):
        local = local_files.get(rel)
        original = upstream_files.get(rel)
        if local == original:
            continue
        if local is None:
            entries.append(("removed", rel, b"", original or b""))
        elif original is None:
            entries.append(("added", rel, local, b""))
        else:
            entries.append(("changed", rel, local, original))
    return entries


def _patch_file_map(root: Path, base: Path, manifest_name: str) -> dict[str, bytes]:
    found: dict[str, bytes] = {}
    for path in root.rglob("*"):
        if not path.is_file() or _ignored_patch_file(path):
            continue
        rel = path.relative_to(base).as_posix()
        if rel == manifest_name:
            continue
        found[rel] = path.read_bytes()
    return found
