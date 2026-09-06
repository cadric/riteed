from __future__ import annotations

import json
import os
import subprocess
import urllib.error
import urllib.parse
import urllib.request
from typing import Any

from tools.checks import foundation, governance_environment


POLICY_FILE = "policy/release.policy.json"


def check_remote_governance(policy: dict[str, Any], active: set[str], errors: list[str]) -> None:
    check_ruleset_governance(policy, active, errors)
    check_rollback_environment_governance(policy, active, errors)
    governance_environment.check_remote(policy, errors)


def check_ruleset_governance(
    policy: dict[str, Any],
    active: set[str],
    errors: list[str],
    fetch_rulesets: Any | None = None,
) -> None:
    if "RIT-AUD-017" in active:
        return
    repo = str(policy.get("release_identity", {}).get("repository_full_name", "")).strip()
    if not repo:
        foundation.add(errors, f"{POLICY_FILE}: release_identity.repository_full_name is required")
        return
    fetch = fetch_rulesets or fetch_repository_rulesets
    rulesets = fetch(repo, errors)
    if rulesets is None:
        return
    check_ruleset_payloads(rulesets, policy, errors)


def fetch_repository_rulesets(repo: str, errors: list[str]) -> list[dict[str, Any]] | None:
    token = _github_token()
    if not token:
        foundation.add(errors, "GitHub ruleset governance requires GITHUB_TOKEN or GH_TOKEN for ruleset API verification")
        return None
    data = _github_api_json(f"https://api.github.com/repos/{repo}/rulesets", token, errors, repo)
    if data is None:
        return None
    if not isinstance(data, list):
        foundation.add(errors, f"GitHub ruleset API verification failed for {repo}: expected ruleset list")
        return None
    rulesets: list[dict[str, Any]] = []
    for item in data:
        if not isinstance(item, dict):
            continue
        ruleset_id = item.get("id")
        if isinstance(ruleset_id, int):
            detail = _github_api_json(
                f"https://api.github.com/repos/{repo}/rulesets/{ruleset_id}",
                token,
                errors,
                f"{repo} ruleset {ruleset_id}",
            )
            if isinstance(detail, dict):
                rulesets.append(detail)
            elif detail is not None:
                foundation.add(
                    errors,
                    f"GitHub ruleset API verification failed for {repo} ruleset {ruleset_id}: expected ruleset object",
                )
        else:
            rulesets.append(item)
    return rulesets


def check_rollback_environment_governance(
    policy: dict[str, Any],
    active: set[str],
    errors: list[str],
    fetch_environment: Any | None = None,
) -> None:
    if "RIT-AUD-017" in active:
        return
    repo = str(policy.get("release_identity", {}).get("repository_full_name", "")).strip()
    name = str(
        policy.get("github_actions_release_safety", {})
        .get("rollback_environment", {})
        .get("name", "")
    ).strip()
    if not repo:
        foundation.add(errors, f"{POLICY_FILE}: release_identity.repository_full_name is required")
        return
    if not name:
        foundation.add(errors, f"{POLICY_FILE}: rollback_environment.name is required")
        return
    fetch = fetch_environment or fetch_repository_environment
    environment = fetch(repo, name, errors)
    if environment is None:
        return
    check_rollback_environment_payload(environment, policy, errors)


def fetch_repository_environment(repo: str, name: str, errors: list[str]) -> dict[str, Any] | None:
    token = _github_token()
    if not token:
        foundation.add(errors, "GitHub rollback environment governance requires GITHUB_TOKEN or GH_TOKEN for API verification")
        return None
    encoded = urllib.parse.quote(name, safe="")
    data = _github_api_json(f"https://api.github.com/repos/{repo}/environments/{encoded}", token, errors, f"{repo} environment {name}")
    if data is None:
        return None
    if not isinstance(data, dict):
        foundation.add(errors, f"GitHub environment API verification failed for {repo} environment {name}: expected object")
        return None
    return data


