from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.checks import stress_fuzz
from tools.tests.test_release_stress_policy import _copy_stress_context, _remove_remediation, _write


class StressFuzzHardeningTests(unittest.TestCase):
    def test_parser_boundary_markers_are_bidirectional(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-015")
            parser = root / "app" / "src" / "markdown" / "parser.rs"
            parser.write_text(
                parser.read_text(encoding="utf-8").replace("// PARSER-BOUNDARY: id=markdown_parse\n", ""),
                encoding="utf-8",
            )
            unsupported = root / "app" / "src" / "markdown" / "unsupported.rs"
            unsupported.write_text(
                unsupported.read_text(encoding="utf-8") + "// PARSER-BOUNDARY: id=unregistered_parser\n",
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("markdown_parse must have PARSER-BOUNDARY marker" in item for item in errors), errors)
            self.assertTrue(any("unregistered_parser is not registered" in item for item in errors), errors)

    def test_stress_fidelity_ignores_role_tokens_without_executable_actions(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            _write(
                root / "stress" / "scripts" / "compare-roundtrip.json",
                json.dumps(
                    {
                        "flow": "compare-roundtrip",
                        "description": "role tokens are not actions",
                        "expect_failure": False,
                        "fixtures": [
                            {"role": "compare reference", "path": "stress/corpus/generated/compare-reference.txt"},
                            {"role": "compare current", "path": "stress/corpus/generated/compare-current.txt"},
                        ],
                        "actions": [{"type": "open", "path": "stress/corpus/generated/compare-current.txt"}],
                        "assertions": [{"type": "compare-pane-diff-state", "state": "diff-visible"}],
                        "artifact_dir": "stress/artifacts/compare-roundtrip",
                    }
                )
                + "\n",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must_start_compare_workflow" in item for item in errors), errors)

    def test_stress_artifact_dir_rejects_parent_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            script = root / "stress" / "scripts" / "open-save-search.json"
            data = json.loads(script.read_text(encoding="utf-8"))
            data["artifact_dir"] = "stress/artifacts/../../outside"
            script.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("artifact_dir must be under stress/artifacts/" in item for item in errors), errors)

    def test_future_registry_review_dates_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            registry = root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json"
            data = json.loads(registry.read_text(encoding="utf-8"))
            data["boundaries"][0]["last_reviewed"] = "2099-01-01"
            data["boundaries"][0]["reviewed_exceptions"] = [
                {
                    "case": "future",
                    "reason": "future",
                    "review_artifact": "docs/audit_report.md",
                    "last_reviewed": "2099-01-01",
                }
            ]
            registry.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("last_reviewed must not be after" in item for item in errors), errors)


if __name__ == "__main__":
    unittest.main()
