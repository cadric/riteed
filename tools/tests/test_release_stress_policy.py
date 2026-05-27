from __future__ import annotations

import datetime as dt
import hashlib
import io
import json
import shutil
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.checks import foundation, release, release_workflow, remediation, rulesets, stress_fuzz
from tools.tests.test_release_rulesets import _active_rulesets
from tools.validation_tooling import iter_files


REPO_ROOT = Path(__file__).resolve().parents[2]


def _write(path: Path, text: str | bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if isinstance(text, bytes):
        path.write_bytes(text)
    else:
        path.write_text(text, encoding="utf-8")


def _copy_policy(root: Path) -> None:
    shutil.copytree(REPO_ROOT / "policy", root / "policy")


def _remove_remediation(root: Path, policy_name: str, finding_id: str) -> None:
    path = root / "policy" / policy_name
    data = json.loads(path.read_text(encoding="utf-8"))
    data["planned_remediation"] = [
        item for item in data.get("planned_remediation", []) if item.get("finding_id") != finding_id
    ]
    path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def _copy_release_context(root: Path) -> None:
    _copy_policy(root)
    _write(root / "AGENTS.md", "# test\n")
    _write(
        root / ".github" / "workflows" / "publish-flatpak.yml",
        (REPO_ROOT / ".github" / "workflows" / "publish-flatpak.yml").read_text(encoding="utf-8"),
    )
    _write(
        root / ".github" / "workflows" / "validate.yml",
        (REPO_ROOT / ".github" / "workflows" / "validate.yml").read_text(encoding="utf-8"),
    )
    shutil.copytree(REPO_ROOT / "app" / "build-aux" / "flatpak", root / "app" / "build-aux" / "flatpak")
    shutil.copytree(
        REPO_ROOT / "app" / "build-aux" / "cargo-patches",
        root / "app" / "build-aux" / "cargo-patches",
    )
    shutil.copytree(
        REPO_ROOT / "app" / "build-aux" / "dependencies",
        root / "app" / "build-aux" / "dependencies",
    )


def _copy_stress_context(root: Path) -> None:
    _copy_policy(root)
    _write(root / "AGENTS.md", "# test\n")
    _write(
        root / ".github" / "workflows" / "validate.yml",
        (REPO_ROOT / ".github" / "workflows" / "validate.yml").read_text(encoding="utf-8"),
    )
    shutil.copytree(REPO_ROOT / "app" / "build-aux" / "validation", root / "app" / "build-aux" / "validation")
    shutil.copytree(REPO_ROOT / "app" / "fuzz", root / "app" / "fuzz")
    shutil.copytree(REPO_ROOT / "app" / "src", root / "app" / "src")
    shutil.copytree(REPO_ROOT / "stress" / "corpus", root / "stress" / "corpus")
    shutil.copytree(REPO_ROOT / "stress" / "git-repos", root / "stress" / "git-repos")
    shutil.copytree(REPO_ROOT / "stress" / "scripts", root / "stress" / "scripts")
    _write(
        root / "stress" / "make_corpus.py",
        (REPO_ROOT / "stress" / "make_corpus.py").read_text(encoding="utf-8"),
    )


class ReleaseStressPolicyTests(unittest.TestCase):
    def setUp(self) -> None:
        self._ruleset_patch = patch.object(rulesets, "fetch_repository_rulesets", return_value=_active_rulesets())
        self._ruleset_patch.start()

    def tearDown(self) -> None:
        self._ruleset_patch.stop()

    def test_target_named_source_directory_is_scanned(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write(root / "src" / "target" / "example.rs", "fn demo() { value.unwrap(); }\n")
            _write(root / "target" / "debug" / "generated.rs", "fn demo() { value.unwrap(); }\n")
            nested = root / "app" / "build-aux" / "cargo-patches" / "sourceview5"
            _write(nested / "Cargo.toml", "[package]\nname='sourceview5'\nversion='0.0.0'\nedition='2024'\n")
            _write(nested / "target" / "debug" / "generated.rs", "unsafe fn generated() {}\n")
            errors: list[str] = []
            foundation.check_forbidden_patterns(root, errors)
            self.assertTrue(any("src/target/example.rs" in item for item in errors), errors)
            scanned = {path.relative_to(root).as_posix() for path in iter_files(root)}
            self.assertIn("src/target/example.rs", scanned)
            self.assertNotIn("target/debug/generated.rs", scanned)
            self.assertNotIn("app/build-aux/cargo-patches/sourceview5/target/debug/generated.rs", scanned)

    def test_planned_remediation_rejects_missing_field_and_expiry(self) -> None:
        policy = {
            "planned_remediation": [
                {
                    "finding_id": "RIT-AUD-001",
                    "target_milestone": "V14.7",
                    "review_artifact": ".github/workflows/publish-flatpak.yml",
                    "created": "2026-01-01",
                    "max_age_days": 1,
                    "reason": "gap",
                    "removal_condition": "fixed",
                },
                {
                    "finding_id": "RIT-AUD-002",
                    "target_milestone": "V14.7",
                    "created": "2026-05-25",
                    "max_age_days": 30,
                    "reason": "gap",
                    "removal_condition": "fixed",
                },
            ]
        }
        errors: list[str] = []
        active = remediation.validate_planned_remediation(policy, "policy/test.json", errors, today=dt.date(2026, 5, 26))
        self.assertTrue(any("expired" in item for item in errors), errors)
        self.assertTrue(any("missing required fields: review_artifact" in item for item in errors), errors)
        self.assertNotIn("RIT-AUD-001", active)

    def test_invalid_created_date_does_not_activate_remediation(self) -> None:
        policy = {
            "planned_remediation": [
                {
                    "finding_id": "RIT-AUD-001",
                    "target_milestone": "V14.7",
                    "review_artifact": ".github/workflows/publish-flatpak.yml",
                    "created": "not-a-date",
                    "max_age_days": 30,
                    "reason": "gap",
                    "removal_condition": "fixed",
                }
            ]
        }
        errors: list[str] = []
        active = remediation.validate_planned_remediation(policy, "policy/test.json", errors, today=dt.date(2026, 5, 26))
        self.assertTrue(any("created must be ISO" in item for item in errors), errors)
        self.assertNotIn("RIT-AUD-001", active)

    def test_future_created_date_does_not_activate_remediation(self) -> None:
        policy = {
            "planned_remediation": [
                {
                    "finding_id": "RIT-AUD-001",
                    "target_milestone": "V14.7",
                    "review_artifact": ".github/workflows/publish-flatpak.yml",
                    "created": "2099-01-01",
                    "max_age_days": 30,
                    "reason": "gap",
                    "removal_condition": "fixed",
                }
            ]
        }
        errors: list[str] = []
        active = remediation.validate_planned_remediation(policy, "policy/test.json", errors, today=dt.date(2026, 5, 26))
        self.assertTrue(any("created must not be after" in item for item in errors), errors)
        self.assertNotIn("RIT-AUD-001", active)

    def test_invalid_empty_finding_ids_do_not_report_fake_duplicates(self) -> None:
        entry = {
            "finding_id": None,
            "target_milestone": "V14.7",
            "review_artifact": "artifact.json",
            "created": "2026-05-25",
            "max_age_days": 30,
            "reason": "gap",
            "removal_condition": "fixed",
        }
        errors: list[str] = []
        remediation.validate_planned_remediation({"planned_remediation": [entry, entry]}, "policy/test.json", errors)
        self.assertFalse(any("duplicate finding_id " in item for item in errors), errors)

    def test_release_policy_id_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / "policy" / "release.policy.json"
            data = json.loads(path.read_text(encoding="utf-8"))
            data["$id"] = "policy/wrong.json"
            path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("policy/release.policy.json must have $id" in item for item in errors), errors)

    def test_release_validation_gate_requires_remediation_until_fixed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            workflow = root / ".github" / "workflows" / "publish-flatpak.yml"
            workflow.write_text(
                workflow.read_text(encoding="utf-8").replace("gh api", "echo").replace("check-runs", "no-checks"),
                encoding="utf-8",
            )
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("exact-commit validation gate" in item for item in errors), errors)

    def test_secret_name_in_comment_does_not_shift_validation_gate_cutoff(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            data = json.loads((root / "policy" / "release.policy.json").read_text(encoding="utf-8"))
            workflow = (root / ".github" / "workflows" / "publish-flatpak.yml").read_text(encoding="utf-8")
            workflow = "# Mentions FLATPAK_GPG_PRIVATE_KEY for docs only.\n" + workflow
            self.assertTrue(release_workflow.has_validation_before_secret(data, workflow))

    def test_release_key_pin_and_patch_manifest_require_remediation_until_fixed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            (root / "app" / "build-aux" / "cargo-patches" / "sourceview5" / "patch-manifest.json").unlink()
            workflow = root / ".github" / "workflows" / "publish-flatpak.yml"
            workflow.write_text(
                workflow.read_text(encoding="utf-8").replace(
                    '          committed_key="app/build-aux/flatpak/riteed-beta-public.asc"\n',
                    "",
                ),
                encoding="utf-8",
            )
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("signing key must be pinned" in item for item in errors), errors)
            self.assertTrue(any("sourceview5/patch-manifest.json" in item for item in errors), errors)

    def test_rollback_gate_ignores_comments(self) -> None:
        errors: list[str] = []
        release._check_rollback_gate(
            "# TODO: rollback procedure\n# emergency contact\nFLATPAK_REPO_URL=example\nsummary\n",
            set(),
            errors,
        )
        self.assertTrue(any("version/ref check" in item for item in errors), errors)

    def test_current_release_workflow_satisfies_rollback_without_remediation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            _remove_remediation(root, "release.policy.json", "RIT-AUD-002")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertFalse(any("version/ref check" in item for item in errors), errors)

    def test_validate_workflow_mutable_inputs_are_release_critical(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            _remove_remediation(root, "release.policy.json", "RIT-AUD-011")
            workflow = root / ".github" / "workflows" / "validate.yml"
            workflow.write_text(
                workflow.read_text(encoding="utf-8").replace(
                    "cargo install cargo-llvm-cov --version 0.8.7 --locked",
                    "cargo install cargo-llvm-cov --locked",
                ),
                encoding="utf-8",
            )
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any(".github/workflows/validate.yml" in item for item in errors), errors)

    def test_pages_artifact_upload_path_must_be_exact_site(self) -> None:
        workflow = """
        find site -type l
        st_nlink > 1
        site/flatpak/repo/summary
        site/flatpak/repo/summary.sig
        uses: actions/upload-pages-artifact@v5
        with:
          path: site/flatpak/repo
        """
        errors: list[str] = []
        release._check_pages_artifact(workflow, errors)
        self.assertTrue(any("upload path site" in item for item in errors), errors)

    def test_key_pin_can_use_committed_key_basename(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write(root / "app" / "build-aux" / "flatpak" / "riteed-beta-public.asc", "key\n")
            _write(
                root / "app" / "build-aux" / "flatpak" / "README.md",
                "rotation revocation compromise emergency\n",
            )
            policy = foundation.release_policy(root)
            workflow = 'COMMITTED_KEY="riteed-beta-public.asc"\ngpg --export "$FLATPAK_GPG_KEY_ID" | cmp - "$COMMITTED_KEY"\n'
            errors: list[str] = []
            release._check_key_governance(root, policy, workflow, set(), errors)
            self.assertEqual(errors, [])

    def test_patch_manifest_validates_checksum_allowed_files_and_unsafe_count(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            patch_dir = root / "patch"
            upstream_root = root / "sourceview5-0.0.0"
            _write(upstream_root / "src" / "lib.rs", "fn demo() {}\n")
            archive = patch_dir / "upstream" / "sourceview5-0.0.0.crate"
            archive.parent.mkdir(parents=True, exist_ok=True)
            with tarfile.open(archive, "w:gz") as tar:
                tar.add(upstream_root, arcname="sourceview5-0.0.0")
            archive_digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            _write(patch_dir / "src" / "lib.rs", 'extern "C" {}\nunsafe fn demo() {}\n')
            _write(patch_dir / "extra.rs", "fn extra() {}\n")
            manifest = patch_dir / "patch-manifest.json"
            data = {
                "crate": "sourceview5",
                "version": "0.0.0",
                "upstream_source": {"crate_archive": "upstream/sourceview5-0.0.0.crate"},
                "upstream_crate_checksum": archive_digest,
                "allowed_changed_files": ["src/lib.rs"],
                "diff_checksum_sha256": "0" * 64,
                "unsafe_ffi_baseline": {"audited_total_matches": 1},
                "review_evidence": "review",
                "last_reviewed": "2026-05-25",
            }
            _write(manifest, json.dumps(data) + "\n")
            errors: list[str] = []
            release._validate_patch_manifest(manifest, {"required_manifest_fields": list(data)}, errors)
            self.assertTrue(any("unlisted local patch file extra.rs" in item for item in errors), errors)
            self.assertTrue(any("diff_checksum_sha256 mismatch" in item for item in errors), errors)
            self.assertTrue(any("unsafe_ffi_baseline mismatch" in item for item in errors), errors)

    def test_patch_crate_extraction_rejects_unsafe_archive_members(self) -> None:
        cases = [
            ("absolute path", "{outside}", tarfile.REGTYPE, "", "unsafe crate archive path"),
            (
                "parent traversal",
                "sourceview5-0.0.0/../escape.txt",
                tarfile.REGTYPE,
                "",
                "unsafe crate archive path",
            ),
            (
                "escaping traversal",
                "../sourceview5-escape.txt",
                tarfile.REGTYPE,
                "",
                "unsafe crate archive path",
            ),
            (
                "symlink",
                "sourceview5-0.0.0/link",
                tarfile.SYMTYPE,
                "{outside}",
                "unsupported member",
            ),
            (
                "hardlink",
                "sourceview5-0.0.0/hardlink",
                tarfile.LNKTYPE,
                "sourceview5-0.0.0/src/lib.rs",
                "unsupported member",
            ),
            ("special file", "sourceview5-0.0.0/fifo", tarfile.FIFOTYPE, "", "unsupported member"),
        ]
        for label, member_name, member_type, linkname, expected in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                archive = root / "sourceview5-0.0.0.crate"
                dest = root / "extract"
                dest.mkdir()
                outside = root / "sourceview5-escape.txt"
                resolved_name = member_name.replace("{outside}", outside.as_posix())
                resolved_linkname = linkname.replace("{outside}", outside.as_posix())
                info = tarfile.TarInfo(resolved_name)
                info.type = member_type
                info.linkname = resolved_linkname
                payload = b"fn demo() {}\n"
                with tarfile.open(archive, "w:gz") as tar:
                    if member_type == tarfile.REGTYPE:
                        info.size = len(payload)
                        tar.addfile(info, io.BytesIO(payload))
                    else:
                        tar.addfile(info)
                errors: list[str] = []
                self.assertFalse(release._extract_crate_safely(archive, dest, errors))
                self.assertTrue(any(expected in item for item in errors), errors)
                self.assertFalse(outside.exists(), errors)
                self.assertEqual(list(dest.rglob("*")), [], errors)

    def test_patch_crate_extraction_enforces_streaming_size_caps(self) -> None:
        original_member = release.CRATE_MEMBER_MAX_BYTES
        original_total = release.CRATE_TOTAL_MAX_BYTES
        try:
            release.CRATE_MEMBER_MAX_BYTES = 5
            release.CRATE_TOTAL_MAX_BYTES = 20
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                archive = root / "sourceview5-0.0.0.crate"
                dest = root / "extract"
                dest.mkdir()
                with tarfile.open(archive, "w:gz") as tar:
                    info = tarfile.TarInfo("sourceview5-0.0.0/large.rs")
                    payload = b"123456"
                    info.size = len(payload)
                    tar.addfile(info, io.BytesIO(payload))
                errors: list[str] = []
                self.assertFalse(release._extract_crate_safely(archive, dest, errors))
                self.assertTrue(any("exceeds 5 bytes" in item for item in errors), errors)

            release.CRATE_MEMBER_MAX_BYTES = 10
            release.CRATE_TOTAL_MAX_BYTES = 12
            with tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                archive = root / "sourceview5-0.0.0.crate"
                dest = root / "extract"
                dest.mkdir()
                with tarfile.open(archive, "w:gz") as tar:
                    for name in ("a.rs", "b.rs"):
                        info = tarfile.TarInfo(f"sourceview5-0.0.0/{name}")
                        payload = b"12345678"
                        info.size = len(payload)
                        tar.addfile(info, io.BytesIO(payload))
                errors = []
                self.assertFalse(release._extract_crate_safely(archive, dest, errors))
                self.assertTrue(any("exceeds 12 extracted bytes" in item for item in errors), errors)
        finally:
            release.CRATE_MEMBER_MAX_BYTES = original_member
            release.CRATE_TOTAL_MAX_BYTES = original_total

    def test_patch_crate_archive_path_rejects_absolute_and_parent_traversal(self) -> None:
        self.assertIsNone(release._safe_patch_relative_path("/tmp/sourceview5.crate"))
        self.assertIsNone(release._safe_patch_relative_path("../sourceview5.crate"))
        self.assertIsNone(release._safe_patch_relative_path("upstream/../../sourceview5.crate"))
        self.assertEqual(
            release._safe_patch_relative_path("upstream/sourceview5.crate"),
            Path("upstream/sourceview5.crate"),
        )

    def test_stress_policy_id_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            path = root / "policy" / "stress-fuzz.policy.json"
            data = json.loads(path.read_text(encoding="utf-8"))
            data["$id"] = "policy/wrong.json"
            path.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("policy/stress-fuzz.policy.json must have $id" in item for item in errors), errors)

    def test_current_stress_policy_satisfies_registry_and_fidelity_without_remediation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-015")
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertEqual(errors, [])

    def test_missing_parser_registry_requires_remediation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-015")
            (root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json").unlink()
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("Parser-boundary registry is required" in item for item in errors), errors)

    def test_git_seed_shape_requires_real_porcelain_v2_z_cases(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-015")
            corpus = root / "app" / "fuzz" / "corpus" / "git_status_parse"
            shutil.rmtree(corpus)
            _write(
                corpus / "newline-only.txt",
                b"1 .M N... 100644 100644 100644 abc def file.txt\n? untracked.txt\n",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("valid NUL-delimited porcelain" in item for item in errors), errors)

    def test_registry_must_cover_required_fuzz_targets_bidirectionally(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-015")
            registry = root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json"
            data = json.loads(registry.read_text(encoding="utf-8"))
            for entry in data["boundaries"]:
                entry["coverage"] = [
                    item for item in entry.get("coverage", []) if item.get("target") != "git_status_parse"
                ]
            registry.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(
                any("coverage missing required fuzz target git_status_parse" in item for item in errors),
                errors,
            )

    def test_registry_must_cover_required_stress_scripts_bidirectionally(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            registry = root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json"
            data = json.loads(registry.read_text(encoding="utf-8"))
            for entry in data["boundaries"]:
                entry["coverage"] = [
                    item for item in entry.get("coverage", []) if item.get("path") != "stress/scripts/compare-roundtrip.json"
                ]
            registry.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(
                any("coverage missing required stress script stress/scripts/compare-roundtrip.json" in item for item in errors),
                errors,
            )

    def test_malformed_parser_registry_entry_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _write(root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json", '[{"id":"only"}]\n')
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("missing required fields" in item for item in errors), errors)
            self.assertTrue(any("missing minimum id markdown_parse" in item for item in errors), errors)

    def test_absolute_registry_paths_are_rejected_without_escape_or_crash(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            self.assertFalse(stress_fuzz._path_or_glob_exists(root, "/etc/hosts"))
            self.assertFalse(stress_fuzz._path_or_glob_exists(root, "../outside"))

    def test_registry_gap_can_reference_active_planned_remediation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(root / "src" / "parser.rs", "fn parse() {}\n")
            _write(root / "app" / "fuzz" / "fuzz_targets" / "parser.rs", "fn main() {}\n")
            entry = {
                "id": "parser",
                "source_paths": ["src/parser.rs"],
                "coverage": [
                    {
                        "type": "fuzz",
                        "path": "app/fuzz/fuzz_targets/parser.rs",
                        "target": "parser",
                        "input_shape_asserted": "bytes",
                    }
                ],
                "gaps": [{"finding_id": "RIT-AUD-015", "case": "extra"}],
                "reviewed_exceptions": [],
                "last_reviewed": "2026-05-25",
            }
            errors: list[str] = []
            stress_fuzz._check_registry_paths(root, entry, "registry[0]", {"RIT-AUD-015"}, errors)
            self.assertFalse(any("non-empty gaps" in item for item in errors), errors)

    def test_git_status_seed_scan_caps_large_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            seed = root / "app" / "fuzz" / "corpus" / "git_status_parse" / "huge.bin"
            _write(seed, b"x" * (8 * 1024 * 1024 + 1))
            errors: list[str] = []
            self.assertFalse(stress_fuzz._corpus_has_git_status_shape(root, seed.parent, errors))
            self.assertTrue(any("too large" in item for item in errors), errors)

    def test_stress_script_schema_and_runner_fidelity_require_remediation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            _write(
                root / "stress" / "scripts" / "open-save-search.json",
                json.dumps(
                    {
                        "flow": "open-save-search",
                        "description": "shape only",
                        "expect_failure": False,
                    }
                )
                + "\n",
            )
            _write(
                root / "app" / "src" / "bin" / "riteed_stress.rs",
                "fn main() { let _path = std::env::temp_dir(); }\n",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("stress script missing required fields" in item for item in errors), errors)
            self.assertTrue(any("must consume declared/generated fixtures" in item for item in errors), errors)

    def test_stress_script_boundary_fidelity_requires_flow_signals(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            _write(
                root / "stress" / "scripts" / "compare-roundtrip.json",
                json.dumps(
                    {
                        "flow": "compare-roundtrip",
                        "description": "shape only",
                        "expect_failure": False,
                        "fixtures": [],
                        "actions": [],
                        "assertions": [],
                        "artifact_dir": "stress/artifacts",
                    }
                )
                + "\n",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must_start_compare_workflow" in item for item in errors), errors)

    def test_failure_artifacts_must_include_fuzz_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            workflow = root / ".github" / "workflows" / "validate.yml"
            workflow.write_text(
                workflow.read_text(encoding="utf-8").replace("            app/fuzz/artifacts/\n", ""),
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("failure artifact upload must include app/fuzz/artifacts/" in item for item in errors), errors)


if __name__ == "__main__":
    unittest.main()