def check_rollback_environment_payload(
    environment: dict[str, Any],
    policy: dict[str, Any],
    errors: list[str],
) -> None:
    reviewed = _reviewed_rollback_reviewer_keys(policy)
    if not reviewed:
        foundation.add(errors, f"{POLICY_FILE}: rollback_environment.reviewed_required_reviewers is required")
        return
    actual: set[tuple[str, int]] = set()
    for rule in environment.get("protection_rules", []):
        if not isinstance(rule, dict) or rule.get("type") != "required_reviewers":
            continue
        for reviewer in rule.get("reviewers", []):
            if not isinstance(reviewer, dict):
                foundation.add(errors, "flatpak-beta-rollback: required reviewers must be objects")
                continue
            actor_type = str(reviewer.get("type", "")).strip()
            actor = reviewer.get("reviewer", {})
            actor_id = actor.get("id") if isinstance(actor, dict) else None
            if not actor_type or not isinstance(actor_id, int):
                foundation.add(errors, "flatpak-beta-rollback: required reviewer is missing reviewed identity fields")
                continue
            key = (actor_type, actor_id)
            actual.add(key)
            if key not in reviewed:
                foundation.add(errors, f"flatpak-beta-rollback: unreviewed required reviewer {actor_type}:{actor_id}")
    if not actual:
        foundation.add(errors, "flatpak-beta-rollback: required_reviewers protection rule is required")
    for missing in sorted(reviewed - actual):
        actor_type, actor_id = missing
        foundation.add(errors, f"flatpak-beta-rollback: missing reviewed required reviewer {actor_type}:{actor_id}")


def check_ruleset_payloads(
    rulesets: list[dict[str, Any]],
    policy: dict[str, Any],
    errors: list[str],
) -> None:
    main = _find_ruleset(rulesets, "Protect main", "branch", "refs/heads/main")
    tags = _find_ruleset(rulesets, "Protect version tags", "tag", "refs/tags/v*")
    if main is None:
        foundation.add(errors, "GitHub ruleset governance requires Protect main for refs/heads/main")
    else:
        _check_main_ruleset(main, policy, errors)
    if tags is None:
        foundation.add(errors, "GitHub ruleset governance requires Protect version tags for refs/tags/v*")
    else:
        _check_tag_ruleset(tags, policy, errors)


def _github_api_json(url: str, token: str, errors: list[str], label: str) -> Any | None:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    try:
        with urllib.request.urlopen(request, timeout=10) as response:
            return json.loads(response.read().decode("utf-8"))
    except (OSError, urllib.error.URLError, json.JSONDecodeError) as exc:
        foundation.add(errors, f"GitHub ruleset API verification failed for {label}: {exc}")
        return None


