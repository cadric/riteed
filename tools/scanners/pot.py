from __future__ import annotations

from pathlib import Path

from tools.validation_tooling import read_text

MessageKey = tuple[str | None, str, str | None]


def _parse_quoted(line: str) -> str:
    body = line.split(" ", 1)[1].strip()
    if body.startswith('"') and body.endswith('"'):
        return bytes(body[1:-1], "utf-8").decode("unicode_escape")
    return ""


def message_keys(path: Path) -> set[MessageKey]:
    text = read_text(path)
    entries: set[MessageKey] = set()
    current: dict[str, str | None] = {"msgctxt": None, "msgid": None, "msgid_plural": None}
    field: str | None = None
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            if current["msgid"] not in (None, ""):
                entries.add((current["msgctxt"], current["msgid"] or "", current["msgid_plural"]))
            current = {"msgctxt": None, "msgid": None, "msgid_plural": None}
            field = None
            continue
        if line.startswith("msgctxt "):
            current["msgctxt"] = _parse_quoted(line)
            field = "msgctxt"
            continue
        if line.startswith("msgid_plural "):
            current["msgid_plural"] = _parse_quoted(line)
            field = "msgid_plural"
            continue
        if line.startswith("msgid "):
            current["msgid"] = _parse_quoted(line)
            field = "msgid"
            continue
        if line.startswith("msgstr"):
            field = None
            continue
        if line.startswith('"') and line.endswith('"') and field is not None:
            current[field] = (current[field] or "") + bytes(line[1:-1], "utf-8").decode("unicode_escape")
    if current["msgid"] not in (None, ""):
        entries.add((current["msgctxt"], current["msgid"] or "", current["msgid_plural"]))
    return entries
