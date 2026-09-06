from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path
from typing import Any

from tools.validation_tooling import contract_root, repo_root, load_json
from tools.checks.cargo_source_inventory import check_source_inventory

CRATES_IO_INDEX = "registry+https://github.com/rust-lang/crates.io-index"
CRATES_IO_SPARSE_INDEX = "sparse+https://index.crates.io/"
CRATES_IO_SOURCES = {CRATES_IO_INDEX, CRATES_IO_SPARSE_INDEX}
STATIC_CRATE_BASE = "https://static.crates.io/crates"
CARGO_SOURCES_REMEDIATION = (
    "Regenerate app/build-aux/cargo/cargo-sources.json using docs/dependency-updates.md."
)
GTK_RS_STACK_CRATES = (
    "gtk4",
    "gtk4-sys",
    "gtk4-macros",
    "gdk4",
    "gdk4-sys",
    "gsk4",
    "gsk4-sys",
    "libadwaita",
    "libadwaita-sys",
    "sourceview5",
    "sourceview5-sys",
    "glib",
    "glib-sys",
    "glib-macros",
    "gio",
    "gio-sys",
    "gobject-sys",
    "glib-build-tools",
    "gdk-pixbuf",
    "gdk-pixbuf-sys",
    "pango",
    "pango-sys",
    "cairo-rs",
    "cairo-sys-rs",
    "graphene-rs",
    "graphene-sys",
)
EXACT_SAFE_SYS_PAIRS = (
    ("gtk4", "gtk4-sys"),
    ("gdk4", "gdk4-sys"),
    ("gsk4", "gsk4-sys"),
    ("libadwaita", "libadwaita-sys"),
    ("sourceview5", "sourceview5-sys"),
    ("gdk-pixbuf", "gdk-pixbuf-sys"),
    ("cairo-rs", "cairo-sys-rs"),
    ("graphene-rs", "graphene-sys"),
)


def check_dependency_preflight(root: Path, errors: list[str]) -> None:
    app_manifest = _load_toml(root / "Cargo.toml", errors)
    if app_manifest is None:
        return
    if _package_name(app_manifest) != "riteed":
        return
    app_lock = _load_toml(root / "Cargo.lock", errors)
    fuzz_lock = _load_toml(root / "fuzz" / "Cargo.lock", errors)
    cargo_sources = _load_json(
        root / "build-aux" / "cargo" / "cargo-sources.json",
        errors,
        expected_type=list,
        type_name="JSON list",
    )
    policy_root = contract_root(root) / "policy"
    gtk_policy = _load_json(
        policy_root / "gtk4-rs.policy.json", errors, expected_type=dict, type_name="JSON object"
    )
    adw_policy = _load_json(
        policy_root / "libadwaita.policy.json", errors, expected_type=dict, type_name="JSON object"
    )

    _check_direct_stack_pins(app_manifest, errors)
    if app_lock is not None and fuzz_lock is not None:
        _check_app_versions(app_manifest, app_lock, fuzz_lock, errors)
        _check_exact_safe_sys_pairs(app_lock, "Cargo.lock", errors)
        _check_exact_safe_sys_pairs(fuzz_lock, "fuzz/Cargo.lock", errors)
        _check_fuzz_stack_sync(app_lock, fuzz_lock, errors)
    if app_lock is not None and gtk_policy is not None:
        _check_policy_target_version(app_manifest, app_lock, gtk_policy, errors)
    if app_lock is not None and adw_policy is not None:
        _check_policy_target_version(app_manifest, app_lock, adw_policy, errors)
    if app_lock is not None and cargo_sources is not None:
        _check_cargo_sources(app_lock, cargo_sources, errors, load_json(policy_root / "validation-tooling.policy.json").get("cargo_source_inventory", {}))


def _load_toml(path: Path, errors: list[str]) -> dict[str, Any] | None:
    try:
        with path.open("rb") as handle:
            data = tomllib.load(handle)
    except FileNotFoundError:
        errors.append(f"dependency preflight: missing TOML file {path}")
        return None
    except tomllib.TOMLDecodeError as exc:
        errors.append(f"dependency preflight: invalid TOML in {path}: {exc}")
        return None
    except OSError as exc:
        errors.append(f"dependency preflight: failed to read {path}: {exc}")
        return None
    if not isinstance(data, dict):
        errors.append(f"dependency preflight: TOML file {path} must contain a table")
        return None
    return data


def _load_json(path: Path, errors: list[str], *, expected_type: type, type_name: str) -> Any:
    try:
        with path.open("r", encoding="utf-8") as handle:
            data = json.load(handle)
    except FileNotFoundError:
        errors.append(f"dependency preflight: missing JSON file {path}")
        return None
    except json.JSONDecodeError as exc:
        errors.append(f"dependency preflight: invalid JSON in {path}: {exc}")
        return None
    except OSError as exc:
        errors.append(f"dependency preflight: failed to read {path}: {exc}")
        return None
    if not isinstance(data, expected_type):
        errors.append(f"dependency preflight: JSON file {path} must contain a {type_name}")
        return None
    return data


