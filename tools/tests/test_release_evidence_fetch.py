from __future__ import annotations

import copy
import contextlib
import http.client
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock
from typing import Any

from tools import release_evidence_fetch
from tools.tests.test_release_live_evidence import HEAD_SHA, REPO, _evidence, _policy


def _complete_policy() -> dict[str, Any]:
    policy = _policy()
    policy["signed_flatpak_publish"] = {
        "hard_requirements": {"required_check_app_slug": "github-actions"}
    }
    return policy


class ReleaseEvidenceFetchTests(unittest.TestCase):
    def test_cli_stderr_never_includes_a_malformed_token(self) -> None:
        secret = "synthetic-secret\nunexpected"

        class RejectingOpener:
            def open(self, request, timeout):
                del timeout
                connection = http.client.HTTPConnection("example.invalid")
                connection.putrequest("GET", "/")
                connection.putheader("Authorization", request.get_header("Authorization"))

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            policy_path = root / "policy.json"
            policy_path.write_text(json.dumps(_complete_policy()), encoding="utf-8")
            stderr = io.StringIO()
            with (
                mock.patch.object(release_evidence_fetch.github_api, "github_token", return_value=secret),
                mock.patch.object(
                    release_evidence_fetch.github_api.urllib.request,
                    "build_opener",
                    return_value=RejectingOpener(),
                ),
                contextlib.redirect_stderr(stderr),
            ):
                status = release_evidence_fetch.main(
                    [
                        "--repository",
                        REPO,
                        "--head-sha",
                        HEAD_SHA,
                        "--policy",
                        str(policy_path),
                        "--output",
                        str(root / "evidence.json"),
                    ]
                )
        self.assertEqual(status, 1)
        self.assertNotIn("synthetic-secret", stderr.getvalue())

    def test_cli_writes_only_validated_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            policy_path = root / "policy.json"
            output_path = root / "evidence.json"
            policy_path.write_text(json.dumps(_complete_policy()), encoding="utf-8")
            with (
                mock.patch.object(release_evidence_fetch.github_api, "github_token", return_value="token"),
                mock.patch.object(release_evidence_fetch, "collect", return_value=_evidence()),
            ):
                status = release_evidence_fetch.main(
                    [
                        "--repository",
                        REPO,
                        "--head-sha",
                        HEAD_SHA,
                        "--policy",
                        str(policy_path),
                        "--output",
                        str(output_path),
                    ]
                )
            self.assertEqual(status, 0)
            self.assertEqual(json.loads(output_path.read_text(encoding="utf-8")), _evidence())

            output_path.unlink()
            with (
                mock.patch.object(release_evidence_fetch.github_api, "github_token", return_value="token"),
                mock.patch.object(release_evidence_fetch, "collect", return_value=None),
            ):
                status = release_evidence_fetch.main(
                    [
                        "--repository",
                        REPO,
                        "--head-sha",
                        HEAD_SHA,
                        "--policy",
                        str(policy_path),
                        "--output",
                        str(output_path),
                    ]
                )
            self.assertEqual(status, 1)
            self.assertFalse(output_path.exists())

    def test_collector_builds_only_policy_repository_api_urls(self) -> None:
        evidence = _evidence(workflow_conclusion="failure")
        requested: list[str] = []

        def pages(url: str, _token: str, _errors: list[str], _label: str) -> Any:
            requested.append(url)
            return evidence["workflow_jobs"] if "/jobs?" in url else evidence["check_runs"]

        def item(url: str, _token: str, _errors: list[str], _label: str) -> Any:
            requested.append(url)
            return evidence["workflow_run"]

        errors: list[str] = []
        result = release_evidence_fetch.collect(
            repository=REPO,
            head_sha=HEAD_SHA,
            policy=_complete_policy(),
            token="secret",
            errors=errors,
            fetch_pages=pages,
            fetch_json=item,
        )

        self.assertEqual(errors, [])
        self.assertEqual(result, evidence)
        self.assertEqual(
            requested,
            [
                f"https://api.github.com/repos/{REPO}/commits/{HEAD_SHA}/check-runs?per_page=100&filter=all",
                f"https://api.github.com/repos/{REPO}/actions/runs/77",
                f"https://api.github.com/repos/{REPO}/actions/runs/77/jobs?per_page=100&filter=all",
            ],
        )

    def test_skipped_step_and_wrong_producer_fail_before_output(self) -> None:
        cases = {
            "skipped": ("workflow_jobs", "steps", "conclusion", "skipped"),
            "wrong producer": ("workflow_run", "event", "event", "pull_request"),
        }
        for label, (target, owner, field, value) in cases.items():
            with self.subTest(label=label):
                evidence = copy.deepcopy(_evidence())
                if target == "workflow_jobs":
                    evidence[target][0]["jobs"][0][owner][0][field] = value
                else:
                    evidence[target][field] = value
                errors: list[str] = []
                result = release_evidence_fetch.collect(
                    repository=REPO,
                    head_sha=HEAD_SHA,
                    policy=_complete_policy(),
                    token="secret",
                    errors=errors,
                    fetch_pages=lambda url, *_args: (
                        evidence["workflow_jobs"] if "/jobs?" in url else evidence["check_runs"]
                    ),
                    fetch_json=lambda *_args: evidence["workflow_run"],
                )
                self.assertIsNone(result)
                self.assertTrue(errors)

    def test_untrusted_payload_urls_are_not_fetched(self) -> None:
        evidence = _evidence()
        evidence["check_runs"][0]["check_runs"][0]["url"] = "https://evil.example/check"
        evidence["workflow_run"]["jobs_url"] = "https://evil.example/jobs"
        requested: list[str] = []

        def pages(url: str, *_args: Any) -> Any:
            requested.append(url)
            return evidence["workflow_jobs"] if "/jobs?" in url else evidence["check_runs"]

        def item(url: str, *_args: Any) -> Any:
            requested.append(url)
            return evidence["workflow_run"]

        errors: list[str] = []
        result = release_evidence_fetch.collect(
            repository=REPO,
            head_sha=HEAD_SHA,
            policy=_complete_policy(),
            token="secret",
            errors=errors,
            fetch_pages=pages,
            fetch_json=item,
        )
        self.assertIsNotNone(result)
        self.assertFalse(any("evil.example" in url for url in requested))


if __name__ == "__main__":
    unittest.main()
