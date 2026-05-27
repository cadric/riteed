from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.checks import release, release_workflow
from tools.tests.test_release_stress_policy import _copy_release_context


REPO_ROOT = Path(__file__).resolve().parents[2]


class ReleaseHardeningTests(unittest.TestCase):
    def test_policy_check_release_path_is_offline_without_github_auth(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertFalse(any("GitHub ruleset governance requires" in item for item in errors), errors)
            self.assertFalse(any("GitHub rollback environment governance requires" in item for item in errors), errors)

    def test_existing_repo_workflows_pass_all_structural_checks(self) -> None:
        for path in sorted((REPO_ROOT / ".github" / "workflows").glob("*.yml")):
            with self.subTest(path=path.name):
                errors: list[str] = []
                workflow = release_workflow.parse(path.relative_to(REPO_ROOT).as_posix(), path.read_text(encoding="utf-8"), errors)
                self.assertEqual(errors, [])
                self.assertIsNotNone(workflow)
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            errors = []
            release.check_release(root, errors)
            self.assertEqual(errors, [])

    def test_ruleset_governance_rejects_ambient_github_token(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            workflow_path = root / ".github" / "workflows" / "validate.yml"
            workflow = workflow_path.read_text(encoding="utf-8").replace(
                "secrets.RULESET_GOVERNANCE_TOKEN",
                "github.token",
            )
            workflow_path.write_text(workflow, encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("RULESET_GOVERNANCE_TOKEN" in item for item in errors), errors)

    def test_publish_required_checks_must_exactly_match_policy(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            policy = json.loads((root / "policy" / "release.policy.json").read_text(encoding="utf-8"))
            required = policy["signed_flatpak_publish"]["hard_requirements"]["required_validate_check_contexts"]
            workflow_path = root / ".github" / "workflows" / "publish-flatpak.yml"
            original = workflow_path.read_text(encoding="utf-8")
            for check in required:
                with self.subTest(check=check):
                    workflow_path.write_text(original.replace(f"            {check}\n", ""), encoding="utf-8")
                    errors: list[str] = []
                    release.check_release(root, errors)
                    self.assertTrue(any("required_checks must exactly match" in item for item in errors), errors)
                    workflow_path.write_text(original, encoding="utf-8")

    def test_validation_gate_rejects_tail_latest_check_run_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            workflow_path = root / ".github" / "workflows" / "publish-flatpak.yml"
            workflow = workflow_path.read_text(encoding="utf-8")
            workflow = workflow.replace('run.get("head_sha") != tag_commit', 'run.get("sha") != tag_commit')
            workflow = workflow.replace('run.get("completed_at") or ""', '"tail -n 1"')
            workflow_path.write_text(workflow, encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("exact-commit validation gate" in item for item in errors), errors)

    def test_preflight_with_only_trivial_tokens_does_not_satisfy_validation_gate(self) -> None:
        policy = {
            "signed_flatpak_publish": {
                "hard_requirements": {
                    "required_validate_check_contexts": ["policy-pack"],
                    "release_critical_validation_suite": [],
                }
            }
        }
        workflow = """
on:
  push:
    tags:
      - "v*"
jobs:
  preflight:
    runs-on: ubuntu-latest
    steps:
      - name: Trivial gate
        run: |
          required_checks=(
            policy-pack
          )
          tag_commit="$(git rev-parse HEAD)"
          gh api user
  build:
    needs: preflight
    runs-on: ubuntu-latest
    environment:
      name: flatpak-beta-signing
    steps:
      - name: Sign
        env:
          FLATPAK_GPG_PRIVATE_KEY: ${{ secrets.FLATPAK_GPG_PRIVATE_KEY }}
        run: echo signing
"""
        self.assertFalse(release_workflow.has_validation_before_secret(policy, workflow))

    def test_pages_permission_at_workflow_root_with_environment_on_decoy_job_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            workflow_path = root / ".github" / "workflows" / "publish-flatpak.yml"
            workflow = workflow_path.read_text(encoding="utf-8")
            workflow = workflow.replace(
                "permissions:\n  contents: read\n",
                "permissions:\n  contents: read\n  pages: write\n  id-token: write\n",
                1,
            )
            workflow = workflow.replace(
                "  deploy:\n",
                "  deploy-decoy:\n    runs-on: ubuntu-latest\n    environment:\n      name: github-pages\n    steps:\n      - run: echo noop\n\n  deploy:\n",
                1,
            )
            workflow_path.write_text(workflow, encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("pages: write must stay scoped" in item for item in errors), errors)
            self.assertTrue(any("id-token: write must stay scoped" in item for item in errors), errors)

    def test_rollback_gate_requires_environment_route_and_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            workflow_path = root / ".github" / "workflows" / "publish-flatpak.yml"
            workflow = workflow_path.read_text(encoding="utf-8").replace("--rollback-reason", "--rollback-note")
            workflow_path.write_text(workflow, encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("version/ref check and explicit rollback path" in item for item in errors), errors)

    def test_rollback_environment_policy_requires_reviewed_reviewer_identity(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / "policy" / "release.policy.json"
            policy = json.loads(path.read_text(encoding="utf-8"))
            policy["github_actions_release_safety"]["rollback_environment"]["reviewed_required_reviewers"] = []
            path.write_text(json.dumps(policy, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            self.assertTrue(any("reviewed_required_reviewers is required" in item for item in errors), errors)

    def test_multiline_curl_pipe_installer_is_forbidden(self) -> None:
        errors: list[str] = []
        release._check_mutable_inputs(
            {
                "workflow.yml": 'curl --proto "=https" --tlsv1.2 -sSf \\\n                  https://example.invalid/install.sh | sh\n'
            },
            set(),
            errors,
        )
        self.assertTrue(any("actions/tool installers must be pinned" in item for item in errors), errors)


if __name__ == "__main__":
    unittest.main()
