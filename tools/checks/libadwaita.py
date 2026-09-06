from __future__ import annotations

from pathlib import Path

from tools.checks.foundation import libadwaita_policy, validation_policy
from tools.scanners.blueprint import blueprint_surface_hits
from tools.scanners.css import css_review_hits, regex_hits as css_regex_hits
from tools.scanners.sites import ReviewEntry, load_review_entries, validate_review_links
from tools.scanners.ui_xml import ui_surface_hits


def check_libadwaita(root: Path, errors: list[str]) -> list[ReviewEntry]:
    policy = libadwaita_policy(root)
    enforcement = policy.get("enforcement", {})
    errors.extend(css_regex_hits(root, enforcement.get("hard_fail_patterns", [])))
    hits = ui_surface_hits(root, errors) + blueprint_surface_hits(root) + css_review_hits(root)
    config = validation_policy(root)["review_artifacts"]["ui"]
    entries = load_review_entries(root, "ui", config, errors)
    validate_review_links(root, hits, entries, errors)
    return entries
