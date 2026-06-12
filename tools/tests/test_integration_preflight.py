from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


def _git(repo: Path, *args: str) -> None:
    subprocess.run(["git", *args], cwd=repo, check=True, capture_output=True, text=True)


def _init_repo(repo: Path) -> None:
    _git(repo, "init", "-b", "main")
    _git(repo, "config", "user.email", "test@example.invalid")
    _git(repo, "config", "user.name", "Test User")
    (repo / "README.md").write_text("base\n", encoding="utf-8")
    _git(repo, "add", "README.md")
    _git(repo, "commit", "-m", "base")


def _commit_branch(repo: Path, branch: str, filename: str) -> None:
    _git(repo, "switch", "-c", branch, "main")
    (repo / filename).write_text(f"{branch}\n", encoding="utf-8")
    _git(repo, "add", filename)
    _git(repo, "commit", "-m", branch)


def _run_preflight(repo: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["python3", "-m", "tools.checks.integration_preflight", "--repo", str(repo), *args],
        cwd=REPO_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


class IntegrationPreflightTests(unittest.TestCase):
    def test_main_passes_without_feature_branches(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo = Path(tmpdir)
            _init_repo(repo)
            result = _run_preflight(repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("branch: main", result.stdout)
        self.assertIn("build mode: integration", result.stdout)

    def test_feature_branch_fails_when_parallel_work_exists(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo = Path(tmpdir)
            _init_repo(repo)
            _commit_branch(repo, "feature/minimap", "minimap.txt")
            _commit_branch(repo, "feature/perf", "perf.txt")
            _git(repo, "switch", "feature/perf")
            result = _run_preflight(repo)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("feature/perf is not an integration build branch", result.stdout)
        self.assertIn("feature/minimap", result.stdout)

    def test_feature_only_override_passes_and_marks_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo = Path(tmpdir)
            _init_repo(repo)
            _commit_branch(repo, "feature/perf", "perf.txt")
            result = _run_preflight(repo, "--feature-only-ok")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("build mode: feature-only", result.stdout)
        self.assertIn("feature-only builds must be reported as partial", result.stdout)

    def test_integration_branch_passes_and_reports_unincluded_branches(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo = Path(tmpdir)
            _init_repo(repo)
            _commit_branch(repo, "feature/minimap", "minimap.txt")
            _commit_branch(repo, "feature/perf", "perf.txt")
            _git(repo, "switch", "-c", "integrate/current-work", "main")
            _git(repo, "merge", "--no-ff", "feature/minimap", "-m", "merge minimap")
            result = _run_preflight(repo)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("branch: integrate/current-work", result.stdout)
        self.assertIn("branches not included in current HEAD:", result.stdout)
        self.assertIn("feature/perf", result.stdout)


if __name__ == "__main__":
    unittest.main()
