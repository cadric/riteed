from __future__ import annotations

import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

from tools.scanners.ui_xml import translatable_property_errors
from tools.validation_tooling import (
    cargo_packages,
    contract_root,
    count_lines,
    dump_json,
    file_hash,
    first_file,
    grep_any,
    grep_lines,
    load_json,
    load_toml,
    match_any,
    read_text,
    relpath,
    scoped_files,
)


def add(errors: list[str], message: str) -> None:
    errors.append(message)


def _policy_root(root: Path) -> Path:
    return contract_root(root)


def policy_bundle(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "gnome-rust-app.bundle.json")


def validation_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "validation-tooling.policy.json")


def rust_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "rust.policy.json")


def flatpak_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "flatpak-metadata.policy.json")


def hig_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "hig.policy.json")


def libadwaita_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "libadwaita.policy.json")


def gettext_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "gettext-i18n.policy.json")


def gsettings_policy(root: Path) -> dict[str, Any]:
    return load_json(_policy_root(root) / "policy" / "gsettings.policy.json")


def _safe_load_toml(path: Path, errors: list[str], label: str) -> dict[str, Any] | None:
    if not path.exists():
        add(errors, f"Missing required TOML file: {label}")
        return None
    try:
        return load_toml(path)
    except SystemExit:
        add(errors, f"Invalid TOML file: {label}")
        return None


def _search_text(paths: list[Path], pattern: str) -> bool:
    regex = re.compile(pattern, re.MULTILINE | re.DOTALL)
    return any(regex.search(read_text(path)) for path in paths)


def _policy_index(root: Path, bundle: dict[str, Any]) -> list[dict[str, Any]]:
    policy_root = _policy_root(root)
    rows: list[dict[str, Any]] = []
    for item in bundle.get("bundle_contains", []):
        rel = str(item.get("file", "")).strip()
        path = policy_root / "policy" / rel
        digest, size = file_hash(path)
        rows.append({"file": rel, "sha256": digest, "bytes": size})
    return rows


def update_artifact_index(root: Path) -> None:
    policy_root = _policy_root(root)
    bundle_path = policy_root / "policy" / "gnome-rust-app.bundle.json"
    bundle = policy_bundle(root)
    bundle["artifact_index"] = _policy_index(root, bundle)
    bundle_path.write_text(dump_json(bundle), encoding="utf-8")


def check_policy_stack(root: Path, errors: list[str]) -> None:
    policy_root = _policy_root(root)
    required = validation_policy(root).get("required_policy_files", [])
    for rel in required:
        if not (policy_root / rel).exists():
            add(errors, f"Missing required policy file: {rel}")
    bundle = policy_bundle(root)
    bundle_ids: list[str] = []
    for item in bundle.get("bundle_contains", []):
        path = policy_root / "policy" / str(item.get("file", "")).strip()
        if not path.exists():
            add(errors, f"Bundle references missing policy file: {path.relative_to(policy_root)}")
            continue
        actual = load_json(path)
        expected_id = item.get("$id")
        if expected_id and actual.get("$id") != expected_id:
            add(errors, f"{path.relative_to(policy_root)} has $id {actual.get('$id')!r}, expected {expected_id!r}")
        if expected_id:
            bundle_ids.append(str(expected_id))
    overlap_ids = [str(item) for item in bundle.get("overlaps_with", [])]
    if sorted(overlap_ids) != sorted(bundle_ids):
        add(errors, "Bundle overlaps_with must stay synchronized with bundle_contains $id values")

    artifact_index = bundle.get("artifact_index", [])
    expected_index = _policy_index(root, bundle)
    if artifact_index != expected_index:
        add(
            errors,
            "Bundle artifact_index is out of date. Run `python3 -m tools.policy_check --update-artifact-index` in the policy-pack repo.",
        )


def check_repo_layout(root: Path, errors: list[str]) -> None:
    layout = policy_bundle(root).get("required_repository_layout", {})
    for rel in layout.get("must_exist", []):
        if not (root / rel).exists():
            add(errors, f"Missing required path from bundle: {rel}")
    policy_root = _policy_root(root)
    for rel in layout.get("contract_must_exist", []):
        if not (policy_root / rel).exists():
            add(errors, f"Missing required contract path from bundle: {rel}")


