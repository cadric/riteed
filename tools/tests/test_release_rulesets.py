from __future__ import annotations

import os
import subprocess
import sys
import unittest
from pathlib import Path
from typing import Any

from tools.checks import rulesets

REPO_ROOT = Path(__file__).resolve().parents[2]


def _policy() -> dict[str, Any]:
    return {
        "signed_flatpak_publish": {
            "hard_requirements": {
                "required_validate_check_contexts": [
                    "dependency-preflight",
                    "policy-pack",
                    "native-tests",
                    "governance-static",
                    "flatpak-tests",
                    "flatpak",
                ]
            }
        },
        "github_actions_release_safety": {
            "repository_governance": {
                "main_pull_request_policy": {
                    "require_pull_request": True,
                    "required_approving_review_count": 0,
                    "required_review_thread_resolution": True,
                    "require_last_push_approval": False,
                },
                "reviewed_bypass_actors": [
                    {
                        "ruleset": "Protect main",
                        "actor_id": 964797,
                        "actor_type": "User",
                        "bypass_mode": "pull_request",
                    }
                ]
            },
            "rollback_environment": {
                "name": "flatpak-beta-rollback",
                "reviewed_required_reviewers": [
                    {"actor_id": 964797, "actor_type": "User"}
                ],
            },
        }
    }


def _active_rulesets() -> list[dict[str, Any]]:
    return [
        {
            "name": "Protect main",
            "target": "branch",
            "enforcement": "active",
            "conditions": {"ref_name": {"include": ["refs/heads/main"], "exclude": []}},
            "bypass_actors": [{"actor_id": 964797, "actor_type": "User", "bypass_mode": "pull_request"}],
            "rules": [
                {"type": "deletion"},
                {"type": "non_fast_forward"},
                {
                    "type": "pull_request",
                    "parameters": {
                        "required_approving_review_count": 0,
                        "dismiss_stale_reviews_on_push": True,
                        "required_reviewers": [],
                        "require_code_owner_review": False,
                        "require_last_push_approval": False,
                        "required_review_thread_resolution": True,
                        "allowed_merge_methods": ["merge", "squash", "rebase"],
                    },
                },
                {"type": "required_signatures"},
                {
                    "type": "required_status_checks",
                    "parameters": {
                        "required_status_checks": [
                            {"context": "dependency-preflight"},
                            {"context": "policy-pack"},
                            {"context": "native-tests"},
                            {"context": "governance-static"},
                            {"context": "flatpak-tests"},
                            {"context": "flatpak"},
                        ],
                        "strict_required_status_checks_policy": True,
                    },
                },
            ],
        },
        {
            "name": "Protect version tags",
            "target": "tag",
            "enforcement": "active",
            "conditions": {"ref_name": {"include": ["refs/tags/v*"], "exclude": []}},
            "bypass_actors": [],
            "rules": [{"type": "update"}, {"type": "deletion"}],
        },
    ]


def _rollback_environment() -> dict[str, Any]:
    return {
        "name": "flatpak-beta-rollback",
        "protection_rules": [
            {
                "type": "required_reviewers",
                "reviewers": [
                    {
                        "type": "User",
                        "reviewer": {"id": 964797, "login": "cadric"},
                    }
                ],
            }
        ],
    }


