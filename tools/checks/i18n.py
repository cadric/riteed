from __future__ import annotations

from pathlib import Path

from tools.checks.foundation import gettext_bootstrap_present, gettext_policy
from tools.scanners.pot import message_keys
from tools.scanners.rust import rust_files, short_gettext_hits, translator_comment_present
from tools.scanners.sites import load_review_entries, validate_review_links
from tools.validation_tooling import grep_any, read_text, relpath, scoped_files


def check_i18n(root: Path, app_id: str | None, errors: list[str]) -> None:
    policy = gettext_policy(root)
    source = rust_files(root)
    po_files = scoped_files(root, ["po/*.po"])
    pot_files = scoped_files(root, ["po/*.pot"])
    if not po_files:
        errors.append("At least one po/*.po catalog is required")
    if not pot_files:
        errors.append("At least one po/*.pot template is required")
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


def validation_policy(root: Path) -> dict[str, object]:
    from tools.checks.foundation import validation_policy as _validation_policy

    return _validation_policy(root)


def normalized_pot_messages(root: Path) -> set[tuple[str | None, str, str | None]]:
    keys: set[tuple[str | None, str, str | None]] = set()
    for path in scoped_files(root, ["po/*.pot"]):
        keys |= message_keys(path)
    return keys