def check_toolchain(root: Path, errors: list[str]) -> None:
    toolchain = _safe_load_toml(root / "rust-toolchain.toml", errors, "rust-toolchain.toml")
    if toolchain is None:
        return
    cfg = toolchain.get("toolchain", {}) if isinstance(toolchain.get("toolchain"), dict) else {}
    channel = str(cfg.get("channel", "")).strip()
    if not channel.startswith("1.95"):
        add(errors, f"rust-toolchain.toml must pin Rust 1.95.x, found {channel!r}")
    components = {str(item) for item in cfg.get("components", [])}
    for component in ("rustfmt", "clippy"):
        if component not in components:
            add(errors, f"rust-toolchain.toml must include component {component!r}")


def _lint_value(table: dict[str, Any], key: str) -> str | None:
    value = table.get(key)
    if value is None:
        return None
    if isinstance(value, dict) and "level" in value:
        return str(value["level"])
    return str(value)


def _check_lint_table(root: Path, lints: dict[str, Any], label: str, errors: list[str]) -> None:
    required_rust = rust_policy(root)["lint_baseline"]["rust_lints_required"]
    required_clippy = rust_policy(root)["lint_baseline"]["clippy_lints_required"]
    rust_lints = lints.get("rust", {}) if isinstance(lints.get("rust"), dict) else {}
    clippy_lints = lints.get("clippy", {}) if isinstance(lints.get("clippy"), dict) else {}
    for key, expected in required_rust.items():
        actual = _lint_value(rust_lints, key)
        if actual != expected:
            add(errors, f"{label}: rust lint {key!r} must be {expected!r}, found {actual!r}")
    for key, expected in required_clippy.items():
        actual = _lint_value(clippy_lints, key)
        if actual != expected:
            add(errors, f"{label}: clippy lint {key!r} must be {expected!r}, found {actual!r}")


def _gettext_system_feature_required(root: Path) -> bool:
    linking = gettext_policy(root).get("linking_and_distribution", {})
    return bool(linking.get("gettext_system_feature_required_on_linux_and_flatpak_targets"))


def _check_gettext_system_feature(root_deps: list[dict[str, Any]], errors: list[str]) -> None:
    gettext_deps = [
        dep
        for dep in root_deps
        if dep.get("name") == "gettext-rs" and dep.get("kind") is None
    ]
    if not gettext_deps:
        return
    if any("gettext-system" in dep.get("features", []) for dep in gettext_deps):
        return
    add(errors, "gettext-rs must enable feature 'gettext-system' because system gettext is required by policy")


def check_manifests(root: Path, errors: list[str]) -> None:
    cargo_toml = _safe_load_toml(root / "Cargo.toml", errors, "Cargo.toml")
    if cargo_toml is None:
        return
    workspace_lints: dict[str, Any] = {}
    workspace = cargo_toml.get("workspace", {})
    if isinstance(workspace, dict):
        workspace_lints = workspace.get("lints", {}) if isinstance(workspace.get("lints"), dict) else {}
    if not workspace_lints and not cargo_toml.get("lints"):
        add(errors, "Cargo.toml must define [workspace.lints] or [lints]")
    if workspace_lints:
        _check_lint_table(root, workspace_lints, "Cargo.toml [workspace.lints]", errors)

    try:
        packages = cargo_packages(root)
    except SystemExit:
        add(errors, "cargo metadata failed; Cargo.toml and workspace manifests must be valid")
        return

    root_deps: set[str] = set()
    root_dependency_entries: list[dict[str, Any]] = []
    root_manifest = (root / "Cargo.toml").resolve()
    for pkg in packages:
        edition = str(pkg.get("edition", ""))
        if edition != "2024":
            add(errors, f"Package {pkg.get('name')} must use edition 2024, found {edition!r}")
        manifest_path = Path(str(pkg["manifest_path"])).resolve()
        if manifest_path == root_manifest:
            for dep in pkg.get("dependencies", []):
                if isinstance(dep, dict) and dep.get("name"):
                    root_deps.add(str(dep["name"]))
                    root_dependency_entries.append(dep)
        data = _safe_load_toml(manifest_path, errors, relpath(manifest_path, root))
        if data is None:
            continue
        label = relpath(manifest_path, root)
        lints = data.get("lints", {})
        if isinstance(lints, dict) and lints.get("workspace") is True:
            if not workspace_lints:
                add(errors, f"{label}: uses lints.workspace=true but root has no [workspace.lints]")
        elif isinstance(lints, dict) and (lints.get("rust") or lints.get("clippy")):
            _check_lint_table(root, lints, label, errors)
        else:
            add(errors, f"{label}: package must define [lints] or set lints.workspace = true")

    dep_policy = validation_policy(root)["dependency_policy"]
    for required in dep_policy["required_runtime_crates"]:
        if required not in root_deps:
            add(errors, f"Missing required runtime crate dependency in root application package: {required}")
    for forbidden in dep_policy["forbidden_crates"]:
        if forbidden in root_deps:
            add(errors, f"Forbidden crate dependency present in root application package: {forbidden}")
    if _gettext_system_feature_required(root):
        _check_gettext_system_feature(root_dependency_entries, errors)


