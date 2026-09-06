from __future__ import annotations

import tomllib
from typing import Any


def check_source_inventory(
    sources: list[Any], expected: set[str], contract: dict[str, Any], errors: list[str]
) -> None:
    """Account for every generated source; never filter unknown entries away."""
    archive_fields = contract.get('archive_fields')
    inline_fields = contract.get('inline_fields')
    vendor = contract.get('vendor_config')
    if (not isinstance(archive_fields, list) or not archive_fields
            or not all(isinstance(v, str) for v in archive_fields)
            or not isinstance(inline_fields, list) or not inline_fields
            or not all(isinstance(v, str) for v in inline_fields)
            or not isinstance(vendor, dict)):
        errors.append('dependency preflight: missing or invalid cargo_source_inventory policy')
        return
    for index, item in enumerate(sources):
        prefix = f'dependency preflight: cargo source {index}'
        if not isinstance(item, dict):
            errors.append(f'{prefix} must be a generated source object')
            continue
        kind = item.get('type')
        fields = archive_fields if kind == 'archive' else inline_fields if kind == 'inline' else None
        if fields is None or set(item) != set(fields):
            errors.append(f'{prefix} has unapproved type or fields')
            continue
        if not all(isinstance(v, str) for v in item.values()):
            errors.append(f'{prefix} source fields must be strings')
            continue
        dest = item['dest']
        if kind == 'archive':
            if dest not in expected:
                errors.append(f'{prefix} archive destination is absent from Cargo.lock: {dest!r}')
        elif dest in expected and item['dest-filename'] == '.cargo-checksum.json':
            continue  # Contents are verified against Cargo.lock by dependency_preflight.
        elif dest == vendor.get('dest') and item['dest-filename'] == vendor.get('dest-filename'):
            try:
                contents = tomllib.loads(item['contents'])
            except tomllib.TOMLDecodeError:
                errors.append(f'{prefix} vendor config must be valid TOML')
                continue
            if contents != vendor.get('contents'):
                errors.append(f'{prefix} vendor config must exactly match the reviewed policy')
        else:
            errors.append(f'{prefix} inline source is not an expected checksum or vendor config')
