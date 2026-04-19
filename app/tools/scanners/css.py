from __future__ import annotations

import re
from pathlib import Path

from tools.scanners.sites import ScanHit
from tools.validation_tooling import normalize_path, read_text, scoped_files


def first_content_line(path: Path) -> tuple[int, str] | None:
    for index, line in enumerate(read_text(path).splitlines(), start=1):
        stripped = line.strip()
        if stripped and not stripped.startswith("/*") and not stripped.startswith("*") and not stripped.startswith("//"):
            return index, line
    return None


def css_review_hits(root: Path) -> list[ScanHit]:
    hits: list[ScanHit] = []
    for path in scoped_files(root, ["data/style/**/*.css"]):
        first = first_content_line(path)
        if first is None:
            continue
        line_no, line = first
        hits.append(
            ScanHit(
                path=normalize_path(path.relative_to(root).as_posix()),
                line=line_no,
                kind="css-file",
                match=line.strip(),
                message="Custom CSS requires a scoped review artifact entry.",
            )
        )
    return hits


def regex_hits(root: Path, patterns: list[dict[str, str]]) -> list[str]:
    findings: list[str] = []
    for item in patterns:
        regex = re.compile(item["pattern"])
        for path in scoped_files(root, item["paths"]):
            for index, line in enumerate(read_text(path).splitlines(), start=1):
                if regex.search(line):
                    findings.append(f"{path.relative_to(root).as_posix()}:{index}: {item['message']}")
    return findings