def _package_version(manifest: dict[str, Any]) -> str | None:
    package = manifest.get("package")
    if isinstance(package, dict) and isinstance(package.get("version"), str):
        return package["version"]
    return None


def _package_name(manifest: dict[str, Any]) -> str | None:
    package = manifest.get("package")
    if isinstance(package, dict) and isinstance(package.get("name"), str):
        return package["name"]
    return None


def _packages_by_name(lock_data: dict[str, Any]) -> dict[str, list[dict[str, Any]]]:
    packages: dict[str, list[dict[str, Any]]] = {}
    for item in lock_data.get("package", []):
        if not isinstance(item, dict) or not isinstance(item.get("name"), str):
            continue
        packages.setdefault(str(item["name"]), []).append(item)
    return packages


def _single_package_version(lock_data: dict[str, Any], name: str) -> str | None:
    matches = _packages_by_name(lock_data).get(name, [])
    if len(matches) != 1:
        return None
    version = matches[0].get("version")
    return version if isinstance(version, str) else None


def _check_app_versions(
    app_manifest: dict[str, Any],
    app_lock: dict[str, Any],
    fuzz_lock: dict[str, Any],
    errors: list[str],
) -> None:
    app_version = _package_version(app_manifest)
    if app_version is None:
        errors.append("dependency preflight: Cargo.toml package.version is required")
        return
    for label, lock_data in (("Cargo.lock", app_lock), ("fuzz/Cargo.lock", fuzz_lock)):
        lock_version = _single_package_version(lock_data, "riteed")
        if lock_version != app_version:
            errors.append(
                "dependency preflight: "
                f"{label} riteed version {lock_version!r} must match Cargo.toml version {app_version!r}"
            )


def _dependency_version(manifest: dict[str, Any], name: str) -> str | None:
    dependencies = manifest.get("dependencies")
    if not isinstance(dependencies, dict):
        return None
    entry = dependencies.get(name)
    if isinstance(entry, str):
        return entry
    if isinstance(entry, dict) and isinstance(entry.get("version"), str):
        return entry["version"]
    return None


def _exact_dependency_version(entry: Any) -> str | None:
    value = entry if isinstance(entry, str) else None
    if isinstance(entry, dict) and isinstance(entry.get("version"), str):
        value = entry["version"]
    if value is None:
        return None
    stripped = value.strip()
    if not stripped.startswith("=") or stripped == "=":
        return None
    return stripped[1:]


def _direct_dependency_entry(manifest: dict[str, Any], name: str) -> tuple[str, Any] | None:
    for section in ("dependencies", "build-dependencies"):
        table = manifest.get(section)
        if isinstance(table, dict) and name in table:
            return (section, table[name])
    return None


def _check_direct_stack_pins(manifest: dict[str, Any], errors: list[str]) -> None:
    for name in GTK_RS_STACK_CRATES:
        direct = _direct_dependency_entry(manifest, name)
        if direct is not None:
            section, entry = direct
            if _exact_dependency_version(entry) is None:
                errors.append(
                    "dependency preflight: "
                    f"Cargo.toml {section}.{name} must be a direct exact pin like '=x.y.z'"
                )
    _check_target_only_stack_dependencies(manifest, errors)


def _check_target_only_stack_dependencies(manifest: dict[str, Any], errors: list[str]) -> None:
    targets = manifest.get("target")
    if not isinstance(targets, dict):
        return
    for target_name, target_table in targets.items():
        if not isinstance(target_table, dict):
            continue
        for section in ("dependencies", "build-dependencies"):
            dependencies = target_table.get(section)
            if not isinstance(dependencies, dict):
                continue
            for name in sorted(set(dependencies) & set(GTK_RS_STACK_CRATES)):
                errors.append(
                    "dependency preflight: "
                    f"Cargo.toml target.{target_name}.{section}.{name} must be a "
                    "top-level direct exact pin, not target-only"
                )