def check_crate_roots(root: Path, errors: list[str]) -> None:
    patterns = ["src/main.rs", "src/lib.rs", "crates/**/src/main.rs", "crates/**/src/lib.rs"]
    roots = scoped_files(root, patterns)
    if not roots:
        add(errors, "At least one Rust crate root is required")
        return
    required = validation_policy(root)["required_source_patterns"][:2]
    for path in roots:
        for item in required:
            if not grep_any(root, [path], item["pattern"]):
                add(errors, f"{relpath(path, root)}: {item['message']}")


def check_forbidden_patterns(root: Path, errors: list[str]) -> None:
    rules = validation_policy(root)["forbidden_source_patterns"]
    for rule in rules:
        paths = scoped_files(root, rule["paths"])
        if not paths:
            continue
        exceptions = rule.get("exceptions", [])
        filtered = [path for path in paths if not match_any(relpath(path, root), exceptions)]
        hits = grep_lines(root, filtered, rule["pattern"])
        if hits:
            add(errors, f"{rule['message']} :: {'; '.join(hits[:5])}")


def check_required_patterns(root: Path, errors: list[str]) -> None:
    rules = validation_policy(root)["required_source_patterns"][2:]
    for rule in rules:
        paths = scoped_files(root, rule["paths"])
        if not paths or not grep_any(root, paths, rule["pattern"]):
            add(errors, rule["message"])


def check_line_limits(root: Path, errors: list[str]) -> None:
    limit = int(validation_policy(root)["thresholds"]["max_file_lines"])
    for path in scoped_files(root, validation_policy(root)["line_limit_globs"]):
        lines = count_lines(path)
        if lines > limit:
            add(errors, f"{relpath(path, root)} exceeds hard LOC limit {limit}: {lines}")


def _walk_json(value: Any) -> list[dict[str, Any]]:
    found: list[dict[str, Any]] = []
    if isinstance(value, dict):
        found.append(value)
        for nested in value.values():
            found.extend(_walk_json(nested))
    elif isinstance(value, list):
        for nested in value:
            found.extend(_walk_json(nested))
    return found


def _parse_flatpak_yaml(text: str) -> dict[str, Any]:
    data: dict[str, Any] = {"finish_args": [], "git_sources": 0, "url_sources": 0}
    lines = text.splitlines()
    in_finish_args = False
    finish_indent = None
    for line in lines:
        if re.match(r"^\s*app-id\s*:", line):
            data["deprecated_app_id"] = True
        for key in ("id", "runtime", "runtime-version", "sdk", "command"):
            match = re.match(rf"^\s*{re.escape(key)}\s*:\s*(.+?)\s*$", line)
            if match and key not in data:
                data[key] = match.group(1).strip().strip("'\"")
        current_indent = len(line) - len(line.lstrip(" "))
        if re.match(r"^\s*finish-args\s*:\s*$", line):
            in_finish_args = True
            finish_indent = current_indent
            continue
        if in_finish_args:
            if line.strip() and current_indent <= (finish_indent or 0):
                in_finish_args = False
            elif re.match(r"^\s*-\s+", line):
                data["finish_args"].append(line.split("-", 1)[1].strip().strip("'\""))
        if re.match(r"^\s*type\s*:\s*git\s*$", line):
            data["git_sources"] += 1
        if re.match(r"^\s*commit\s*:\s*\S+", line):
            data.setdefault("commits", 0)
            data["commits"] += 1
        if re.match(r"^\s*url\s*:\s*\S+", line):
            data["url_sources"] += 1
        if re.match(r"^\s*sha(256|512)\s*:\s*\S+", line):
            data.setdefault("hashes", 0)
            data["hashes"] += 1
    data["unpinned_git_sources"] = max(0, data["git_sources"] - int(data.get("commits", 0)))
    data["unhashed_url_sources"] = max(0, data["url_sources"] - int(data.get("hashes", 0)))
    return data


