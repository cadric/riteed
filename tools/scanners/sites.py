from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from tools.validation_tooling import line_text, load_json, normalize_path, read_text, scoped_files


@dataclass(frozen=True)
class ScanHit:
    path: str
    line: int
    kind: str
    match: str
    message: str


@dataclass(frozen=True)
class ReviewEntry:
    path: str
    line: int
    kind: str
    match: str
    source_file: str
    payload: dict[str, Any]


def _entry_key(path: str, line: int, kind: str) -> tuple[str, int, str]:
    return normalize_path(path), line, kind.strip()


def _review_path(root: Path, rel: str) -> Path | None:
    normalized = normalize_path(rel)
    posix_path = PurePosixPath(normalized)
    if (
        not normalized
        or normalized == "."
        or posix_path.is_absolute()
        or any(part in {"", ".", ".."} for part in posix_path.parts)
        or (posix_path.parts and ":" in posix_path.parts[0])
    ):
        return None
    candidate = (root / Path(*posix_path.parts)).resolve()
    try:
        candidate.relative_to(root.resolve())
    except ValueError:
        return None
    return candidate


def load_review_entries(
    root: Path,
    domain: str,
    config: dict[str, Any],
    errors: list[str],
) -> list[ReviewEntry]:
    entries: list[ReviewEntry] = []
    for path in scoped_files(root, config.get("globs", [])):
        payload = load_json(path)
        version = payload.get("version")
        if not isinstance(version, int):
            errors.append(f"{path.relative_to(root)}: review artifact must define integer version")
        sections = config.get("sections", {})
        for section_name, section_cfg in sections.items():
            raw_items = payload.get(section_name, [])
            if raw_items == []:
                continue
            if not isinstance(raw_items, list):
                errors.append(f"{path.relative_to(root)}: section {section_name!r} must be an array")
                continue
            for item in raw_items:
                if not isinstance(item, dict):
                    errors.append(f"{path.relative_to(root)}: section {section_name!r} entries must be objects")
                    continue
                kind = str(section_cfg.get("kind") or item.get("kind") or "").strip()
                if not kind:
                    errors.append(f"{path.relative_to(root)}: section {section_name!r} entries must define kind")
                    continue
                missing = [
                    field
                    for field in section_cfg.get("required_fields", [])
                    if item.get(field) in (None, "")
                ]
                if missing:
                    errors.append(
                        f"{path.relative_to(root)}: section {section_name!r} entry is missing required fields {missing}"
                    )
                    continue
                rel = normalize_path(str(item.get("path", "")))
                line_no = item.get("line")
                if not isinstance(line_no, int) or line_no < 1:
                    errors.append(f"{path.relative_to(root)}: review entry line must be a 1-based integer")
                    continue
                match = str(item.get("match", ""))
                entries.append(
                    ReviewEntry(
                        path=rel,
                        line=line_no,
                        kind=kind,
                        match=match,
                        source_file=normalize_path(path.relative_to(root).as_posix()),
                        payload=item,
                    )
                )
    return entries


def validate_review_links(root: Path, hits: list[ScanHit], entries: list[ReviewEntry], errors: list[str]) -> None:
    hit_map: dict[tuple[str, int, str], ScanHit] = {}
    for hit in hits:
        key = _entry_key(hit.path, hit.line, hit.kind)
        if key in hit_map:
            errors.append(
                f"{hit.path}:{hit.line}: scanner emitted duplicate review-required hit for kind {hit.kind!r}"
            )
            continue
        hit_map[key] = hit

    entry_map: dict[tuple[str, int, str], ReviewEntry] = {}
    for entry in entries:
        key = _entry_key(entry.path, entry.line, entry.kind)
        if key in entry_map:
            errors.append(
                f"{entry.source_file}: duplicate review entry for {entry.path}:{entry.line}:{entry.kind}"
            )
            continue
        file_path = _review_path(root, entry.path)
        if file_path is None:
            errors.append(f"{entry.source_file}: invalid review entry path: {entry.path!r}")
            continue
        entry_map[key] = entry
        if not file_path.exists():
            errors.append(f"{entry.source_file}: review entry path does not exist: {entry.path}")
            continue
        text = read_text(file_path)
        actual = line_text(text, entry.line)
        if actual is None:
            errors.append(f"{entry.source_file}: review entry line out of range for {entry.path}:{entry.line}")
            continue
        if entry.match not in actual:
            errors.append(
                f"{entry.source_file}: review entry match is not present on {entry.path}:{entry.line}"
            )

    for key, hit in hit_map.items():
        if key not in entry_map:
            errors.append(f"{hit.path}:{hit.line}: missing review entry for kind {hit.kind!r}")
    for key, entry in entry_map.items():
        if key not in hit_map:
            errors.append(
                f"{entry.source_file}: stale review entry for {entry.path}:{entry.line}:{entry.kind}"
            )
