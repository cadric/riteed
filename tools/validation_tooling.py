#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any, Iterable, NoReturn, Sequence

EXCLUDED_PARTS = {
    ".git",
    "target",
    ".flatpak-builder",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".venv",
    "venv",
}

_GLOB_CACHE: dict[str, re.Pattern[str]] = {}


def fail(message: str) -> NoReturn:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def _has_app_layout(path: Path) -> bool:
    return (
        (path / "Cargo.toml").is_file()
        and (path / "src").is_dir()
        and (path / "data").is_dir()
        and (path / "po").is_dir()
        and (path / "build-aux").is_dir()
    )


def _has_contract_layout(path: Path) -> bool:
    return (path / "policy").is_dir()


def _is_target_repo(path: Path) -> bool:
    return _has_app_layout(path) and (path / "policy").is_dir()


def _is_policy_pack_repo(path: Path) -> bool:
    return (path / "AGENTS.md").exists() and (path / "policy").is_dir() and (path / "tools").is_dir() and not (path / "Cargo.toml").exists()


def contract_root(root: Path) -> Path:
    start = root.resolve()
    for cur in [start, *start.parents]:
        if _has_contract_layout(cur):
            return cur
    fail(f"[validation] unable to locate policy/tooling contract root for {start}")


def repo_root(explicit: str | None = None, *, allow_policy_pack: bool = False) -> Path:
    candidates: list[Path] = []
    if explicit:
        candidates.append(Path(explicit).resolve())
    candidates.extend([Path.cwd().resolve(), Path(__file__).resolve().parent.parent])
    seen: set[Path] = set()
    for start in candidates:
        for cur in [start, *start.parents]:
            if cur in seen:
                continue
            seen.add(cur)
            if _has_app_layout(cur):
                try:
                    contract_root(cur)
                except SystemExit:
                    pass
                else:
                    return cur
            if allow_policy_pack and _is_policy_pack_repo(cur):
                return cur
    if explicit:
        fail(f"[validation] repository root is not a supported target: {Path(explicit).resolve()}")
    fail("[validation] unable to auto-detect a target application repository root; pass --root")


def normalize_path(path: str) -> str:
    value = path.replace("\\", "/").strip()
    while value.startswith("./"):
        value = value[2:]
    return value


def relpath(path: Path, root: Path) -> str:
    return normalize_path(path.relative_to(root).as_posix())


def load_json(path: Path) -> dict[str, Any]:
    try:
        with path.open("r", encoding="utf-8") as handle:
            return json.load(handle)
    except FileNotFoundError:
        fail(f"[validation] missing JSON file: {path}")
    except json.JSONDecodeError as exc:
        fail(f"[validation] invalid JSON in {path}: {exc}")
    except OSError as exc:
        fail(f"[validation] failed to read JSON file {path}: {exc}")


def load_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            return tomllib.load(handle)
    except FileNotFoundError:
        fail(f"[validation] missing TOML file: {path}")
    except tomllib.TOMLDecodeError as exc:
        fail(f"[validation] invalid TOML in {path}: {exc}")
    except OSError as exc:
        fail(f"[validation] failed to read TOML file {path}: {exc}")


def dump_json(payload: dict[str, Any]) -> str:
    return json.dumps(payload, indent=2, ensure_ascii=True) + "\n"


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError as exc:
        fail(f"[validation] failed to read text file {path}: {exc}")


def count_lines(path: Path) -> int:
    return len(read_text(path).splitlines())


def split_lines(text: str) -> list[str]:
    return text.splitlines()


def line_text(text: str, line_number: int) -> str | None:
    lines = split_lines(text)
    if line_number < 1 or line_number > len(lines):
        return None
    return lines[line_number - 1]


def iter_files(root: Path) -> Iterable[Path]:
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in EXCLUDED_PARTS for part in path.parts):
            continue
        yield path


