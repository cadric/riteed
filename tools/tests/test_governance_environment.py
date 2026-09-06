from __future__ import annotations

import contextlib
import copy
import io
import unittest
from unittest import mock
from typing import Any

from tools import ruleset_governance_check
from tools.checks import governance_environment


def _policy() -> dict[str, Any]:
    return {
        "release_identity": {"repository_full_name": "cadric/riteed"},
        "github_actions_release_safety": {
            "repository_governance": {
                "truthful_checks": {
                    "live_environment": "ruleset-governance-live",
                    "live_secret": "RULESET_GOVERNANCE_TOKEN",
                    "environment_branch_policy": {
                        "protected_branches": False,
                        "custom_branch_policies": True,
                        "name": "main",
                        "type": "branch",
                    },
                    "repository_secret_forbidden": True,
                }
            }
        },
    }


def _environment() -> dict[str, Any]:
    return {
        "name": "ruleset-governance-live",
        "deployment_branch_policy": {
            "protected_branches": False,
            "custom_branch_policies": True,
        },
    }


def _pages(key: str, values: list[dict[str, Any]]) -> list[dict[str, Any]]:
    return [{"total_count": len(values), key: values}]


def _errors(
    environment: dict[str, Any] | None = None,
    branch_pages: list[dict[str, Any]] | None = None,
    repo_secret_pages: list[dict[str, Any]] | None = None,
    env_secret_pages: list[dict[str, Any]] | None = None,
) -> list[str]:
    errors: list[str] = []
    governance_environment.check_payloads(
        _environment() if environment is None else environment,
        _pages("branch_policies", [{"id": 1, "name": "main", "type": "branch"}])
        if branch_pages is None
        else branch_pages,
        _pages("secrets", []) if repo_secret_pages is None else repo_secret_pages,
        _pages("secrets", [{"name": "RULESET_GOVERNANCE_TOKEN"}])
        if env_secret_pages is None
        else env_secret_pages,
        _policy(),
        errors,
    )
    return errors


