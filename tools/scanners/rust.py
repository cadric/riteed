from __future__ import annotations

import re
from pathlib import Path

from tools.scanners.sites import ScanHit
from tools.validation_tooling import match_any, normalize_path, read_text, relpath, scoped_files

RUST_GLOBS = ["src/**/*.rs", "crates/**/*.rs"]
GETTEXT_CALL_RE = re.compile(
    r"\b(?P<name>gettext|ngettext|pgettext|npgettext)\s*\(\s*\"(?P<msgid>[^\"]+)\""
)


def rust_files(root: Path) -> list[Path]:
    return scoped_files(root, RUST_GLOBS)


def _test_only_file(rel: str) -> bool:
    return (
        rel.endswith("/tests.rs")
        or rel.endswith("/test_support.rs")
        or rel.startswith("src/gtk_tests")
    )


def _cfg_test_lines(lines: list[str]) -> set[int]:
    ignored: set[int] = set()
    pending = False
    depth: int | None = None
    for index, line in enumerate(lines, start=1):
        stripped = line.strip()
        if "#[cfg(test)]" in stripped or "#[cfg(any(test" in stripped:
            ignored.add(index)
            pending = True
            continue
        if pending:
            ignored.add(index)
            if not stripped or stripped.startswith("#["):
                continue
            delta = line.count("{") - line.count("}")
            if "{" in line and delta > 0:
                depth = delta
                pending = False
            else:
                pending = False
                depth = None
            continue
        if depth is not None:
            ignored.add(index)
            depth += line.count("{") - line.count("}")
            if depth <= 0:
                depth = None
    return ignored


def source_regex_hits(root: Path, patterns: list[dict[str, object]]) -> list[str]:
    findings: list[str] = []
    for item in patterns:
        regex = re.compile(str(item["pattern"]))
        exceptions = item.get("exceptions", [])
        paths = [
            path
            for path in scoped_files(root, item["paths"])
            if not match_any(relpath(path, root), exceptions)
        ]
        for path in paths:
            for index, line in enumerate(read_text(path).splitlines(), start=1):
                if regex.search(line):
                    findings.append(f"{path.relative_to(root).as_posix()}:{index}: {item['message']}")
    return findings


def _context_window(lines: list[str], line_no: int, size: int = 8) -> str:
    start = max(0, line_no - size - 1)
    end = min(len(lines), line_no + size)
    return "\n".join(lines[start:end])


def runtime_review_hits(root: Path, patterns: list[dict[str, object]]) -> list[ScanHit]:
    hits: list[ScanHit] = []
    for item in patterns:
        regex = re.compile(str(item["pattern"]))
        kind = str(item["kind"])
        message = str(item["message"])
        ignore_test_only = bool(item.get("ignore_test_only"))
        for path in scoped_files(root, item.get("paths", RUST_GLOBS)):
            rel = normalize_path(path.relative_to(root).as_posix())
            if ignore_test_only and _test_only_file(rel):
                continue
            lines = read_text(path).splitlines()
            ignored = _cfg_test_lines(lines) if ignore_test_only else set()
            for index, line in enumerate(lines, start=1):
                if index in ignored:
                    continue
                if regex.search(line):
                    hits.append(
                        ScanHit(
                            path=rel,
                            line=index,
                            kind=kind,
                            match=line.strip(),
                            message=message,
                        )
                    )
    return hits


def gsettings_review_hits(root: Path, write_pattern: str, bind_pattern: str) -> list[ScanHit]:
    write_re = re.compile(write_pattern)
    bind_re = re.compile(bind_pattern)
    hits: list[ScanHit] = []
    for path in rust_files(root):
        for index, line in enumerate(read_text(path).splitlines(), start=1):
            if write_re.search(line):
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="gsettings-write",
                        match=line.strip(),
                        message="GSettings write sites require review coverage.",
                    )
                )
            if bind_re.search(line):
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="gsettings-bind",
                        match=line.strip(),
                        message="GSettings binding sites require review coverage.",
                    )
                )
    return hits


def short_gettext_hits(root: Path, max_length: int) -> list[ScanHit]:
    hits: list[ScanHit] = []
    for path in rust_files(root):
        for index, line in enumerate(read_text(path).splitlines(), start=1):
            match = GETTEXT_CALL_RE.search(line)
            if not match:
                continue
            if match.group("name") in {"pgettext", "npgettext"}:
                continue
            msgid = match.group("msgid").strip()
            if 0 < len(msgid) <= max_length:
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="i18n-short-string",
                        match=line.strip(),
                        message="Short gettext strings require explicit context review.",
                    )
                )
    return hits


def translator_comment_present(path: Path, line_no: int, prefix: str) -> bool:
    lines = read_text(path).splitlines()
    for offset in range(1, 3):
        index = line_no - offset
        if index < 1:
            break
        candidate = lines[index - 1].strip()
        if not candidate:
            continue
        if prefix in candidate:
            return True
        if not candidate.startswith("//"):
            break
    return False


def startup_like_write(path: Path, line_no: int, contexts: list[str]) -> bool:
    lines = read_text(path).splitlines()
    window = _context_window(lines, line_no)
    lowered = window.lower()
    return any(ctx.lower() in lowered for ctx in contexts)


def async_blocking_hit(path: Path, line_no: int) -> bool:
    lines = read_text(path).splitlines()
    window = _context_window(lines, line_no, size=12)
    return "async fn" in window or "async move {" in window or "async {" in window
