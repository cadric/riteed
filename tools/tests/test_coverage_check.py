from __future__ import annotations

import json
import os
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

from tools import coverage_check


class CoverageCheckTests(unittest.TestCase):
    def test_main_passes_headless_gtk_environment_to_llvm_cov(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            captured: dict[str, object] = {}

            def fake_run_checked(
                cmd: list[str],
                cwd: Path,
                label: str | None = None,
                env: dict[str, str] | None = None,
            ) -> str:
                captured["cmd"] = cmd
                captured["cwd"] = cwd
                captured["label"] = label
                captured["env"] = env
                Path(cmd[-1]).write_text(
                    json.dumps({"totals": {"lines": {"percent": 100.0}}}),
                    encoding="utf-8",
                )
                return ""

            with patch.object(coverage_check, "parse_args", return_value=Namespace(root=str(root), json_summary=None)):
                with patch.object(coverage_check, "repo_root", return_value=root):
                    with patch.object(
                        coverage_check,
                        "validation_policy",
                        return_value={
                            "thresholds": {"min_line_coverage_percent": 80.0},
                            "coverage_validation": {
                                "required_tools": ["cargo", "cargo-llvm-cov"],
                                "default_command": [
                                    "cargo",
                                    "llvm-cov",
                                    "--workspace",
                                    "--all-features",
                                    "--json",
                                    "--summary-only",
                                    "--output-path",
                                    "<output-path>",
                                ],
                            },
                        },
                    ):
                        with patch.object(coverage_check, "require_tool"):
                            with patch.object(coverage_check, "run_checked", side_effect=fake_run_checked):
                                self.assertEqual(coverage_check.main(), 0)

            self.assertEqual(captured["cwd"], root)
            self.assertEqual(captured["label"], "cargo llvm-cov failed")
            self.assertEqual(
                captured["env"],
                {
                    "GSK_RENDERER": os.environ.get("GSK_RENDERER", "cairo"),
                    "GTK_A11Y": os.environ.get("GTK_A11Y", "none"),
                },
            )


if __name__ == "__main__":
    unittest.main()
