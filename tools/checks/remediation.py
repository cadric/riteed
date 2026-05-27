from __future__ import annotations

import datetime as dt
import re
from typing import Any


REQUIRED_FIELDS = (
    "finding_id",
    "target_milestone",
    "review_artifact",
    "created",
    "max_age_days",
    "reason",
    "removal_condition",
)
FINDING_RE = re.compile(r"^(RIT-AUD-[0-9]{3}|POLICY-[A-Z0-9_-]+)$")


def validate_planned_remediation(
    policy: dict[str, Any],
    label: str,
    errors: list[str],
    *,
    today: dt.date | None = None,
) -> set[str]:
    today = today or dt.datetime.now(dt.UTC).date()
    entries = policy.get("planned_remediation", [])
    if not isinstance(entries, list):
        errors.append(f"{label}: planned_remediation must be an array")
        return set()
    active: set[str] = set()
    seen: set[str] = set()
    for index, entry in enumerate(entries):
        prefix = f"{label}: planned_remediation[{index}]"
        if not isinstance(entry, dict):
            errors.append(f"{prefix} must be an object")
            continue
        missing = [field for field in REQUIRED_FIELDS if field not in entry]
        if missing:
            errors.append(f"{prefix} missing required fields: {', '.join(missing)}")
            continue
        valid = True
        finding_id = _string(entry, "finding_id")
        if not finding_id or not FINDING_RE.match(finding_id):
            errors.append(f"{prefix}: finding_id must look like RIT-AUD-001 or POLICY-DEBT")
            valid = False
        elif finding_id in seen:
            errors.append(f"{prefix}: duplicate finding_id {finding_id}")
            valid = False
        elif finding_id:
            seen.add(finding_id)
        for field in ("target_milestone", "review_artifact", "reason", "removal_condition"):
            if not _string(entry, field):
                errors.append(f"{prefix}: {field} must be a non-empty string")
                valid = False
        if "approval_required" in entry and not _string(entry, "approval_required"):
            errors.append(f"{prefix}: approval_required must be a non-empty string when present")
            valid = False
        created = _parse_date(_string(entry, "created"), prefix, "created", errors)
        if created is None:
            valid = False
        elif created > today:
            errors.append(f"{prefix}: created must not be after {today}")
            valid = False
        max_age = entry.get("max_age_days")
        if isinstance(max_age, bool) or not isinstance(max_age, int) or max_age <= 0:
            errors.append(f"{prefix}: max_age_days must be a positive integer")
            continue
        if created is not None and created + dt.timedelta(days=max_age) < today:
            errors.append(f"{prefix}: remediation for {finding_id} expired on {created + dt.timedelta(days=max_age)}")
            continue
        if valid and finding_id:
            active.add(finding_id)
    return active


def _string(entry: dict[str, Any], field: str) -> str:
    value = entry.get(field)
    return value.strip() if isinstance(value, str) else ""


def _parse_date(value: str, prefix: str, field: str, errors: list[str]) -> dt.date | None:
    try:
        return dt.date.fromisoformat(value)
    except ValueError:
        errors.append(f"{prefix}: {field} must be ISO YYYY-MM-DD")
        return None