def _check_policy_target_version(
    app_manifest: dict[str, Any],
    app_lock: dict[str, Any],
    policy: dict[str, Any],
    errors: list[str],
) -> None:
    targets = policy.get("targets")
    if not isinstance(targets, dict):
        errors.append("dependency preflight: policy targets.crate must be an object")
        return
    crate = targets.get("crate")
    package = crate.get("cargo_package") if isinstance(crate, dict) else None
    target = crate.get("target_version") if isinstance(crate, dict) else None
    if not isinstance(package, str) or not package or not isinstance(target, str) or not target:
        errors.append("dependency preflight: policy crate cargo_package and target_version are required")
        return
    direct = _direct_dependency_entry(app_manifest, package)
    manifest_version = _exact_dependency_version(direct[1]) if direct is not None else None
    if direct is None:
        errors.append(
            "dependency preflight: "
            f"Cargo.toml {package} must be a direct exact pin matching policy target {target!r}"
        )
    elif manifest_version is None:
        return
    if manifest_version is not None and manifest_version != target:
        errors.append(
            "dependency preflight: "
            f"Cargo.toml {package} version {manifest_version!r} must match policy target {target!r}"
        )
    lock_version = _single_package_version(app_lock, package)
    if lock_version != target:
        errors.append(
            "dependency preflight: "
            f"Cargo.lock {package} version {lock_version!r} must match policy target {target!r}"
        )


def _check_exact_safe_sys_pairs(lock_data: dict[str, Any], label: str, errors: list[str]) -> None:
    packages = _packages_by_name(lock_data)
    for safe_crate, sys_crate in EXACT_SAFE_SYS_PAIRS:
        safe_versions = _package_versions(packages, safe_crate)
        sys_versions = _package_versions(packages, sys_crate)
        if not safe_versions and not sys_versions:
            continue
        if len(safe_versions) != 1 or len(sys_versions) != 1:
            errors.append(
                "dependency preflight: "
                f"{label} must contain exactly one {safe_crate} and one {sys_crate}; "
                f"found {safe_versions!r} and {sys_versions!r}"
            )
            continue
        safe_version = safe_versions[0]
        sys_version = sys_versions[0]
        if safe_version is None or sys_version is None:
            errors.append(f"dependency preflight: {label} {safe_crate}/{sys_crate} must have versions")
        elif safe_version != sys_version:
            errors.append(
                "dependency preflight: "
                f"{label} {safe_crate} {safe_version!r} must exactly match {sys_crate} {sys_version!r}"
            )


def _package_versions(packages: dict[str, list[dict[str, Any]]], name: str) -> list[str | None]:
    versions: list[str | None] = []
    for item in packages.get(name, []):
        version = item.get("version")
        versions.append(version if isinstance(version, str) else None)
    return versions


def _stack_versions(lock_data: dict[str, Any], label: str, errors: list[str]) -> dict[str, str]:
    packages = _packages_by_name(lock_data)
    versions: dict[str, str] = {}
    for name in GTK_RS_STACK_CRATES:
        matches = packages.get(name, [])
        if not matches:
            continue
        if len(matches) != 1:
            errors.append(f"dependency preflight: {label} must contain only one {name} package")
            continue
        version = matches[0].get("version")
        if isinstance(version, str):
            versions[name] = version
        else:
            errors.append(f"dependency preflight: {label} {name} must have a version")
    return versions


def _check_fuzz_stack_sync(
    app_lock: dict[str, Any],
    fuzz_lock: dict[str, Any],
    errors: list[str],
) -> None:
    app_versions = _stack_versions(app_lock, "Cargo.lock", errors)
    fuzz_versions = _stack_versions(fuzz_lock, "fuzz/Cargo.lock", errors)
    for name, fuzz_version in fuzz_versions.items():
        app_version = app_versions.get(name)
        if app_version is None:
            errors.append(
                "dependency preflight: "
                f"fuzz/Cargo.lock contains {name} {fuzz_version!r}, but Cargo.lock does not"
            )
        elif fuzz_version != app_version:
            errors.append(
                "dependency preflight: "
                f"fuzz/Cargo.lock {name} {fuzz_version!r} must match Cargo.lock {name} {app_version!r}"
            )


def _registry_packages(lock_data: dict[str, Any]) -> dict[str, tuple[str, str, str]]:
    expected: dict[str, tuple[str, str, str]] = {}
    for item in lock_data.get("package", []):
        if not isinstance(item, dict):
            continue
        name = item.get("name")
        version = item.get("version")
        source = item.get("source")
        checksum = item.get("checksum")
        if not all(isinstance(value, str) for value in (name, version, source, checksum)):
            continue
        if source in CRATES_IO_SOURCES:
            dest = f"cargo/vendor/{name}-{version}"
            expected[dest] = (name, version, checksum)
    return expected


def _sources_by_dest(
    cargo_sources: list[Any], *, kind: str, filename: str | None = None
) -> dict[str, dict[str, Any]]:
    found: dict[str, dict[str, Any]] = {}
    for item in cargo_sources:
        if not isinstance(item, dict):
            continue
        if item.get("type") != kind or not isinstance(item.get("dest"), str):
            continue
        if filename is not None and item.get("dest-filename") != filename:
            continue
        found[str(item["dest"])] = item
    return found