def _github_token() -> str | None:
    token = os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN")
    if token:
        return token
    try:
        result = subprocess.run(
            ["gh", "auth", "token"],
            check=False,
            encoding="utf-8",
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            timeout=10,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    token = result.stdout.strip()
    return token if result.returncode == 0 and token else None


def _find_ruleset(
    rulesets: list[dict[str, Any]],
    name: str,
    target: str,
    include_ref: str,
) -> dict[str, Any] | None:
    for ruleset in rulesets:
        if ruleset.get("name") != name or ruleset.get("target") != target:
            continue
        refs = ruleset.get("conditions", {}).get("ref_name", {}).get("include", [])
        if include_ref in refs:
            return ruleset
    return None


def _check_main_ruleset(ruleset: dict[str, Any], policy: dict[str, Any], errors: list[str]) -> None:
    _require_active_ruleset(ruleset, errors)
    types = _ruleset_rule_types(ruleset)
    for required in ("deletion", "non_fast_forward", "pull_request", "required_signatures", "required_status_checks"):
        if required not in types:
            foundation.add(errors, f"Protect main ruleset missing {required} rule")
    _check_main_pull_request_policy(ruleset, policy, errors)
    checks = _required_status_check_contexts(ruleset)
    required_checks = _required_policy_status_checks(policy)
    for context in required_checks:
        if context not in checks:
            foundation.add(errors, f"Protect main ruleset missing required status check {context}")
    for context in sorted(checks - set(required_checks)):
        foundation.add(errors, f"Protect main ruleset has unreviewed required status check {context}")
    if not _strict_required_status_checks_policy(ruleset):
        foundation.add(errors, "Protect main ruleset must require strict_required_status_checks_policy")
    _require_reviewed_bypass(ruleset, policy, errors, required=True)


def _check_tag_ruleset(ruleset: dict[str, Any], policy: dict[str, Any], errors: list[str]) -> None:
    _require_active_ruleset(ruleset, errors)
    types = _ruleset_rule_types(ruleset)
    for required in ("update", "deletion"):
        if required not in types:
            foundation.add(errors, f"Protect version tags ruleset missing {required} rule")
    _require_reviewed_bypass(ruleset, policy, errors, required=False)


def _require_active_ruleset(ruleset: dict[str, Any], errors: list[str]) -> None:
    if ruleset.get("enforcement") != "active":
        foundation.add(errors, f"{ruleset.get('name', 'ruleset')}: enforcement must be active")


def _ruleset_rule_types(ruleset: dict[str, Any]) -> set[str]:
    return {str(rule.get("type")) for rule in ruleset.get("rules", []) if isinstance(rule, dict)}


def _required_status_check_contexts(ruleset: dict[str, Any]) -> set[str]:
    contexts: set[str] = set()
    for rule in ruleset.get("rules", []):
        if not isinstance(rule, dict) or rule.get("type") != "required_status_checks":
            continue
        checks = rule.get("parameters", {}).get("required_status_checks", [])
        for check in checks:
            if isinstance(check, dict) and check.get("context"):
                contexts.add(str(check["context"]))
    return contexts


def _strict_required_status_checks_policy(ruleset: dict[str, Any]) -> bool:
    for rule in ruleset.get("rules", []):
        if not isinstance(rule, dict) or rule.get("type") != "required_status_checks":
            continue
        return rule.get("parameters", {}).get("strict_required_status_checks_policy") is True
    return False


def _check_main_pull_request_policy(ruleset: dict[str, Any], policy: dict[str, Any], errors: list[str]) -> None:
    pr_policy = (
        policy.get("github_actions_release_safety", {})
        .get("repository_governance", {})
        .get("main_pull_request_policy")
    )
    if not isinstance(pr_policy, dict):
        foundation.add(errors, f"{POLICY_FILE}: repository_governance.main_pull_request_policy is required")
        return
    if pr_policy.get("require_pull_request") is not True:
        foundation.add(errors, f"{POLICY_FILE}: main_pull_request_policy.require_pull_request must be true")

    params = _rule_parameters(ruleset, "pull_request")
    if params is None:
        foundation.add(errors, "Protect main ruleset pull_request parameters are required")
        return

    expected_count = pr_policy.get("required_approving_review_count")
    if isinstance(expected_count, bool) or not isinstance(expected_count, int) or expected_count < 0:
        foundation.add(errors, f"{POLICY_FILE}: main_pull_request_policy.required_approving_review_count must be a non-negative integer")
    elif params.get("required_approving_review_count") != expected_count:
        foundation.add(
            errors,
            f"Protect main ruleset required_approving_review_count must be {expected_count}",
        )

    for field in ("required_review_thread_resolution", "require_last_push_approval"):
        expected = pr_policy.get(field)
        if not isinstance(expected, bool):
            foundation.add(errors, f"{POLICY_FILE}: main_pull_request_policy.{field} must be boolean")
        elif params.get(field) is not expected:
            foundation.add(errors, f"Protect main ruleset {field} must be {str(expected).lower()}")


def _rule_parameters(ruleset: dict[str, Any], rule_type: str) -> dict[str, Any] | None:
    for rule in ruleset.get("rules", []):
        if not isinstance(rule, dict) or rule.get("type") != rule_type:
            continue
        parameters = rule.get("parameters")
        return parameters if isinstance(parameters, dict) else None
    return None


def _required_policy_status_checks(policy: dict[str, Any]) -> list[str]:
    raw = (
        policy.get("signed_flatpak_publish", {})
        .get("hard_requirements", {})
        .get("required_validate_check_contexts", [])
    )
    return [str(item).strip() for item in raw if isinstance(item, str) and item.strip()]


def _require_reviewed_bypass(
    ruleset: dict[str, Any],
    policy: dict[str, Any],
    errors: list[str],
    *,
    required: bool,
) -> None:
    name = str(ruleset.get("name", "ruleset"))
    reviewed = _reviewed_bypass_keys(policy, name)
    if required and not reviewed:
        foundation.add(errors, f"{POLICY_FILE}: repository_governance.reviewed_bypass_actors is required")
        return
    actual: set[tuple[str, int, str]] = set()
    for bypass in ruleset.get("bypass_actors", []):
        if not isinstance(bypass, dict):
            foundation.add(errors, f"{name}: bypass actors must be objects")
            continue
        actor_type = str(bypass.get("actor_type", "")).strip()
        actor_id = bypass.get("actor_id")
        bypass_mode = str(bypass.get("bypass_mode", "")).strip()
        if not actor_type or not isinstance(actor_id, int) or not bypass_mode:
            foundation.add(errors, f"{name}: bypass actor is missing reviewed identity fields")
            continue
        if bypass_mode != "pull_request":
            foundation.add(errors, f"{name}: bypass actor {actor_type}:{actor_id} must use pull_request bypass mode")
            continue
        key = (actor_type, actor_id, bypass_mode)
        actual.add(key)
        if key not in reviewed:
            foundation.add(
                errors,
                f"{name}: unreviewed bypass actor {actor_type}:{actor_id}:{bypass_mode}",
            )
    for missing in sorted(reviewed - actual):
        actor_type, actor_id, bypass_mode = missing
        foundation.add(
            errors,
            f"{name}: missing reviewed bypass actor {actor_type}:{actor_id}:{bypass_mode}",
        )
    if not required and actual:
        foundation.add(errors, f"{name}: bypass actors are forbidden for tag rulesets")


def _reviewed_bypass_keys(policy: dict[str, Any], ruleset_name: str) -> set[tuple[str, int, str]]:
    governance = policy.get("github_actions_release_safety", {}).get("repository_governance", {})
    actors = governance.get("reviewed_bypass_actors", [])
    keys: set[tuple[str, int, str]] = set()
    for actor in actors if isinstance(actors, list) else []:
        if not isinstance(actor, dict):
            continue
        if str(actor.get("ruleset", "")).strip() != ruleset_name:
            continue
        actor_type = str(actor.get("actor_type", "")).strip()
        actor_id = actor.get("actor_id")
        bypass_mode = str(actor.get("bypass_mode", "")).strip()
        if actor_type and isinstance(actor_id, int) and bypass_mode:
            keys.add((actor_type, actor_id, bypass_mode))
    return keys


def _reviewed_rollback_reviewer_keys(policy: dict[str, Any]) -> set[tuple[str, int]]:
    rollback = policy.get("github_actions_release_safety", {}).get("rollback_environment", {})
    reviewers = rollback.get("reviewed_required_reviewers", [])
    keys: set[tuple[str, int]] = set()
    for reviewer in reviewers if isinstance(reviewers, list) else []:
        if not isinstance(reviewer, dict):
            continue
        actor_type = str(reviewer.get("actor_type", "")).strip()
        actor_id = reviewer.get("actor_id")
        if actor_type and isinstance(actor_id, int):
            keys.add((actor_type, actor_id))
    return keys
