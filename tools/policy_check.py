#!/usr/bin/env python3
from __future__ import annotations

if __name__ == "__main__" and (__package__ is None or __package__ == ""):
    import runpy
    import sys
    from pathlib import Path

    sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
    try:
        runpy.run_module("tools.policy_check", run_name="__main__")
    except (ImportError, ModuleNotFoundError):
        print(
            "[policy-check] failed to resolve the tools package; run this from the policy-pack repo or use `python3 -m tools.policy_check`",
            file=sys.stderr,
        )
        raise SystemExit(1)
    raise SystemExit(0)

import argparse

from tools.checks import (
    commands,
    dependency_preflight,
    foundation,
    gsettings,
    hig,
    i18n,
    libadwaita,
    line_limits,
    release,
    runtime,
    stress_fuzz,
)
from tools.validation_tooling import contract_root, repo_root


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Hard-fail policy checker for strict GNOME Rust apps.")
    parser.add_argument("--root", help="Repository root. Defaults to auto-detection.")
    parser.add_argument("--strict", action="store_true", help="Accepted for compatibility; strict mode is always enabled.")
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--update-artifact-index", action="store_true", help="Regenerate the bundle artifact index in the policy-pack repo.")
    modes.add_argument("--policy-pack-check", action="store_true", help="Validate the policy-pack contract without running app checks.")
    return parser.parse_args()


def _print_errors(errors: list[str]) -> int:
    for item in errors:
        print(f"[policy-check] {item}")
    return 1


def main() -> int:
    args = parse_args()
    if args.policy_pack_check:
        target = repo_root(args.root, allow_policy_pack=True)
        root = contract_root(target)
        errors: list[str] = []
        foundation.check_policy_stack(root, errors)
        line_limits.check_line_limits(root, errors, scope="policy-pack")
        if errors:
            return _print_errors(errors)
        print("[policy-check] OK")
        return 0

    root = repo_root(args.root, allow_policy_pack=args.update_artifact_index)
    if args.update_artifact_index:
        return commands.run_update_artifact_index(root, args)

    errors: list[str] = []
    foundation.check_policy_stack(root, errors)
    foundation.check_repo_layout(root, errors)
    dependency_preflight.check_dependency_preflight(root, errors)
    foundation.check_toolchain(root, errors)
    foundation.check_manifests(root, errors)
    foundation.check_crate_roots(root, errors)
    foundation.check_forbidden_patterns(root, errors)
    foundation.check_required_patterns(root, errors)
    foundation.check_line_limits(root, errors)
    app_id = foundation.check_flatpak_and_identity(root, errors)
    foundation.check_resources(root, app_id, errors)
    foundation.check_ui_localization(root, errors)
    ui_entries = libadwaita.check_libadwaita(root, errors)
    hig.check_hig(root, errors, ui_entries)
    i18n.check_i18n(root, app_id, errors)
    gsettings.check_gsettings(root, app_id, errors)
    runtime.check_runtime(root, errors)
    release.check_release(root, errors)
    stress_fuzz.check_stress_fuzz(root, errors)
    if errors:
        return _print_errors(errors)
    commands.run_required_commands(root, errors)
    if errors:
        return _print_errors(errors)
    print("[policy-check] OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
