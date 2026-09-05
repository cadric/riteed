from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from tools.validation_tooling import load_toml


def check_toolchain_policy(root: Path, policy: dict[str, Any], errors: list[str]) -> None:
    data = load_toml(root / 'rust-toolchain.toml')
    target = policy.get('targets', {}).get('rust', {})
    family = target.get('target_rust_family')
    family_match = re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.x', family) if isinstance(family, str) else None
    if family_match is None:
        errors.append('rust policy target_rust_family must be a major.minor.x family')
        return
    cfg = data.get('toolchain', {})
    if not isinstance(cfg, dict):
        errors.append('rust-toolchain.toml must define a toolchain table')
        return
    channel = cfg.get('channel')
    version = re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', channel) if isinstance(channel, str) else None
    if version is None or version.groups()[:2] != family_match.groups():
        errors.append(f'rust-toolchain.toml must pin stable Rust {family}, found {channel!r}')
    required = policy.get('toolchain', {}).get('required_components')
    if not isinstance(required, list) or not required or not all(isinstance(v, str) and v.strip() for v in required):
        errors.append('rust policy toolchain.required_components must be a non-empty string list')
        return
    components = cfg.get('components', [])
    if not isinstance(components, list) or not all(isinstance(v, str) for v in components):
        errors.append('rust-toolchain.toml components must be a string list')
        return
    for component in required:
        if component not in components:
            errors.append(f'rust-toolchain.toml must include component {component!r}')