def _check_duplicate_cargo_sources(cargo_sources: list[Any], errors: list[str]) -> None:
    seen: set[tuple[str, str, str]] = set()
    for item in cargo_sources:
        if not isinstance(item, dict):
            continue
        kind = item.get("type")
        dest = item.get("dest")
        filename = item.get("dest-filename", "")
        if not isinstance(kind, str) or not isinstance(dest, str) or not isinstance(filename, str):
            continue
        key = (kind, dest, filename)
        if key in seen:
            _cargo_sources_error(errors, f"duplicate Flatpak cargo source entry for {key!r}")
        seen.add(key)


def _is_static_crates_io_archive(item: dict[str, Any]) -> bool:
    url = item.get("url")
    dest = item.get("dest")
    return (
        item.get("type") == "archive"
        and item.get("archive-type") == "tar-gzip"
        and isinstance(url, str)
        and url.startswith(f"{STATIC_CRATE_BASE}/")
        and isinstance(dest, str)
        and dest.startswith("cargo/vendor/")
    )


def _check_cargo_sources(app_lock: dict[str, Any], cargo_sources: list[Any], errors: list[str], contract: dict[str, Any]) -> None:
    _check_duplicate_cargo_sources(cargo_sources, errors)
    expected = _registry_packages(app_lock)
    check_source_inventory(cargo_sources, set(expected), contract, errors)
    archives = {
        dest: item
        for dest, item in _sources_by_dest(cargo_sources, kind="archive").items()
        if _is_static_crates_io_archive(item)
    }
    checksums = _sources_by_dest(cargo_sources, kind="inline", filename=".cargo-checksum.json")
    for dest, (name, version, checksum) in expected.items():
        archive = archives.get(dest)
        if archive is None:
            _cargo_sources_error(errors, f"missing Flatpak cargo archive source for {name} {version}")
        else:
            _check_archive_source(dest, name, version, checksum, archive, errors)
        inline = checksums.get(dest)
        if inline is None:
            _cargo_sources_error(errors, f"missing Flatpak cargo checksum source for {name} {version}")
        else:
            _check_checksum_source(dest, name, version, checksum, inline, errors)

    for stale in sorted(set(archives) - set(expected)):
        _cargo_sources_error(errors, f"stale Flatpak cargo archive source {stale}")
    static_checksum_dests = {dest for dest in checksums if dest in archives or dest in expected}
    for stale in sorted(static_checksum_dests - set(expected)):
        _cargo_sources_error(errors, f"stale Flatpak cargo checksum source {stale}")


def _cargo_sources_error(errors: list[str], message: str) -> None:
    errors.append(f"dependency preflight: {message}. {CARGO_SOURCES_REMEDIATION}")


def _check_archive_source(
    dest: str,
    name: str,
    version: str,
    checksum: str,
    archive: dict[str, Any],
    errors: list[str],
) -> None:
    expected_url = f"{STATIC_CRATE_BASE}/{name}/{name}-{version}.crate"
    expected = {
        "archive-type": "tar-gzip",
        "url": expected_url,
        "sha256": checksum,
    }
    for key, value in expected.items():
        if archive.get(key) != value:
            _cargo_sources_error(errors, f"{dest} {key} {archive.get(key)!r} must match locked value {value!r}")


def _check_checksum_source(
    dest: str,
    name: str,
    version: str,
    checksum: str,
    inline: dict[str, Any],
    errors: list[str],
) -> None:
    contents = inline.get("contents")
    if not isinstance(contents, str):
        _cargo_sources_error(
            errors, f"{dest} checksum source for {name} {version} must contain JSON"
        )
        return
    try:
        payload = json.loads(contents)
    except json.JSONDecodeError as exc:
        _cargo_sources_error(
            errors, f"{dest} checksum source for {name} {version} has invalid JSON: {exc}"
        )
        return
    if not isinstance(payload, dict):
        _cargo_sources_error(errors, f"{dest} checksum payload for {name} {version} must be a JSON object")
        return
    if payload.get("package") != checksum:
        _cargo_sources_error(errors, f"{dest} checksum payload must match locked checksum for {name} {version}")
    if not isinstance(payload.get("files"), dict):
        _cargo_sources_error(errors, f"{dest} checksum payload files field for {name} {version} must be an object")


def main() -> int:
    parser = argparse.ArgumentParser(description="Fast dependency preflight for Riteed CI.")
    parser.add_argument("--root", help="Application root. Defaults to auto-detection.")
    args = parser.parse_args()
    root = repo_root(args.root)
    errors: list[str] = []
    check_dependency_preflight(root, errors)
    for item in errors:
        print(f"[dependency-preflight] {item}")
    if errors:
        return 1
    print("[dependency-preflight] OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