def parse_flatpak_manifest(path: Path) -> dict[str, Any]:
    text = read_text(path)
    if path.suffix == ".json":
        manifest = json.loads(text)
        git_sources = 0
        commits = 0
        url_sources = 0
        hashes = 0
        for node in _walk_json(manifest):
            source_type = str(node.get("type", "")).strip()
            if source_type == "git":
                git_sources += 1
                if node.get("commit"):
                    commits += 1
            if node.get("url"):
                url_sources += 1
                if node.get("sha256") or node.get("sha512"):
                    hashes += 1
        finish_args = [str(item) for item in manifest.get("finish-args", []) if isinstance(item, str)]
        return {
            "id": manifest.get("id"),
            "runtime": manifest.get("runtime"),
            "runtime-version": manifest.get("runtime-version"),
            "sdk": manifest.get("sdk"),
            "command": manifest.get("command"),
            "finish_args": finish_args,
            "deprecated_app_id": "app-id" in manifest,
            "unpinned_git_sources": max(0, git_sources - commits),
            "unhashed_url_sources": max(0, url_sources - hashes),
        }
    return _parse_flatpak_yaml(text)


def _desktop_value(text: str, key: str) -> str | None:
    for prefix in (key, f"_{key}"):
        match = re.search(rf"(?m)^{re.escape(prefix)}=(.+)$", text)
        if match:
            return match.group(1).strip()
    return None


def _manifest_basename(path: Path) -> str:
    for suffix in (".yml", ".yaml", ".json"):
        if path.name.endswith(suffix):
            return path.name[: -len(suffix)]
    return path.stem


def find_flatpak_manifest(root: Path) -> Path | None:
    for path in scoped_files(root, ["build-aux/*.yml", "build-aux/*.yaml", "build-aux/*.json"]):
        try:
            data = parse_flatpak_manifest(path)
        except json.JSONDecodeError:
            continue
        has_id = bool(str(data.get("id") or "").strip() or data.get("deprecated_app_id"))
        looks_like_manifest = any(str(data.get(key) or "").strip() for key in ("runtime", "runtime-version", "sdk", "command")) or bool(data.get("finish_args"))
        if has_id and looks_like_manifest:
            return path
    return None


def _load_permission_justifications(root: Path, errors: list[str], required: bool) -> dict[str, str]:
    rel = flatpak_policy(root)["sandbox_permissions"].get(
        "permission_justification_file",
        "build-aux/permissions/flatpak-permissions.justifications.json",
    )
    path = root / rel
    if not path.exists():
        if required:
            add(errors, f"{rel} is required when reviewed Flatpak permissions are present")
        return {}
    try:
        raw = json.loads(read_text(path))
    except json.JSONDecodeError as exc:
        add(errors, f"{rel}: invalid JSON ({exc})")
        return {}
    payload = raw.get("finish_args", raw) if isinstance(raw, dict) else None
    if not isinstance(payload, dict):
        add(errors, f"{rel}: expected an object or a top-level 'finish_args' object")
        return {}
    normalized: dict[str, str] = {}
    for key, value in payload.items():
        if not isinstance(key, str) or not isinstance(value, str) or not value.strip():
            add(errors, f"{rel}: justification entries must map finish-arg strings to non-empty text")
            continue
        normalized[key.strip()] = value.strip()
    return normalized


def _needs_justification(arg: str, exact: set[str], prefixes: list[str]) -> bool:
    return arg in exact or any(arg.startswith(prefix) for prefix in prefixes)