def _glob_to_regex(pattern: str) -> re.Pattern[str]:
    cached = _GLOB_CACHE.get(pattern)
    if cached is not None:
        return cached
    normalized = normalize_path(pattern)
    parts: list[str] = ["^"]
    index = 0
    while index < len(normalized):
        if normalized.startswith("**/", index):
            parts.append(r"(?:[^/]+/)*")
            index += 3
            continue
        char = normalized[index]
        if char == "*":
            if normalized.startswith("**", index):
                parts.append(r".*")
                index += 2
            else:
                parts.append(r"[^/]*")
                index += 1
        elif char == "?":
            parts.append(r"[^/]")
            index += 1
        else:
            parts.append(re.escape(char))
            index += 1
    parts.append("$")
    regex = re.compile("".join(parts))
    _GLOB_CACHE[pattern] = regex
    return regex


def match_any(rel: str, patterns: Sequence[str]) -> bool:
    normalized = normalize_path(rel)
    return any(_glob_to_regex(pattern).match(normalized) for pattern in patterns)


def scoped_files(root: Path, patterns: Sequence[str]) -> list[Path]:
    matches: list[Path] = []
    for path in iter_files(root):
        if match_any(relpath(path, root), patterns):
            matches.append(path)
    return sorted(set(matches), key=lambda item: relpath(item, root))


def first_file(root: Path, patterns: Sequence[str]) -> Path | None:
    files = scoped_files(root, patterns)
    return files[0] if files else None


def require_tool(name: str) -> None:
    if shutil.which(name) is None:
        fail(f"[validation] required tool not found: {name}")


def run_capture(cmd: Sequence[str], cwd: Path, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    try:
        merged_env = os.environ.copy()
        if env:
            merged_env.update(env)
        return subprocess.run(
            list(cmd),
            cwd=str(cwd),
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=merged_env,
            check=False,
        )
    except OSError as exc:
        fail(f"[validation] failed to run {' '.join(cmd)}: {exc}")


def run_checked(cmd: Sequence[str], cwd: Path, label: str | None = None, env: dict[str, str] | None = None) -> str:
    result = run_capture(cmd, cwd, env=env)
    if result.returncode != 0:
        detail = failure_detail(result.stdout, result.stderr)
        fail(f"[validation] {label or 'command failed'}: {' '.join(cmd)} :: {detail}")
    return result.stdout


def failure_detail(stdout: str, stderr: str) -> str:
    streams = []
    if stdout.strip():
        streams.append(f"stdout:\n{stdout.strip()}")
    if stderr.strip():
        streams.append(f"stderr:\n{stderr.strip()}")
    return "\n".join(streams) or "unknown error"


def grep_lines(root: Path, paths: Sequence[Path], pattern: str) -> list[str]:
    regex = re.compile(pattern, re.MULTILINE)
    hits: list[str] = []
    for path in paths:
        text = read_text(path)
        for index, line in enumerate(text.splitlines(), start=1):
            if regex.search(line):
                hits.append(f"{relpath(path, root)}:{index}: {line.strip()}")
    return hits


def grep_any(root: Path, paths: Sequence[Path], pattern: str) -> bool:
    return bool(grep_lines(root, paths, pattern))


def toml_string(value: Any) -> str:
    if isinstance(value, str):
        return value
    if value is True:
        return "true"
    if value is False:
        return "false"
    return str(value)


def file_hash(path: Path) -> tuple[str, int]:
    data = path.read_bytes()
    return hashlib.sha256(data).hexdigest(), len(data)


def manifest_paths_from_metadata(root: Path) -> list[Path]:
    data = json.loads(
        run_checked(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            root,
            "cargo metadata failed",
        )
    )
    paths = {
        Path(pkg["manifest_path"]).resolve()
        for pkg in data.get("packages", [])
        if isinstance(pkg, dict) and pkg.get("manifest_path")
    }
    return sorted(paths)


def cargo_packages(root: Path) -> list[dict[str, Any]]:
    data = json.loads(
        run_checked(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            root,
            "cargo metadata failed",
        )
    )
    return [pkg for pkg in data.get("packages", []) if isinstance(pkg, dict)]
