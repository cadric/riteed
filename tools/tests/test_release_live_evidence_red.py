import contextlib
import copy
import io
import json
from pathlib import Path
import tempfile
import unittest

from tools import release_check_runs
from tools.checks import release
from tools.tests.test_release_stress_policy import _copy_release_context
from tools.tests.test_release_live_evidence import HEAD_SHA, _evidence, _policy


class ReleaseLiveEvidenceDecisionTests(unittest.TestCase):
    def test_release_policy_cannot_disable_live_publish_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            policy_path = root / "policy" / "release.policy.json"
            policy = json.loads(policy_path.read_text(encoding="utf-8"))
            policy["signed_flatpak_publish"]["hard_requirements"][
                "required_live_governance_check_for_publish"
            ] = False
            policy_path.write_text(json.dumps(policy), encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
        self.assertTrue(
            any("required_live_governance_check_for_publish" in error for error in errors),
            errors,
        )

    def test_required_pr_context_cannot_reuse_old_live_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            policy_path = root / "policy" / "release.policy.json"
            policy = json.loads(policy_path.read_text(encoding="utf-8"))
            contexts = policy["signed_flatpak_publish"]["hard_requirements"][
                "required_validate_check_contexts"
            ]
            contexts[contexts.index("governance-static")] = "ruleset-governance"
            policy_path.write_text(json.dumps(policy), encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
        self.assertTrue(any("truthful_checks" in error for error in errors), errors)

    def _status(self, evidence: dict, policy: dict) -> tuple[int, str]:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            evidence_path = root / "evidence.json"
            policy_path = root / "policy.json"
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            policy_path.write_text(json.dumps(policy), encoding="utf-8")
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr):
                status = release_check_runs.main(
                    [
                        "--input",
                        str(evidence_path),
                        "--policy",
                        str(policy_path),
                        "--head-sha",
                        HEAD_SHA,
                    ]
                )
            return status, stderr.getvalue()

    def _complete_policy(self) -> dict:
        policy = _policy()
        policy["signed_flatpak_publish"] = {
            "hard_requirements": {
                "required_validate_check_contexts": ["governance-live"],
                "required_check_app_slug": "github-actions",
                "required_live_governance_check_for_publish": True,
            }
        }
        return policy

    def test_complete_valid_live_evidence_is_accepted(self) -> None:
        status, stderr = self._status(_evidence(workflow_conclusion="failure"), self._complete_policy())
        self.assertEqual(status, 0, stderr)

    def test_green_check_without_successful_decisive_step_is_rejected(self) -> None:
        evidence = _evidence()
        evidence["workflow_jobs"][0]["jobs"][0]["steps"][0]["conclusion"] = "skipped"
        status, stderr = self._status(evidence, self._complete_policy())
        self.assertEqual(status, 1)
        self.assertIn("decisive step", stderr)

    def test_wrong_producer_does_not_fall_back_to_old_success(self) -> None:
        evidence = _evidence()
        old = copy.deepcopy(evidence["check_runs"][0]["check_runs"][0])
        old["id"] -= 1
        newest = evidence["check_runs"][0]["check_runs"][0]
        evidence["check_runs"][0] = {"total_count": 2, "check_runs": [old, newest]}
        evidence["workflow_run"]["event"] = "pull_request"
        status, stderr = self._status(evidence, self._complete_policy())
        self.assertEqual(status, 1)
        self.assertIn("event", stderr.lower())


if __name__ == "__main__":
    unittest.main()
