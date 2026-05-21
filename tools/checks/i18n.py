from __future__ import annotations

import ast
import re
from pathlib import Path

from tools.checks.foundation import gettext_bootstrap_present, gettext_policy
from tools.scanners.pot import message_keys
from tools.scanners.rust import rust_files, short_gettext_hits, translator_comment_present
from tools.scanners.sites import load_review_entries, validate_review_links
from tools.validation_tooling import grep_any, read_text, relpath, scoped_files

_LOCALE_TOKEN = re.compile(r"^[A-Za-z]{2,3}(?:_[A-Za-z][A-Za-z0-9]*)*(?:\.[A-Za-z0-9_.-]+)?(?:@[A-Za-z0-9_.-]+)?$")


def check_i18n(root: Path, app_id: str | None, errors: list[str]) -> None:
    policy = gettext_policy(root)
    source = rust_files(root)
    po_files = scoped_files(root, ["po/*.po"])
    pot_files = scoped_files(root, ["po/*.pot"])
    if not po_files:
        errors.append("At least one po/*.po catalog is required")
    if not pot_files:
        errors.append("At least one po/*.pot template is required")
    check_linguas_catalogs(root, errors)
    if not gettext_bootstrap_present(root):
        errors.append("gettext bootstrap is required before UI construction")
    if app_id and not grep_any(root, source, app_id.replace(".", r"\.")):
        errors.append(f"Application id {app_id} must appear in source constants or initialization code")

    enforcement = policy.get("enforcement", {})
    hits = short_gettext_hits(root, int(enforcement.get("short_string_max_length", 20)))
    entries = load_review_entries(root, "i18n", validation_policy(root)["review_artifacts"]["i18n"], errors)
    validate_review_links(root, hits, entries, errors)
    prefix = str(policy["translator_support"]["translator_comment_prefix"])
    for entry in entries:
        if entry.kind != "i18n-short-string":
            continue
        if entry.payload.get("comment_required") is True:
            if not translator_comment_present(root / entry.path, entry.line, prefix):
                errors.append(f"{entry.source_file}: translator comment {prefix!r} is required for {entry.path}:{entry.line}")


def check_linguas_catalogs(root: Path, errors: list[str]) -> None:
    linguas = root / "po" / "LINGUAS"
    if not linguas.exists():
        errors.append("po/LINGUAS is required to declare supported locales")
        return
    pot_messages = normalized_pot_messages(root)
    for locale in _linguas_locales(linguas):
        if not _valid_linguas_locale(locale):
            errors.append(f"po/LINGUAS locale {locale!r}: invalid locale token")
            continue
        po_path = root / "po" / f"{locale}.po"
        if not po_path.exists():
            errors.append(f"po/LINGUAS locale {locale}: missing po/{locale}.po")
            continue
        missing = pot_messages - message_keys(po_path)
        if missing:
            errors.append(f"po/LINGUAS locale {locale}: missing POT entries in po/{locale}.po")
        fuzzy, untranslated = _po_catalog_state(po_path)
        if fuzzy:
            errors.append(f"po/LINGUAS locale {locale}: fuzzy entries remain in po/{locale}.po")
        if untranslated:
            errors.append(f"po/LINGUAS locale {locale}: untranslated entries remain in po/{locale}.po")


def _linguas_locales(path: Path) -> list[str]:
    locales: list[str] = []
    for raw in read_text(path).splitlines():
        line = raw.split("#", 1)[0].strip()
        if line:
            locales.extend(line.split())
    return locales


def _valid_linguas_locale(locale: str) -> bool:
    if not locale or "/" in locale or "\\" in locale or ".." in locale:
        return False
    return bool(_LOCALE_TOKEN.fullmatch(locale))


def _po_catalog_state(path: Path) -> tuple[int, int]:
    fuzzy = 0
    untranslated = 0
    entry: dict[str, object] = {"msgid": None, "msgid_plural": None, "msgstrs": {}, "fuzzy": False}
    field: tuple[str, str | None] | None = None
    for raw in [*read_text(path).splitlines(), ""]:
        line = raw.strip()
        if not line:
            entry_fuzzy, entry_untranslated = _po_entry_state(entry)
            fuzzy += entry_fuzzy
            untranslated += entry_untranslated
            entry = {"msgid": None, "msgid_plural": None, "msgstrs": {}, "fuzzy": False}
            field = None
            continue
        if line.startswith("#,"):
            flags = {flag.strip() for flag in line[2:].split(",")}
            entry["fuzzy"] = bool(entry["fuzzy"]) or "fuzzy" in flags
            continue
        if line.startswith("#"):
            continue
        if line.startswith("msgid_plural "):
            entry["msgid_plural"] = _po_quoted(line)
            field = ("msgid_plural", None)
        elif line.startswith("msgid "):
            entry["msgid"] = _po_quoted(line)
            field = ("msgid", None)
        elif line.startswith("msgstr["):
            key = line.split("]", 1)[0].removeprefix("msgstr[")
            _msgstrs(entry)[key] = _po_quoted(line)
            field = ("msgstr", key)
        elif line.startswith("msgstr "):
            _msgstrs(entry)[""] = _po_quoted(line)
            field = ("msgstr", "")
        elif line.startswith('"') and field is not None:
            _append_po_continuation(entry, field, _po_quoted(line))
    return fuzzy, untranslated


def _po_entry_state(entry: dict[str, object]) -> tuple[int, int]:
    msgid = entry["msgid"]
    if msgid in (None, ""):
        return 0, 0
    msgstrs = _msgstrs(entry)
    return int(bool(entry["fuzzy"])), int(not msgstrs or any(value == "" for value in msgstrs.values()))


def _msgstrs(entry: dict[str, object]) -> dict[str, str]:
    return entry["msgstrs"]  # type: ignore[return-value]


def _append_po_continuation(entry: dict[str, object], field: tuple[str, str | None], value: str) -> None:
    name, key = field
    if name == "msgstr" and key is not None:
        _msgstrs(entry)[key] = _msgstrs(entry).get(key, "") + value
    else:
        entry[name] = str(entry[name] or "") + value


def _po_quoted(line: str) -> str:
    try:
        value = ast.literal_eval(line[line.index('"') :])
    except (SyntaxError, ValueError):
        return ""
    return value if isinstance(value, str) else ""


def validation_policy(root: Path) -> dict[str, object]:
    from tools.checks.foundation import validation_policy as _validation_policy

    return _validation_policy(root)


def normalized_pot_messages(root: Path) -> set[tuple[str | None, str, str | None]]:
    keys: set[tuple[str | None, str, str | None]] = set()
    for path in scoped_files(root, ["po/*.pot"]):
        keys |= message_keys(path)
    return keys
