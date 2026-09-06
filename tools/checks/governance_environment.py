from __future__ import annotations

import re
import urllib.parse
from typing import Any

from tools.checks import github_api


POLICY_FILE = "policy/release.policy.json"


def check_remote(policy: dict[str, Any], errors: list[str]) -> None:
    config = _config(policy, errors)
    if config is None:
        return
    token = github_api.github_token()
    if not token:
        errors.append("GitHub governance environment verification requires GITHUB_TOKEN or GH_TOKEN.")
        return
    repository, environment_name, _secret, _branch = config
    encoded = urllib.parse.quote(environment_name, safe="")
    environment = github_api.fetch_json(
        github_api.api_url(repository, f"/environments/{encoded}"),
        token,
        errors,
        environment_name,
    )
    branch_pages = github_api.fetch_pages(
        github_api.api_url(
            repository,
            f"/environments/{encoded}/deployment-branch-policies",
            "per_page=100",
        ),
        token,
        errors,
        f"{environment_name} branch policies",
    )
    repo_secret_pages = github_api.fetch_pages(
        github_api.api_url(repository, "/actions/secrets", "per_page=100"),
        token,
        errors,
        "repository secrets",
    )
    env_secret_pages = github_api.fetch_pages(
        github_api.api_url(repository, f"/environments/{encoded}/secrets", "per_page=100"),
        token,
        errors,
        f"{environment_name} secrets",
    )
    if environment is not None and not isinstance(environment, dict):
        errors.append(f"{environment_name}: GitHub environment response must be an object.")
    if (
        not isinstance(environment, dict)
        or branch_pages is None
        or repo_secret_pages is None
        or env_secret_pages is None
    ):
        return
    check_payloads(
        environment,
        branch_pages,
        repo_secret_pages,
        env_secret_pages,
        policy,
        errors,
    )


def check_payloads(
    environment: Any,
    branch_pages: Any,
    repo_secret_pages: Any,
    env_secret_pages: Any,
    policy: dict[str, Any],
    errors: list[str],
) -> None:
    config = _config(policy, errors)
    if config is None:
        return
    _repository, environment_name, secret_name, branch = config
    if not isinstance(environment, dict) or environment.get("name") != environment_name:
        errors.append(f"{environment_name}: exact environment payload is required.")
        return
    expected_deployment = {
        "protected_branches": branch["protected_branches"],
        "custom_branch_policies": branch["custom_branch_policies"],
    }
    if environment.get("deployment_branch_policy") != expected_deployment:
        errors.append(f"{environment_name}: deployment branch policy must be custom main-only.")
    branches = _paged_values(branch_pages, "branch_policies", "id", "deployment branch policy", errors)
    repo_secrets = _paged_values(repo_secret_pages, "secrets", "name", "repository secret", errors)
    env_secrets = _paged_values(env_secret_pages, "secrets", "name", "environment secret", errors)
    expected_branch = {"name": branch["name"], "type": branch["type"]}
    actual_branches = [
        {"name": item.get("name"), "type": item.get("type")} for item in branches
    ]
    if actual_branches != [expected_branch]:
        errors.append(f"{environment_name}: exactly one main branch deployment policy is required.")
    if any(item.get("name") == secret_name for item in repo_secrets):
        errors.append(f"Repository secret {secret_name} must be absent.")
    matching_env = [item for item in env_secrets if item.get("name") == secret_name]
    if len(matching_env) != 1:
        errors.append(f"Environment secret {secret_name} must exist exactly once.")


def _config(
    policy: dict[str, Any], errors: list[str]
) -> tuple[str, str, str, dict[str, Any]] | None:
    repository = policy.get("release_identity", {}).get("repository_full_name")
    config = (
        policy.get("github_actions_release_safety", {})
        .get("repository_governance", {})
        .get("truthful_checks")
    )
    if not isinstance(config, dict):
        errors.append(f"{POLICY_FILE}: truthful_checks is required for governance environment.")
        return None
    environment = config.get("live_environment")
    secret = config.get("live_secret")
    branch = config.get("environment_branch_policy")
    if (
        not isinstance(repository, str)
        or re.fullmatch(r"[^/\s]+/[^/\s]+", repository) is None
        or not isinstance(environment, str)
        or not environment
        or not isinstance(secret, str)
        or not secret
        or config.get("repository_secret_forbidden") is not True
        or not isinstance(branch, dict)
    ):
        errors.append(f"{POLICY_FILE}: governance environment policy is incomplete.")
        return None
    expected = {
        "protected_branches": False,
        "custom_branch_policies": True,
        "name": "main",
        "type": "branch",
    }
    if branch != expected:
        errors.append(f"{POLICY_FILE}: governance environment branch policy must be exact main-only.")
        return None
    return repository, environment, secret, expected


def _paged_values(
    pages: Any,
    key: str,
    identity: str,
    label: str,
    errors: list[str],
) -> list[dict[str, Any]]:
    if not isinstance(pages, list) or not pages:
        errors.append(f"{label} metadata requires non-empty paginated payloads.")
        return []
    expected: int | None = None
    values: list[dict[str, Any]] = []
    seen: set[Any] = set()
    for page in pages:
        if not isinstance(page, dict):
            errors.append(f"{label} pages must be objects.")
            return []
        total = page.get("total_count")
        items = page.get(key)
        if type(total) is not int or total < 0 or not isinstance(items, list):
            errors.append(f"{label} pages require total_count and {key}.")
            return []
        if expected is not None and total != expected:
            errors.append(f"{label} total_count changed during pagination.")
            return []
        expected = total
        for item in items:
            value = item.get(identity) if isinstance(item, dict) else None
            valid = (
                type(value) is int and value > 0
                if identity == "id"
                else isinstance(value, str) and bool(value.strip())
            )
            if not isinstance(item, dict) or not valid or value in seen:
                errors.append(f"{label} identities must be non-empty and unique.")
                return []
            seen.add(value)
            values.append(item)
    if expected is None or len(values) != expected:
        errors.append(f"Incomplete {label} pagination.")
        return []
    return values