class GovernanceEnvironmentTests(unittest.TestCase):
    def test_configured_secret_identifier_is_not_emitted_in_diagnostics(self) -> None:
        sentinel = "SENSITIVE_POLICY_IDENTIFIER_SENTINEL"
        policy = _policy()
        policy["github_actions_release_safety"]["repository_governance"][
            "truthful_checks"
        ]["live_secret"] = sentinel
        repo_copy = _pages("secrets", [{"name": sentinel}])
        missing_env = _pages("secrets", [])

        errors: list[str] = []
        governance_environment.check_payloads(
            _environment(),
            _pages("branch_policies", [{"id": 1, "name": "main", "type": "branch"}]),
            repo_copy,
            missing_env,
            policy,
            errors,
        )
        self.assertEqual(len(errors), 2, errors)
        self.assertNotIn(sentinel, "\n".join(errors))

        def fetch_pages(url: str, *_args: Any) -> Any:
            if url.endswith("deployment-branch-policies?per_page=100"):
                return _pages(
                    "branch_policies", [{"id": 1, "name": "main", "type": "branch"}]
                )
            if "/environments/" in url:
                return missing_env
            return repo_copy

        output = io.StringIO()
        with (
            mock.patch.object(
                ruleset_governance_check.foundation, "release_policy", return_value=policy
            ),
            mock.patch.object(
                ruleset_governance_check.remediation,
                "validate_planned_remediation",
                return_value=set(),
            ),
            mock.patch.object(ruleset_governance_check.rulesets, "check_ruleset_governance"),
            mock.patch.object(
                ruleset_governance_check.rulesets,
                "check_rollback_environment_governance",
            ),
            mock.patch.object(governance_environment.github_api, "github_token", return_value="token"),
            mock.patch.object(
                governance_environment.github_api, "fetch_json", return_value=_environment()
            ),
            mock.patch.object(
                governance_environment.github_api, "fetch_pages", side_effect=fetch_pages
            ),
            contextlib.redirect_stdout(output),
        ):
            self.assertEqual(ruleset_governance_check.main(), 1)
        self.assertIn("absent at repository scope", output.getvalue())
        self.assertIn("exactly once at environment scope", output.getvalue())
        self.assertNotIn(sentinel, output.getvalue())

    def test_remote_check_requires_a_token(self) -> None:
        with mock.patch.object(governance_environment.github_api, "github_token", return_value=None):
            errors: list[str] = []
            governance_environment.check_remote(_policy(), errors)
        self.assertTrue(any("requires GITHUB_TOKEN" in error for error in errors), errors)

    def test_remote_check_fetches_all_reviewed_metadata_endpoints(self) -> None:
        calls: list[str] = []

        def fetch_json(url: str, token: str, _errors: list[str], _label: str) -> Any:
            calls.append(url)
            self.assertEqual(token, "token")
            return _environment()

        def fetch_pages(url: str, token: str, _errors: list[str], _label: str) -> Any:
            calls.append(url)
            self.assertEqual(token, "token")
            if url.endswith("deployment-branch-policies?per_page=100"):
                return _pages("branch_policies", [{"id": 1, "name": "main", "type": "branch"}])
            if "/environments/" in url:
                return _pages("secrets", [{"name": "RULESET_GOVERNANCE_TOKEN"}])
            return _pages("secrets", [])

        with (
            mock.patch.object(governance_environment.github_api, "github_token", return_value="token"),
            mock.patch.object(governance_environment.github_api, "fetch_json", side_effect=fetch_json),
            mock.patch.object(governance_environment.github_api, "fetch_pages", side_effect=fetch_pages),
        ):
            errors: list[str] = []
            governance_environment.check_remote(_policy(), errors)
        self.assertEqual(errors, [])
        self.assertEqual(len(calls), 4)

    def test_remote_environment_payload_must_be_an_object(self) -> None:
        with (
            mock.patch.object(governance_environment.github_api, "github_token", return_value="token"),
            mock.patch.object(governance_environment.github_api, "fetch_json", return_value=[]),
            mock.patch.object(
                governance_environment.github_api,
                "fetch_pages",
                return_value=[{"total_count": 0, "branch_policies": []}],
            ),
        ):
            errors: list[str] = []
            governance_environment.check_remote(_policy(), errors)
        self.assertTrue(errors)

    def test_actual_fetch_json_null_environment_fails_closed(self) -> None:
        def pages(url: str, *_args: Any) -> Any:
            if url.endswith("deployment-branch-policies?per_page=100"):
                return _pages("branch_policies", [{"id": 1, "name": "main", "type": "branch"}])
            if "/environments/" in url:
                return _pages("secrets", [{"name": "RULESET_GOVERNANCE_TOKEN"}])
            return _pages("secrets", [{"name": "RULESET_GOVERNANCE_TOKEN"}])

        with (
            mock.patch.object(governance_environment.github_api, "github_token", return_value="token"),
            mock.patch.object(
                governance_environment.github_api,
                "_request_page",
                return_value=(None, ""),
            ),
            mock.patch.object(
                governance_environment.github_api,
                "fetch_pages",
                side_effect=pages,
            ),
        ):
            errors: list[str] = []
            governance_environment.check_remote(_policy(), errors)
        self.assertTrue(any("null payload" in error for error in errors), errors)

    def test_exact_main_environment_secret_scope_passes(self) -> None:
        self.assertEqual(_errors(), [])

    def test_environment_protection_and_branch_policy_are_exact(self) -> None:
        cases: list[tuple[dict[str, Any], list[dict[str, Any]]]] = []
        environment = _environment()
        environment["deployment_branch_policy"]["protected_branches"] = True
        cases.append((environment, _pages("branch_policies", [{"id": 1, "name": "main", "type": "branch"}])))
        cases.append((_environment(), _pages("branch_policies", [])))
        cases.append((_environment(), _pages("branch_policies", [{"id": 1, "name": "refs/heads/main", "type": "branch"}])))
        cases.append((_environment(), _pages("branch_policies", [
            {"id": 1, "name": "main", "type": "branch"},
            {"id": 2, "name": "release", "type": "branch"},
        ])))
        for index, (payload, pages) in enumerate(cases):
            with self.subTest(index=index):
                self.assertTrue(_errors(environment=payload, branch_pages=pages))

    def test_secret_must_exist_only_in_environment(self) -> None:
        repo_copy = _pages("secrets", [{"name": "RULESET_GOVERNANCE_TOKEN"}])
        missing_env = _pages("secrets", [])
        duplicate_env = _pages("secrets", [
            {"name": "RULESET_GOVERNANCE_TOKEN"},
            {"name": "RULESET_GOVERNANCE_TOKEN"},
        ])
        self.assertTrue(_errors(repo_secret_pages=repo_copy))
        self.assertTrue(_errors(env_secret_pages=missing_env))
        self.assertTrue(_errors(env_secret_pages=duplicate_env))

    def test_all_metadata_pages_are_complete_and_unique(self) -> None:
        incomplete = [{"total_count": 2, "secrets": [{"name": "OTHER"}]}]
        changed = [
            {"total_count": 2, "secrets": [{"name": "OTHER"}]},
            {"total_count": 1, "secrets": [{"name": "RULESET_GOVERNANCE_TOKEN"}]},
        ]
        duplicate = _pages("branch_policies", [
            {"id": 1, "name": "main", "type": "branch"},
            copy.deepcopy({"id": 1, "name": "main", "type": "branch"}),
        ])
        self.assertTrue(_errors(repo_secret_pages=incomplete))
        self.assertTrue(_errors(env_secret_pages=changed))
        self.assertTrue(_errors(branch_pages=duplicate))


if __name__ == "__main__":
    unittest.main()
