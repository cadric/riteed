from __future__ import annotations

import copy
import unittest
from typing import Any

from tools.checks import release_evidence


HEAD_SHA = "0123456789abcdef0123456789abcdef01234567"
REPO = "cadric/riteed"
CHECK_ID = 700
JOB_ID = 900
RUN_ID = 77


def _policy() -> dict[str, Any]:
    return {
        "release_identity": {"repository_full_name": REPO},
        "github_actions_release_safety": {
            "repository_governance": {
                "truthful_checks": {
                    "live_context": "governance-live",
                    "live_allowed_events": ["push", "schedule", "workflow_dispatch"],
                    "live_workflow_name": "Validate",
                    "live_workflow_path": ".github/workflows/validate.yml",
                    "live_job": "governance-live",
                    "live_decisive_step": "Verify GitHub ruleset governance",
                    "main_branch": "main",
                }
            }
        },
    }


def _check(
    *,
    check_id: int = CHECK_ID,
    status: str = "completed",
    conclusion: str | None = "success",
) -> dict[str, Any]:
    return {
        "id": check_id,
        "name": "governance-live",
        "head_sha": HEAD_SHA,
        "status": status,
        "conclusion": conclusion,
        "app": {"slug": "github-actions"},
        "details_url": f"https://github.com/{REPO}/actions/runs/{RUN_ID}/job/{JOB_ID}",
    }


def _evidence(*, workflow_conclusion: str = "success") -> dict[str, Any]:
    return {
        "check_runs": [{"total_count": 1, "check_runs": [_check()]}],
        "workflow_run": {
            "id": RUN_ID,
            "name": "Validate",
            "path": ".github/workflows/validate.yml",
            "event": "push",
            "status": "completed",
            "conclusion": workflow_conclusion,
            "head_sha": HEAD_SHA,
            "head_branch": "main",
            "repository": {"full_name": REPO},
            "head_repository": {"full_name": REPO},
        },
        "workflow_jobs": [
            {
                "total_count": 1,
                "jobs": [
                    {
                        "id": JOB_ID,
                        "run_id": RUN_ID,
                        "name": "governance-live",
                        "head_sha": HEAD_SHA,
                        "status": "completed",
                        "conclusion": "success",
                        "check_run_url": f"https://api.github.com/repos/{REPO}/check-runs/{CHECK_ID}",
                        "html_url": f"https://github.com/{REPO}/actions/runs/{RUN_ID}/job/{JOB_ID}",
                        "steps": [
                            {
                                "name": "Verify GitHub ruleset governance",
                                "status": "completed",
                                "conclusion": "success",
                            }
                        ],
                    }
                ],
            }
        ],
    }


def _errors(evidence: dict[str, Any]) -> list[str]:
    return release_evidence.check_live_governance(
        evidence,
        policy=_policy(),
        head_sha=HEAD_SHA,
        app_slug="github-actions",
    )


