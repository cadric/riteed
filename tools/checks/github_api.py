from __future__ import annotations

import json
import http.client
import os
import re
import subprocess
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Callable


API_ORIGIN = "https://api.github.com"
MAX_RESPONSE_BYTES = 8 * 1024 * 1024
MAX_PAGES = 1000
PageRequest = Callable[[str, str], tuple[Any, str]]


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(
        self,
        req: urllib.request.Request,
        fp: Any,
        code: int,
        msg: str,
        headers: Any,
        newurl: str,
    ) -> None:
        return None


def github_token() -> str | None:
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
    value = result.stdout.strip()
    return value if result.returncode == 0 and value else None


def fetch_json(url: str, token: str, errors: list[str], label: str) -> Any | None:
    if not _safe_api_url(url):
        errors.append(f"GitHub API URL is outside the reviewed origin for {label}.")
        return None
    try:
        payload, _ = _request_page(url, token)
        if payload is None:
            errors.append(f"GitHub API returned a null payload for {label}.")
            return None
        return payload
    except (
        OSError,
        http.client.HTTPException,
        urllib.error.URLError,
        json.JSONDecodeError,
        ValueError,
    ) as exc:
        errors.append(f"GitHub API verification failed for {label}: {_safe_failure(exc)}")
        return None


def fetch_pages(
    initial_url: str,
    token: str,
    errors: list[str],
    label: str,
    *,
    request_page: PageRequest | None = None,
) -> list[dict[str, Any]] | None:
    try:
        expected = urllib.parse.urlsplit(initial_url)
        base_query = _query(expected.query)
    except ValueError:
        expected = urllib.parse.SplitResult("", "", "", "", "")
        base_query = None
    if not _safe_api_url(initial_url) or base_query is None:
        errors.append(f"GitHub API pagination URL is invalid for {label}.")
        return None
    request = request_page or _request_page
    pages: list[dict[str, Any]] = []
    visited: set[str] = set()
    url = initial_url
    while url:
        if len(pages) >= MAX_PAGES:
            errors.append(f"GitHub API pagination exceeded the reviewed page limit for {label}.")
            return None
        if url in visited or not _same_page_resource(url, expected.path, base_query):
            errors.append(f"GitHub API pagination escaped or repeated for {label}.")
            return None
        visited.add(url)
        try:
            payload, link = request(url, token)
        except (
            OSError,
            http.client.HTTPException,
            urllib.error.URLError,
            json.JSONDecodeError,
            ValueError,
        ) as exc:
            errors.append(f"GitHub API verification failed for {label}: {_safe_failure(exc)}")
            return None
        if not isinstance(payload, dict):
            errors.append(f"GitHub API pagination expected object pages for {label}.")
            return None
        pages.append(payload)
        link_valid, next_url = _next_link(link)
        if not link_valid:
            errors.append(f"GitHub API pagination Link header is invalid for {label}.")
            return None
        url = next_url
    return pages


def api_url(repository: str, suffix: str, query: str = "") -> str:
    owner, name = repository.split("/", 1)
    path = "/repos/" + urllib.parse.quote(owner, safe="") + "/" + urllib.parse.quote(name, safe="")
    path += suffix
    return API_ORIGIN + path + (("?" + query) if query else "")


def _request_page(url: str, token: str) -> tuple[Any, str]:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "application/vnd.github+json",
            "Authorization": f"Bearer {token}",
            "X-GitHub-Api-Version": "2022-11-28",
        },
    )
    opener = urllib.request.build_opener(_NoRedirect())
    with opener.open(request, timeout=20) as response:
        raw = response.read(MAX_RESPONSE_BYTES + 1)
        if len(raw) > MAX_RESPONSE_BYTES:
            raise ValueError("response exceeded the bounded JSON limit")
        return json.loads(raw.decode("utf-8")), response.headers.get("Link", "")


def _safe_api_url(url: str) -> bool:
    try:
        parsed = urllib.parse.urlsplit(url)
        return (
            parsed.scheme == "https"
            and parsed.hostname == "api.github.com"
            and parsed.port is None
            and parsed.username is None
            and parsed.password is None
            and not parsed.fragment
            and parsed.path.startswith("/repos/")
        )
    except ValueError:
        return False


def _same_page_resource(url: str, path: str, base_query: dict[str, str]) -> bool:
    if not _safe_api_url(url):
        return False
    try:
        parsed = urllib.parse.urlsplit(url)
    except ValueError:
        return False
    query = _query(parsed.query)
    if parsed.path != path or query is None:
        return False
    without_page = {key: value for key, value in query.items() if key != "page"}
    page = query.get("page")
    return without_page == base_query and (
        page is None or re.fullmatch(r"[1-9][0-9]*", page) is not None
    )


def _query(value: str) -> dict[str, str] | None:
    try:
        parsed = urllib.parse.parse_qs(value, keep_blank_values=True, strict_parsing=True)
    except ValueError:
        return None
    if any(len(items) != 1 for items in parsed.values()):
        return None
    return {key: items[0] for key, items in parsed.items()}


def _next_link(value: str) -> tuple[bool, str]:
    if not value.strip():
        return True, ""
    relations: dict[str, str] = {}
    for part in value.split(","):
        match = re.fullmatch(r'\s*<([^<>]+)>;\s*rel="([a-z]+)"\s*', part)
        if match is None or match.group(2) in relations:
            return False, ""
        relations[match.group(2)] = match.group(1)
    return True, relations.get("next", "")


def _safe_failure(exc: BaseException) -> str:
    if isinstance(exc, urllib.error.HTTPError):
        return f"HTTP {exc.code}"
    if isinstance(exc, json.JSONDecodeError):
        return "invalid JSON response"
    if isinstance(exc, ValueError):
        return "invalid request or response"
    return "transport failure"
