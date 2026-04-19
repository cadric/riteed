from __future__ import annotations

from pathlib import Path

from tools.checks.foundation import hig_policy
from tools.scanners.blueprint import icon_only_buttons as blueprint_icon_only
from tools.scanners.ui_xml import icon_only_buttons as ui_icon_only
from tools.scanners.sites import ReviewEntry
from tools.scanners.rust import source_regex_hits


def check_hig(root: Path, errors: list[str], ui_entries: list[ReviewEntry]) -> None:
    policy = hig_policy(root)
    enforcement = policy.get("enforcement", {})
    errors.extend(source_regex_hits(root, enforcement.get("hard_fail_patterns", [])))
    errors.extend(ui_icon_only(root))
    errors.extend(blueprint_icon_only(root))
    max_items = int(enforcement.get("primary_menu_max_items", 12))
    required_items = {item.lower() for item in enforcement.get("required_primary_menu_items", [])}
    forbidden_items = {item.lower() for item in enforcement.get("forbidden_primary_menu_items", [])}
    for entry in ui_entries:
        payload = entry.payload
        if entry.kind == "surface":
            if not payload.get("adaptive_pattern") or not payload.get("collapse_behavior"):
                errors.append(f"{entry.source_file}: surface review entry must document adaptive_pattern and collapse_behavior")
            if payload.get("copy_reviewed") is not True or payload.get("a11y_reviewed") is not True:
                errors.append(f"{entry.source_file}: surface review entry must confirm copy_reviewed and a11y_reviewed")
        if entry.kind == "menu":
            items = payload.get("items")
            if not isinstance(items, int) or items < 1:
                errors.append(f"{entry.source_file}: menu review entry must declare positive integer items")
                continue
            if items > max_items:
                errors.append(f"{entry.source_file}: primary menu exceeds max items {max_items}")
            standard_items = {str(item).lower() for item in payload.get("standard_items", []) if isinstance(item, str)}
            missing = sorted(required_items - standard_items)
            if missing:
                errors.append(f"{entry.source_file}: menu review entry is missing standard items {missing}")
            forbidden = sorted(forbidden_items & standard_items)
            if forbidden:
                errors.append(f"{entry.source_file}: primary menu includes forbidden standard items {forbidden}")
