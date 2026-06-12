from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path

from tools.checks import foundation, line_limits


REPO_ROOT = Path(__file__).resolve().parents[2]


def _write_lines(path: Path, count: int) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(f"line {index}\n" for index in range(count)), encoding="utf-8")


def _copy_policy(root: Path) -> None:
    shutil.copytree(REPO_ROOT / "policy", root / "policy")
    _set_waivers(root, [])


def _set_waivers(root: Path, waivers: list[dict[str, object]]) -> None:
    path = root / "policy" / "validation-tooling.policy.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    data["line_limit_waivers"] = waivers
    path.write_text(json.dumps(data, separators=(",", ":")) + "\n", encoding="utf-8")


def _waiver(**overrides: object) -> dict[str, object]:
    waiver: dict[str, object] = {
        "scope": "app",
        "path": "src/big.rs",
        "max_total_lines": 650,
        "reason": "Reviewed subsystem file that is capped against further growth.",
        "finding_id": "POLICY-LINE-LIMIT",
        "last_reviewed": "2026-01-01",
    }
    waiver.update(overrides)
    return waiver


class LineLimitTests(unittest.TestCase):
    def test_policy_declares_differentiated_thresholds_and_test_globs(self) -> None:
        policy = foundation.validation_policy(REPO_ROOT)
        thresholds = policy["thresholds"]
        self.assertEqual(thresholds["max_file_lines"], 600)
        self.assertEqual(thresholds["max_file_lines_test"], 800)
        self.assertEqual(thresholds["max_file_lines_waiver_cap"], 720)
        self.assertEqual(
            policy["line_limit_waiver_required_fields"],
            ["scope", "path", "max_total_lines", "reason", "finding_id", "last_reviewed"],
        )
        self.assertIn(
            {
                "scope": "app",
                "path": "src/bin/riteed_stress.rs",
                "max_total_lines": 620,
                "reason": "CI stress runner keeps flow execution, action-state waits, and failure artifact trail together so native-test failures remain diagnosable without splitting one reviewed driver.",
                "finding_id": "ci-native-tests-stress-save-action-race",
                "last_reviewed": "2026-06-12",
            },
            policy["line_limit_waivers"],
        )
        self.assertIn("tests/**/*.rs", policy["test_file_globs"])
        self.assertIn("**/gtk_tests*.rs", policy["test_file_globs"])
        self.assertIn("tools/tests/test_*.py", policy["test_file_globs"])

    def test_production_file_over_default_limit_requires_waiver(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write_lines(root / "src" / "big.rs", 601)
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="app")
        self.assertTrue(any("src/big.rs exceeds hard LOC limit 600" in item for item in errors), errors)

    def test_production_waiver_sets_frozen_file_limit(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write_lines(root / "src" / "big.rs", 650)
            _set_waivers(root, [_waiver(max_total_lines=650)])
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="app")
            self.assertEqual(errors, [])
            _write_lines(root / "src" / "big.rs", 651)
            line_limits.check_line_limits(root, errors, scope="app")
        self.assertTrue(any("src/big.rs exceeds waivered LOC limit 650" in item for item in errors), errors)

    def test_rust_test_files_have_eight_hundred_line_limit(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write_lines(root / "src" / "gtk_tests.rs", 800)
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="app")
            self.assertEqual(errors, [])
            _write_lines(root / "src" / "gtk_tests.rs", 801)
            line_limits.check_line_limits(root, errors, scope="app")
        self.assertTrue(any("src/gtk_tests.rs exceeds test LOC limit 800" in item for item in errors), errors)

    def test_policy_pack_python_tests_have_eight_hundred_line_limit(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write_lines(root / "tools" / "tests" / "test_x.py", 801)
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="policy-pack")
        self.assertTrue(any("tools/tests/test_x.py exceeds test LOC limit 800" in item for item in errors), errors)

    def test_waiver_cap_stale_and_test_file_waivers_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _write_lines(root / "src" / "big.rs", 600)
            _write_lines(root / "src" / "testing.rs", 650)
            _set_waivers(
                root,
                [
                    _waiver(max_total_lines=800),
                    _waiver(path="src/stale.rs", max_total_lines=650),
                    _waiver(path="src/testing.rs", max_total_lines=650),
                ],
            )
            _write_lines(root / "src" / "stale.rs", 600)
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="app")
        self.assertTrue(any("exceeds cap 720" in item for item in errors), errors)
        self.assertTrue(any("stale line-limit waiver" in item for item in errors), errors)
        self.assertTrue(any("waiver is not allowed for test files" in item for item in errors), errors)

    def test_waiver_required_fields_are_type_aware(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _set_waivers(
                root,
                [
                    {"scope": "app", "path": "", "max_total_lines": True, "reason": "", "last_reviewed": "bad"},
                ],
            )
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="app")
        self.assertTrue(any("missing required fields: finding_id" in item for item in errors), errors)
        self.assertTrue(any("path must be a non-empty string" in item for item in errors), errors)
        self.assertTrue(any("reason must be a non-empty string" in item for item in errors), errors)
        self.assertTrue(any("max_total_lines must be an integer" in item for item in errors), errors)
        self.assertTrue(any("last_reviewed must be YYYY-MM-DD" in item for item in errors), errors)

    def test_waiver_rejects_unsafe_or_out_of_scope_paths(self) -> None:
        cases = [
            (_waiver(path="/tmp/big.rs"), "path must be relative"),
            (_waiver(path="../big.rs"), "parent segments"),
            (_waiver(path="README.md"), "outside scoped files"),
        ]
        for waiver, expected in cases:
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_policy(root)
                _write_lines(root / "README.md", 650)
                _set_waivers(root, [waiver])
                errors: list[str] = []
                line_limits.check_line_limits(root, errors, scope="app")
                self.assertTrue(any(expected in item for item in errors), errors)

    def test_future_review_date_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_policy(root)
            _set_waivers(root, [_waiver(last_reviewed="2099-01-01")])
            errors: list[str] = []
            line_limits.check_line_limits(root, errors, scope="app")
        self.assertTrue(any("last_reviewed must not be after" in item for item in errors), errors)

    def test_invalid_scope_fails_in_both_gates(self) -> None:
        for scope in ("app", "policy-pack"):
            with self.subTest(scope=scope), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_policy(root)
                _set_waivers(root, [_waiver(scope="other")])
                errors: list[str] = []
                line_limits.check_line_limits(root, errors, scope=scope)
                self.assertTrue(any("scope must be one of app, policy-pack" in item for item in errors), errors)

    def test_missing_scope_fails_in_both_gates(self) -> None:
        for scope in ("app", "policy-pack"):
            with self.subTest(scope=scope), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_policy(root)
                waiver = _waiver()
                del waiver["scope"]
                _set_waivers(root, [waiver])
                errors: list[str] = []
                line_limits.check_line_limits(root, errors, scope=scope)
                self.assertTrue(any("missing required fields: scope" in item for item in errors), errors)

    def test_other_scope_waiver_is_not_checked_against_this_scope_files(self) -> None:
        cases = [
            ("policy-pack", _waiver(scope="app", path="src/big.rs", max_total_lines=650)),
            ("app", _waiver(scope="policy-pack", path="tools/checks/big.py", max_total_lines=650)),
        ]
        for scope, waiver in cases:
            with self.subTest(scope=scope), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_policy(root)
                _set_waivers(root, [waiver])
                errors: list[str] = []
                line_limits.check_line_limits(root, errors, scope=scope)
                self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
