from __future__ import annotations

import argparse
import fnmatch
import subprocess
import sys
from pathlib import Path

ALLOWED_BUILD_BRANCHES = ("main", "integrate/*")


def _git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        capture_output=True,
        check=False,
    )
    if check and result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or result.stdout.strip() or f"git {' '.join(args)} failed")
    return result


def _lines(text: str) -> list[str]:
    return [line.strip() for line in text.splitlines() if line.strip()]


def _current_branch(repo: Path) -> str:
    return _git(repo, "branch", "--show-current").stdout.strip()


def _unmerged_branches(repo: Path, base: str) -> list[str]:
    return _lines(_git(repo, "branch", "--no-merged", base, "--format=%(refname:short)").stdout)


def _branch_is_in_head(repo: Path, branch: str) -> bool:
    return _git(repo, "merge-base", "--is-ancestor", branch, "HEAD", check=False).returncode == 0


def _allowed_branch(branch: str) -> bool:
    return any(fnmatch.fnmatchcase(branch, pattern) for pattern in ALLOWED_BUILD_BRANCHES)


def check(repo: Path, *, main_branch: str = "main", feature_only_ok: bool = False) -> int:
    repo = repo.resolve()
    branch = _current_branch(repo)
    status = _git(repo, "status", "--short", "--branch").stdout.strip()
    unmerged = _unmerged_branches(repo, main_branch)
    not_in_head = [name for name in unmerged if name != branch and not _branch_is_in_head(repo, name)]
    allowed = bool(branch) and _allowed_branch(branch)
    feature_only = feature_only_ok and not allowed

    print(f"[integration-preflight] repo: {repo}")
    print(f"[integration-preflight] branch: {branch or '<detached>'}")
    print(f"[integration-preflight] build mode: {'feature-only' if feature_only else 'integration'}")
    print("[integration-preflight] status:")
    print(status or "clean")

    if unmerged:
        print("[integration-preflight] branches not merged into main:")
        for name in unmerged:
            print(f"  - {name}")
    else:
        print("[integration-preflight] branches not merged into main: none")

    if not_in_head:
        print("[integration-preflight] branches not included in current HEAD:")
        for name in not_in_head:
            print(f"  - {name}")
    else:
        print("[integration-preflight] branches not included in current HEAD: none")

    errors: list[str] = []
    if not branch:
        errors.append("detached HEAD is not a supported Flatpak test-build branch")
    if not allowed and not feature_only_ok:
        errors.append(
            f"{branch or '<detached>'} is not an integration build branch; use main, integrate/*, or --feature-only-ok"
        )
    if feature_only:
        print("[integration-preflight] feature-only builds must be reported as partial")

    for error in errors:
        print(f"[integration-preflight] ERROR: {error}")
    if errors:
        return 1
    print("[integration-preflight] OK")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Guard local Flatpak test builds against branch-split mistakes.")
    parser.add_argument("--repo", default=".", help="Git repository to inspect.")
    parser.add_argument("--main", default="main", help="Main branch name.")
    parser.add_argument(
        "--feature-only-ok",
        action="store_true",
        help="Allow a feature-branch build, marking it as intentionally partial.",
    )
    args = parser.parse_args(argv)
    try:
        return check(Path(args.repo), main_branch=args.main, feature_only_ok=args.feature_only_ok)
    except RuntimeError as error:
        print(f"[integration-preflight] ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
