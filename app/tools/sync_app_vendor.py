#!/usr/bin/env python3
from __future__ import annotations

import argparse
import shutil
import sys
from pathlib import Path

SYNC_MAP: tuple[tuple[str, str], ...] = (
    ("policy", "app/policy"),
    ("tools", "app/tools"),
    ("scripts/policy-check", "app/scripts/policy-check"),
)

IGNORED_PARTS = {"__pycache__", ".pytest_cache"}
IGNORED_SUFFIXES = {".pyc", ".pyo"}


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Sync vendored policy/tool files into app/.")
    parser.add_argument("--root", help="Repository root. Defaults to this script's parent directory.")
    parser.add_argument("--check", action="store_true", help="Fail if vendored files differ from root sources.")
    return parser.parse_args(argv)


def repo_root(explicit: str | None = None) -> Path:
    if explicit:
        return Path(explicit).resolve()
    return Path(__file__).resolve().parents[1]


def _iter_files(base: Path) -> list[Path]:
    files: list[Path] = []
    if not base.exists():
        return files
    for path in sorted(base.rglob("*")):
        if not path.is_file():
            continue
        if any(part in IGNORED_PARTS for part in path.parts):
            continue
        if path.suffix in IGNORED_SUFFIXES:
            continue
        files.append(path)
    return files


def _dir_diffs(source: Path, target: Path, source_label: str, target_label: str) -> list[str]:
    diffs: list[str] = []
    source_files = {path.relative_to(source).as_posix(): path for path in _iter_files(source)}
    target_files = {path.relative_to(target).as_posix(): path for path in _iter_files(target)}

    for rel in sorted(source_files):
        target_path = target_files.get(rel)
        if target_path is None:
            diffs.append(f"missing vendored file: {target_label}/{rel}")
            continue
        if source_files[rel].read_bytes() != target_path.read_bytes():
            diffs.append(f"vendored file differs: {target_label}/{rel}")

    extras = sorted(set(target_files) - set(source_files))
    diffs.extend(f"unexpected vendored file: {target_label}/{rel}" for rel in extras)
    return diffs


def gather_diffs(root: Path) -> list[str]:
    diffs: list[str] = []
    for source_rel, target_rel in SYNC_MAP:
        source = root / source_rel
        target = root / target_rel
        if source.is_dir():
            if not target.exists():
                diffs.append(f"missing vendored directory: {target_rel}")
                continue
            diffs.extend(_dir_diffs(source, target, source_rel, target_rel))
            continue
        if not source.exists():
            diffs.append(f"missing source file: {source_rel}")
            continue
        if not target.exists():
            diffs.append(f"missing vendored file: {target_rel}")
            continue
        if source.read_bytes() != target.read_bytes():
            diffs.append(f"vendored file differs: {target_rel}")
    return diffs


def apply_sync(root: Path) -> None:
    for source_rel, target_rel in SYNC_MAP:
        source = root / source_rel
        target = root / target_rel
        target.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            if target.exists():
                shutil.rmtree(target)
            target.mkdir(parents=True, exist_ok=True)
            for path in _iter_files(source):
                rel = path.relative_to(source)
                out = target / rel
                out.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(path, out)
            continue
        shutil.copy2(source, target)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    root = repo_root(args.root)
    if args.check:
        diffs = gather_diffs(root)
        if diffs:
            for item in diffs:
                print(f"[sync-app-vendor] {item}", file=sys.stderr)
            return 1
        print("[sync-app-vendor] OK")
        return 0
    apply_sync(root)
    print("[sync-app-vendor] synced app vendor files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