class ReleaseRulesetTests(unittest.TestCase):
    def test_active_rulesets_satisfy_governance_payload(self) -> None:
        errors: list[str] = []
        rulesets.check_ruleset_payloads(_active_rulesets(), _policy(), errors)
        self.assertEqual(errors, [])

    def test_reviewed_rollback_environment_satisfies_governance_payload(self) -> None:
        errors: list[str] = []
        rulesets.check_rollback_environment_payload(_rollback_environment(), _policy(), errors)
        self.assertEqual(errors, [])

    def test_disabled_or_incomplete_rulesets_fail_governance_payload(self) -> None:
        payload = _active_rulesets()
        payload[0]["enforcement"] = "disabled"
        for rule in payload[0]["rules"]:
            if rule.get("type") == "required_status_checks":
                rule["parameters"]["required_status_checks"].pop()
        payload.pop()
        errors: list[str] = []
        rulesets.check_ruleset_payloads(payload, _policy(), errors)
        self.assertTrue(any("enforcement must be active" in item for item in errors), errors)
        self.assertTrue(any("flatpak" in item for item in errors), errors)
        self.assertTrue(any("Protect version tags" in item for item in errors), errors)

    def test_unreviewed_ruleset_bypass_actor_fails_governance_payload(self) -> None:
        payload = _active_rulesets()
        payload[0]["bypass_actors"] = [
            {"actor_id": 1234, "actor_type": "Team", "bypass_mode": "pull_request"}
        ]
        errors: list[str] = []
        rulesets.check_ruleset_payloads(payload, _policy(), errors)
        self.assertTrue(any("unreviewed bypass actor Team:1234:pull_request" in item for item in errors), errors)
        self.assertTrue(any("missing reviewed bypass actor User:964797:pull_request" in item for item in errors), errors)

    def test_ruleset_bypass_modes_other_than_pull_request_fail(self) -> None:
        for mode in ("always", "exempt", "typo"):
            with self.subTest(mode=mode):
                payload = _active_rulesets()
                payload[0]["bypass_actors"] = [
                    {"actor_id": 964797, "actor_type": "User", "bypass_mode": mode}
                ]
                errors: list[str] = []
                rulesets.check_ruleset_payloads(payload, _policy(), errors)
                self.assertTrue(any("must use pull_request bypass mode" in item for item in errors), errors)

    def test_tag_ruleset_bypass_actors_fail(self) -> None:
        payload = _active_rulesets()
        payload[1]["bypass_actors"] = [
            {"actor_id": 964797, "actor_type": "User", "bypass_mode": "pull_request"}
        ]
        errors: list[str] = []
        rulesets.check_ruleset_payloads(payload, _policy(), errors)
        self.assertTrue(any("bypass actors are forbidden for tag rulesets" in item for item in errors), errors)

    def test_main_ruleset_requires_pr_signatures_and_strict_status_checks(self) -> None:
        payload = _active_rulesets()
        for rule in payload[0]["rules"]:
            if rule.get("type") == "required_status_checks":
                rule["parameters"]["strict_required_status_checks_policy"] = False
        payload[0]["rules"] = [rule for rule in payload[0]["rules"] if rule.get("type") != "pull_request"]
        payload[0]["rules"] = [rule for rule in payload[0]["rules"] if rule.get("type") != "required_signatures"]
        errors: list[str] = []
        rulesets.check_ruleset_payloads(payload, _policy(), errors)
        self.assertTrue(any("missing pull_request rule" in item for item in errors), errors)
        self.assertTrue(any("missing required_signatures rule" in item for item in errors), errors)
        self.assertTrue(any("strict_required_status_checks_policy" in item for item in errors), errors)

    def test_main_ruleset_pull_request_policy_must_match_reviewed_solo_model(self) -> None:
        cases = (
            ("required_approving_review_count", 1, "required_approving_review_count must be 0"),
            ("required_review_thread_resolution", False, "required_review_thread_resolution must be true"),
            ("require_last_push_approval", True, "require_last_push_approval must be false"),
        )
        for field, value, expected in cases:
            with self.subTest(field=field):
                payload = _active_rulesets()
                for rule in payload[0]["rules"]:
                    if rule.get("type") == "pull_request":
                        rule["parameters"][field] = value
                errors: list[str] = []
                rulesets.check_ruleset_payloads(payload, _policy(), errors)
                self.assertTrue(any(expected in item for item in errors), errors)

    def test_main_ruleset_pull_request_policy_requires_parameters(self) -> None:
        payload = _active_rulesets()
        for rule in payload[0]["rules"]:
            if rule.get("type") == "pull_request":
                rule.pop("parameters")
        errors: list[str] = []
        rulesets.check_ruleset_payloads(payload, _policy(), errors)
        self.assertTrue(any("pull_request parameters are required" in item for item in errors), errors)

    def test_main_ruleset_pull_request_policy_requires_policy_contract(self) -> None:
        policy = _policy()
        policy["github_actions_release_safety"]["repository_governance"].pop("main_pull_request_policy")
        errors: list[str] = []
        rulesets.check_ruleset_payloads(_active_rulesets(), policy, errors)
        self.assertTrue(any("main_pull_request_policy is required" in item for item in errors), errors)

    def test_unreviewed_rollback_environment_reviewer_fails_governance_payload(self) -> None:
        payload = _rollback_environment()
        payload["protection_rules"][0]["reviewers"] = [
            {"type": "Team", "reviewer": {"id": 1234, "name": "release"}}
        ]
        errors: list[str] = []
        rulesets.check_rollback_environment_payload(payload, _policy(), errors)
        self.assertTrue(any("unreviewed required reviewer Team:1234" in item for item in errors), errors)
        self.assertTrue(any("missing reviewed required reviewer User:964797" in item for item in errors), errors)

    def test_ruleset_governance_uses_ruleset_api_after_remediation_clears(self) -> None:
        policy = _policy() | {"release_identity": {"repository_full_name": "cadric/riteed"}}
        calls: list[str] = []

        def fetch(repo: str, errors: list[str]) -> list[dict[str, Any]]:
            calls.append(repo)
            self.assertEqual(errors, [])
            return _active_rulesets()

        errors: list[str] = []
        rulesets.check_ruleset_governance(policy, set(), errors, fetch)
        self.assertEqual(calls, ["cadric/riteed"])
        self.assertEqual(errors, [])

    def test_rollback_environment_governance_uses_environment_api(self) -> None:
        policy = _policy() | {"release_identity": {"repository_full_name": "cadric/riteed"}}
        calls: list[tuple[str, str]] = []

        def fetch(repo: str, name: str, errors: list[str]) -> dict[str, Any]:
            calls.append((repo, name))
            self.assertEqual(errors, [])
            return _rollback_environment()

        errors: list[str] = []
        rulesets.check_rollback_environment_governance(policy, set(), errors, fetch)
        self.assertEqual(calls, [("cadric/riteed", "flatpak-beta-rollback")])
        self.assertEqual(errors, [])

    def test_fetch_repository_rulesets_expands_list_entries_to_detail_payloads(self) -> None:
        calls: list[str] = []

        def api_json(url: str, token: str, errors: list[str], label: str) -> Any:
            calls.append(url)
            self.assertEqual(token, "token")
            self.assertEqual(errors, [])
            if url.endswith("/rulesets"):
                return [{"id": 16713108}, {"id": 16713116}]
            if url.endswith("/16713108"):
                return _active_rulesets()[0]
            return _active_rulesets()[1]

        original_token = rulesets._github_token
        original_api_json = rulesets._github_api_json
        rulesets._github_token = lambda: "token"
        rulesets._github_api_json = api_json
        try:
            errors: list[str] = []
            payload = rulesets.fetch_repository_rulesets("cadric/riteed", errors)
        finally:
            rulesets._github_token = original_token
            rulesets._github_api_json = original_api_json
        self.assertEqual(errors, [])
        self.assertEqual(payload, _active_rulesets())
        self.assertEqual(len(calls), 3)

    def test_fetch_repository_environment_uses_environment_endpoint(self) -> None:
        calls: list[str] = []

        def api_json(url: str, token: str, errors: list[str], label: str) -> Any:
            calls.append(url)
            self.assertEqual(token, "token")
            self.assertEqual(errors, [])
            self.assertIn("flatpak-beta-rollback", label)
            return _rollback_environment()

        original_token = rulesets._github_token
        original_api_json = rulesets._github_api_json
        rulesets._github_token = lambda: "token"
        rulesets._github_api_json = api_json
        try:
            errors: list[str] = []
            payload = rulesets.fetch_repository_environment("cadric/riteed", "flatpak-beta-rollback", errors)
        finally:
            rulesets._github_token = original_token
            rulesets._github_api_json = original_api_json
        self.assertEqual(errors, [])
        self.assertEqual(payload, _rollback_environment())
        self.assertEqual(calls, ["https://api.github.com/repos/cadric/riteed/environments/flatpak-beta-rollback"])

    def test_active_remediation_defers_ruleset_api_requirement(self) -> None:
        policy = {"release_identity": {"repository_full_name": "cadric/riteed"}}
        errors: list[str] = []
        rulesets.check_ruleset_governance(policy, {"RIT-AUD-017"}, errors, None)
        self.assertEqual(errors, [])

    @unittest.skipUnless(os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"), "requires live GitHub token")
    def test_ruleset_governance_module_live_api_path_when_token_is_available(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "tools.ruleset_governance_check"],
            cwd=REPO_ROOT,
            check=False,
            encoding="utf-8",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)


if __name__ == "__main__":
    unittest.main()