class ReleaseLiveEvidenceTests(unittest.TestCase):
    def test_distinct_job_and_check_ids_and_failed_aggregate_are_accepted(self) -> None:
        self.assertNotEqual(JOB_ID, CHECK_ID)
        self.assertEqual(_errors(_evidence(workflow_conclusion="failure")), [])

    def test_decisive_step_must_be_unique_completed_success(self) -> None:
        cases: dict[str, Any] = {
            "missing": [],
            "skipped": [{"name": "Verify GitHub ruleset governance", "status": "completed", "conclusion": "skipped"}],
            "neutral": [{"name": "Verify GitHub ruleset governance", "status": "completed", "conclusion": "neutral"}],
            "failed": [{"name": "Verify GitHub ruleset governance", "status": "completed", "conclusion": "failure"}],
            "duplicate": [
                {"name": "Verify GitHub ruleset governance", "status": "completed", "conclusion": "success"},
                {"name": "Verify GitHub ruleset governance", "status": "completed", "conclusion": "success"},
            ],
        }
        for label, steps in cases.items():
            with self.subTest(label=label):
                evidence = _evidence()
                evidence["workflow_jobs"][0]["jobs"][0]["steps"] = steps
                self.assertTrue(_errors(evidence))

    def test_live_job_and_check_must_be_completed_success(self) -> None:
        for target, value in (
            (("check_runs", 0, "check_runs", 0, "conclusion"), "neutral"),
            (("workflow_jobs", 0, "jobs", 0, "conclusion"), "failure"),
            (("workflow_jobs", 0, "jobs", 0, "status"), "in_progress"),
        ):
            with self.subTest(target=target):
                evidence = _evidence()
                owner: Any = evidence
                for key in target[:-1]:
                    owner = owner[key]
                owner[target[-1]] = value
                self.assertTrue(_errors(evidence))

    def test_producer_identity_fields_are_exact(self) -> None:
        mutations = {
            "run id": ("id", RUN_ID + 1),
            "workflow": ("name", "Other"),
            "path": ("path", ".github/workflows/other.yml"),
            "event": ("event", "pull_request"),
            "sha": ("head_sha", "f" * 40),
            "branch": ("head_branch", "release"),
            "repository": ("repository", {"full_name": "other/repo"}),
            "head repository": ("head_repository", {"full_name": "fork/repo"}),
        }
        for label, (field, value) in mutations.items():
            with self.subTest(label=label):
                evidence = _evidence()
                evidence["workflow_run"][field] = value
                self.assertTrue(_errors(evidence))

    def test_numeric_producer_ids_require_strict_integers(self) -> None:
        for label, target, value in (
            ("run bool", ("workflow_run", "id"), True),
            ("run float", ("workflow_run", "id"), float(RUN_ID)),
            ("job run bool", ("workflow_jobs", 0, "jobs", 0, "run_id"), True),
            ("job run float", ("workflow_jobs", 0, "jobs", 0, "run_id"), float(RUN_ID)),
        ):
            with self.subTest(label=label):
                evidence = _evidence()
                owner: Any = evidence
                for key in target[:-1]:
                    owner = owner[key]
                owner[target[-1]] = value
                self.assertTrue(_errors(evidence))

    def test_policy_strings_and_job_sha_require_exact_strings(self) -> None:
        policy = _policy()
        policy["github_actions_release_safety"]["repository_governance"]["truthful_checks"][
            "live_job"
        ] = None
        errors = release_evidence.check_live_governance(
            _evidence(), policy=policy, head_sha=HEAD_SHA, app_slug="github-actions"
        )
        self.assertTrue(errors)

        evidence = _evidence()
        evidence["workflow_jobs"][0]["jobs"][0]["head_sha"] = None
        self.assertTrue(_errors(evidence))

    def test_job_urls_bind_selected_check_and_details(self) -> None:
        for field, value in (
            ("check_run_url", f"https://api.github.com/repos/{REPO}/check-runs/{CHECK_ID + 1}"),
            ("html_url", f"https://github.com/{REPO}/actions/runs/{RUN_ID}/job/{JOB_ID + 1}"),
            ("run_id", RUN_ID + 1),
        ):
            with self.subTest(field=field):
                evidence = _evidence()
                evidence["workflow_jobs"][0]["jobs"][0][field] = value
                self.assertTrue(_errors(evidence))

    def test_newest_matching_check_cannot_fall_back_to_old_success(self) -> None:
        evidence = _evidence()
        old = copy.deepcopy(evidence["check_runs"][0]["check_runs"][0])
        old["id"] = CHECK_ID - 1
        newer = copy.deepcopy(old)
        newer["id"] = CHECK_ID
        newer["conclusion"] = "failure"
        evidence["check_runs"] = [{"total_count": 2, "check_runs": [old, newer]}]

        errors = _errors(evidence)

        self.assertTrue(any("newest" in error.lower() or "failure" in error.lower() for error in errors), errors)

    def test_check_and_job_pagination_must_be_complete_and_stable(self) -> None:
        cases = []
        missing_check = _evidence()
        missing_check["check_runs"][0]["total_count"] = 2
        cases.append(missing_check)
        missing_job = _evidence()
        missing_job["workflow_jobs"][0]["total_count"] = 2
        cases.append(missing_job)
        duplicate_job = _evidence()
        job = duplicate_job["workflow_jobs"][0]["jobs"][0]
        duplicate_job["workflow_jobs"] = [
            {"total_count": 2, "jobs": [job]},
            {"total_count": 2, "jobs": [copy.deepcopy(job)]},
        ]
        cases.append(duplicate_job)
        changed_count = _evidence()
        changed_count["check_runs"] = [
            {"total_count": 2, "check_runs": [_check(check_id=699)]},
            {"total_count": 1, "check_runs": [_check()]},
        ]
        cases.append(changed_count)
        for index, evidence in enumerate(cases):
            with self.subTest(index=index):
                self.assertTrue(_errors(evidence))


if __name__ == "__main__":
    unittest.main()
