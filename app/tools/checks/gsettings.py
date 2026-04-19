from __future__ import annotations

import re
from pathlib import Path

from tools.checks.foundation import gsettings_policy
from tools.scanners.rust import gsettings_review_hits, rust_files, startup_like_write
from tools.scanners.sites import load_review_entries, validate_review_links
from tools.validation_tooling import grep_any, read_text, scoped_files


def check_gsettings(root: Path, app_id: str | None, errors: list[str]) -> None:
    policy = gsettings_policy(root)
    schema_files = scoped_files(root, ["data/schemas/**/*.gschema.xml"])
    source = rust_files(root)
    if schema_files and not grep_any(root, source, r"\b(gio::Settings|Settings::new(_with_path|_full)?|settings\.bind)\b"):
        errors.append("Source must use gio::Settings when GSettings schemas are present")
    for schema in schema_files:
        text = read_text(schema)
        match = re.search(r'<schema[^>]+id="([^"]+)"', text)
        if app_id and match and not match.group(1).startswith(app_id):
            errors.append(f"{schema.relative_to(root).as_posix()}: schema id must align with application id {app_id}")

    enforcement = policy.get("enforcement", {})
    hits = gsettings_review_hits(root, enforcement["write_pattern"], enforcement["bind_pattern"])
    entries = load_review_entries(root, "gsettings", validation_policy(root)["review_artifacts"]["gsettings"], errors)
    validate_review_links(root, hits, entries, errors)

    signal_terms = [term.lower() for term in enforcement.get("per_keystroke_signals", [])]
    startup_terms = [term.lower() for term in enforcement.get("startup_contexts", [])]
    for hit in hits:
        if hit.kind != "gsettings-write":
            continue
        abs_path = root / hit.path
        if startup_like_write(abs_path, hit.line, startup_terms):
            errors.append(f"{hit.path}:{hit.line}: GSettings writes during startup-like setup are forbidden")
        lines = read_text(abs_path).splitlines()
        window = "\n".join(lines[max(0, hit.line - 5) : min(len(lines), hit.line + 4)]).lower()
        if any(term in window for term in signal_terms):
            errors.append(f"{hit.path}:{hit.line}: GSettings writes on every keystroke are forbidden")


def validation_policy(root: Path) -> dict[str, object]:
    from tools.checks.foundation import validation_policy as _validation_policy

    return _validation_policy(root)
