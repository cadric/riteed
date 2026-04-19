#!/usr/bin/env python3
from __future__ import annotations

if __name__ == "__main__" and (__package__ is None or __package__ == ""):
    import runpy
    import sys
    from pathlib import Path

    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    try:
        runpy.run_module("tools.coverage_check", run_name="__main__")
    except (ImportError, ModuleNotFoundError):
        print(
            "[coverage-check] failed to resolve the tools package; run this from the policy-pack repo or use `python3 -m tools.coverage_check`",
            file=sys.stderr,
        )
        raise SystemExit(1)
    raise SystemExit(0)

import argparse
import json
import os
import tempfile
from pathlib import Path
from typing import Any

from tools.validation_tooling import contract_root, load_json, repo_root, require_tool, run_checked

def validation_policy(root: Path) -> dict[str, Any]:
    return load_json(contract_root(root) / "policy" / "validation-tooling.policy.json")

def extract_line_percent(payload: dict[str, Any]) -> float:
    for candidate in (
        payload.get("data", [{}])[0].get("totals", {}).get("lines", {}).get("percent"),
        payload.get("data", [{}])[0].get("summary", {}).get("lines", {}).get("percent"),
        payload.get("totals", {}).get("lines", {}).get("percent"),
    ):
        if candidate is not None:
            return float(candidate)
    raise SystemExit("[coverage-check] unable to find line coverage percent in cargo-llvm-cov JSON")

def worst_files(payload: dict[str, Any], limit: int = 10) -> list[str]:
    files = payload.get("data", [{}])[0].get("files", [])
    ranked: list[tuple[float, str, int, int]] = []
    for item in files:
        if not isinstance(item, dict):
            continue
        name = str(item.get("filename", "")).strip()
        summary = item.get("summary", {})
        lines = summary.get("lines", {}) if isinstance(summary, dict) else {}
        count = int(lines.get("count", 0) or 0)
        covered = int(lines.get("covered", 0) or 0)
        if count <= 0 or not name:
            continue
        percent = (covered * 100.0) / count
        ranked.append((percent, name, covered, count))
    ranked.sort(key=lambda row: (row[0], row[1]))
    return [f"{name}: {percent:.1f}% ({covered}/{count})" for percent, name, covered, count in ranked[:limit]]

def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Hard-fail Rust line coverage gate for strict GNOME apps.")
    parser.add_argument("--root", help="Repository root. Defaults to auto-detection.")
    parser.add_argument("--json-summary", help="Existing cargo-llvm-cov JSON summary file to validate.")
    return parser.parse_args()

def main() -> int:
    args = parse_args()
    root = repo_root(args.root)
    cfg = validation_policy(root)
    threshold = float(cfg["thresholds"]["min_line_coverage_percent"])
    coverage = cfg.get("coverage_validation", {})
    if args.json_summary:
        payload = json.loads(Path(args.json_summary).read_text(encoding="utf-8"))
    else:
        for tool in coverage.get("required_tools", ["cargo", "cargo-llvm-cov"]):
            require_tool(str(tool))
        with tempfile.TemporaryDirectory(prefix="cargo-llvm-cov-") as tmpdir:
            out = Path(tmpdir) / "coverage.json"
            command = [str(out) if part == "<output-path>" else str(part) for part in coverage.get("default_command", ["cargo", "llvm-cov", "--workspace", "--all-features", "--json", "--summary-only", "--output-path", "<output-path>"])]
            run_checked(
                command,
                root,
                "cargo llvm-cov failed",
                env={"GSK_RENDERER": os.environ.get("GSK_RENDERER", "cairo")},
            )
            payload = json.loads(out.read_text(encoding="utf-8"))
    percent = extract_line_percent(payload)
    if percent < threshold:
        print(f"[coverage-check] line coverage {percent:.1f}% is below required minimum {threshold:.1f}%")
        for line in worst_files(payload):
            print(f"[coverage-check] {line}")
        return 1
    print(f"[coverage-check] OK - line coverage {percent:.1f}% (minimum {threshold:.1f}%)")
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
