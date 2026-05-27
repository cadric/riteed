from __future__ import annotations

import os
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.validation_tooling import run_capture


REPO_ROOT = Path(__file__).resolve().parents[2]


class ValidationToolingSecurityTests(unittest.TestCase):
    def test_run_capture_strips_ambient_github_tokens(self) -> None:
        script = (
            "import os; "
            "print(os.environ.get('GITHUB_TOKEN'), "
            "os.environ.get('GH_TOKEN'), "
            "os.environ.get('RITEED_TEST'))"
        )
        with patch.dict(os.environ, {"GITHUB_TOKEN": "ambient", "GH_TOKEN": "ambient-gh"}):
            result = run_capture(["python3", "-c", script], REPO_ROOT, env={"RITEED_TEST": "ok"})
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout.strip(), "None None ok")

    def test_run_capture_keeps_explicit_token_env(self) -> None:
        script = "import os; print(os.environ.get('GITHUB_TOKEN'))"
        with patch.dict(os.environ, {"GITHUB_TOKEN": "ambient"}):
            result = run_capture(["python3", "-c", script], REPO_ROOT, env={"GITHUB_TOKEN": "explicit"})
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout.strip(), "explicit")


if __name__ == "__main__":
    unittest.main()