def check_flatpak_and_identity(root: Path, errors: list[str]) -> str | None:
    manifest = find_flatpak_manifest(root)
    if manifest is None:
        add(errors, "Flatpak manifest is required under build-aux/")
        return None
    try:
        data = parse_flatpak_manifest(manifest)
    except json.JSONDecodeError as exc:
        add(errors, f"{relpath(manifest, root)}: invalid JSON manifest ({exc})")
        return None

    app_id = str(data.get("id") or "").strip()
    if not app_id:
        add(errors, f"{relpath(manifest, root)}: manifest must define id")
        return None
    if data.get("deprecated_app_id"):
        add(errors, f"{relpath(manifest, root)}: deprecated app-id key is forbidden")

    expected = {"runtime": "org.gnome.Platform", "runtime-version": "50", "sdk": "org.gnome.Sdk"}
    for key, value in expected.items():
        actual = str(data.get(key) or "").strip()
        if actual != value:
            add(errors, f"{relpath(manifest, root)}: {key} must be {value!r}, found {actual!r}")
    if not str(data.get("command") or "").strip():
        add(errors, f"{relpath(manifest, root)}: command is required")
    if _manifest_basename(manifest) != app_id:
        add(errors, f"{relpath(manifest, root)}: manifest filename must equal application id {app_id}")

    sandbox = flatpak_policy(root)["sandbox_permissions"]
    forbidden_args = set(sandbox.get("forbidden_by_default", []))
    exact = set(sandbox.get("requires_written_justification", []))
    prefixes = list(sandbox.get("requires_written_justification_prefixes", []))
    finish_args = [str(arg).strip() for arg in data.get("finish_args", []) if str(arg).strip()]
    needed = [arg for arg in finish_args if _needs_justification(arg, exact, prefixes)]
    justifications = _load_permission_justifications(root, errors, required=bool(needed)) if needed else {}
    for arg in finish_args:
        if arg in forbidden_args:
            add(errors, f"{relpath(manifest, root)}: forbidden Flatpak finish-arg {arg}")
            continue
        if _needs_justification(arg, exact, prefixes) and arg not in justifications:
            rel = sandbox.get("permission_justification_file", "build-aux/permissions/flatpak-permissions.justifications.json")
            add(errors, f"{relpath(manifest, root)}: finish-arg {arg} requires written justification in {rel}")
    if "--socket=fallback-x11" in finish_args and "--socket=wayland" not in finish_args:
        add(errors, f"{relpath(manifest, root)}: --socket=fallback-x11 requires --socket=wayland for native GTK4 apps")
    if data.get("unpinned_git_sources"):
        add(errors, f"{relpath(manifest, root)}: git sources must be pinned to commits")
    if data.get("unhashed_url_sources"):
        add(errors, f"{relpath(manifest, root)}: URL sources must be checksum pinned")

    desktop = first_file(root, ["data/*.desktop.in.in", "data/*.desktop.in", "data/*.desktop"])
    desktop_name: str | None = None
    if desktop is None:
        add(errors, "Desktop file is required under data/")
    else:
        stem = desktop.name.replace(".desktop.in.in", "").replace(".desktop.in", "").replace(".desktop", "")
        if stem != app_id:
            add(errors, f"{relpath(desktop, root)}: desktop basename must equal application id {app_id}")
        text = read_text(desktop)
        icon = _desktop_value(text, "Icon")
        if icon and icon not in {app_id, "@icon@", "@appid@", "${application_id}"}:
            add(errors, f"{relpath(desktop, root)}: Icon must equal application id {app_id} or a reviewed template placeholder")
        desktop_name = _desktop_value(text, "Name")

    metainfo = first_file(root, ["data/*.metainfo.xml.in.in", "data/*.metainfo.xml.in", "data/*.metainfo.xml"])
    if metainfo is None:
        add(errors, "Metainfo file is required under data/")
    else:
        try:
            meta = ET.fromstring(read_text(metainfo))
        except ET.ParseError as exc:
            add(errors, f"{relpath(metainfo, root)}: invalid XML ({exc})")
            meta = None
        if meta is not None:
            meta_id = (meta.findtext(".//id") or "").strip()
            if meta_id not in {app_id, "@appid@", "${application_id}"}:
                add(errors, f"{relpath(metainfo, root)}: component id must equal application id {app_id} or a reviewed template placeholder")
            launchable = meta.find(".//launchable")
            launchable_text = (launchable.text or "").strip() if launchable is not None else ""
            if launchable is None or launchable_text not in {f"{app_id}.desktop", "@appid@.desktop", "${application_id}.desktop"}:
                add(errors, f"{relpath(metainfo, root)}: launchable desktop-id must equal {app_id}.desktop or a reviewed template placeholder")
            meta_name = (meta.findtext(".//name") or "").strip()
            if desktop is not None and desktop_name and meta_name and desktop_name != meta_name:
                add(errors, f"{relpath(desktop, root)} and {relpath(metainfo, root)} must use the same app name")

    icon_files = [
        path
        for path in scoped_files(root, ["data/icons/**/*.svg", "data/icons/**/*.png"])
        if path.name in {f"{app_id}.svg", f"{app_id}.png", f"{app_id}.symbolic.svg"}
    ]
    if not icon_files:
        add(errors, f"Icon basename must include application id {app_id} under data/icons/")
    return app_id


