from __future__ import annotations

from pathlib import Path

from tools.checks import foundation, remediation, rulesets


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    errors: list[str] = []
    policy = foundation.release_policy(root)
    active = remediation.validate_planned_remediation(policy, rulesets.POLICY_FILE, errors)
    rulesets.check_remote_governance(policy, active, errors)
    for error in errors:
        print(error)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