def check_resources(root: Path, app_id: str | None, errors: list[str]) -> None:
    path = root / "data" / "resources.gresource.xml"
    assets = scoped_files(root, ["data/ui/**/*.ui", "data/ui/**/*.blp", "data/style/**/*.css"])
    if not assets and not path.exists():
        return
    if not path.exists():
        add(errors, "data/resources.gresource.xml is required when packaged UI or CSS assets exist")
        return
    try:
        xml = ET.fromstring(read_text(path))
    except ET.ParseError as exc:
        add(errors, f"{relpath(path, root)}: invalid XML ({exc})")
        return
    prefixes = [node.get("prefix", "") for node in xml.findall(".//gresource")]
    if app_id:
        resource_prefix = "/" + app_id.replace(".", "/")
        if not any(prefix.startswith(resource_prefix) for prefix in prefixes):
            add(errors, f"{relpath(path, root)}: gresource prefix must start with {resource_prefix}")
    manifest_files = {(node.text or "").strip() for node in xml.findall(".//file") if (node.text or "").strip()}
    for asset in assets:
        rel = asset.relative_to(path.parent).as_posix()
        if rel not in manifest_files:
            add(errors, f"{relpath(path, root)} must include resource entry for {rel}")


def check_ui_localization(root: Path, errors: list[str]) -> None:
    errors.extend(translatable_property_errors(root))
    for path in scoped_files(root, ["data/**/*.gschema.xml"]):
        try:
            tree = ET.fromstring(read_text(path))
        except ET.ParseError as exc:
            add(errors, f"{relpath(path, root)}: invalid XML ({exc})")
            continue
        for schema in tree.findall(".//schema"):
            if not schema.get("id"):
                add(errors, f"{relpath(path, root)}: schema id is required")
            if not schema.get("path"):
                add(errors, f"{relpath(path, root)}: schema path is required for non-relocatable app preferences")
            for key in schema.findall("./key"):
                if not (key.get("type") or key.get("enum") or key.get("flags")):
                    add(errors, f"{relpath(path, root)}: key {key.get('name')} must declare type, enum, or flags")
                for child in ("default", "summary", "description"):
                    value = (key.findtext(child) or "").strip()
                    if not value:
                        add(errors, f"{relpath(path, root)}: key {key.get('name')} must define {child}")


def looks_like_target_repo(root: Path) -> bool:
    return (root / "Cargo.toml").exists() and (root / "src").is_dir() and (root / "data").is_dir()


def gettext_bootstrap_present(root: Path) -> bool:
    source = scoped_files(root, ["src/**/*.rs", "crates/**/*.rs"])
    bootstrap_ok = grep_any(root, source, r"\b(bindtextdomain|textdomain|bind_textdomain_codeset|setlocale)\b")
    if bootstrap_ok:
        return True
    return _search_text(source, r"TextDomain::new\s*\(.*?\)(?:\s*\.\w+\s*\(.*?\))*\s*\.init\s*\(")
