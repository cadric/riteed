from __future__ import annotations

import argparse
import os
import shlex
import tempfile
import xml.etree.ElementTree as ET
from pathlib import Path

from tools.checks.foundation import (
    add,
    find_flatpak_manifest,
    looks_like_target_repo,
    update_artifact_index,
    validation_policy,
)
from tools.checks.i18n import normalized_pot_messages
from tools.scanners.pot import message_keys
from tools.scanners.rust import rust_files
from tools.validation_tooling import relpath, read_text, require_tool, run_checked, scoped_files


def _xgettext_messages(root: Path) -> set[tuple[str | None, str, str | None]]:
    files_by_language = [
        (
            "Rust",
            rust_files(root),
            [
                "--keyword=gettext",
                "--keyword=ngettext:1,2",
                "--keyword=pgettext:1c,2",
                "--keyword=npgettext:1c,2,3",
                "--keyword=_",
            ],
        ),
        (
            "Desktop",
            scoped_files(root, ["data/*.desktop.in.in", "data/*.desktop.in", "data/*.desktop"]),
            [],
        ),
        (
            "GSettings",
            scoped_files(root, ["data/schemas/**/*.gschema.xml"]),
            [],
        ),
    ]
    generated: set[tuple[str | None, str, str | None]] = set()
    for language, paths, keywords in files_by_language:
        if not paths:
            continue
        with tempfile.TemporaryDirectory(prefix=f"xgettext-{language.lower()}-") as tmpdir:
            out = Path(tmpdir) / f"{language.lower()}.pot"
            cmd = [
                "xgettext",
                "--language",
                language,
                "--from-code=UTF-8",
                "--sort-by-file",
                "--omit-header",
                "--no-location",
                "--output",
                str(out),
                *keywords,
                *[str(path) for path in sorted(paths)],
            ]
            run_checked(cmd, root, f"xgettext {language}")
            generated |= message_keys(out)
    return generated


def _metainfo_messages(root: Path) -> set[tuple[str | None, str, str | None]]:
    messages: set[tuple[str | None, str, str | None]] = set()
    for path in scoped_files(root, ["data/*.metainfo.xml.in.in", "data/*.metainfo.xml.in", "data/*.metainfo.xml"]):
        try:
            tree = ET.fromstring(read_text(path))
        except ET.ParseError:
            continue
        for node in tree.iter():
            tag = node.tag.rsplit("}", 1)[-1]
            if tag not in {"name", "summary", "p", "li", "caption"}:
                continue
            if node.attrib.get("translate") == "no":
                continue
            text = (node.text or "").strip()
            if text:
                messages.add((None, text, None))
    return messages


def check_xgettext_completeness(root: Path, errors: list[str]) -> None:
    generated = _xgettext_messages(root) | _metainfo_messages(root)
    if not generated:
        return
    existing = normalized_pot_messages(root)
    sort_key = lambda item: (item[0] or "", item[1], item[2] or "")
    missing = sorted(generated - existing, key=sort_key)
    extra = sorted(existing - generated, key=sort_key)
    if missing:
        preview = ", ".join(repr(item[1]) for item in missing[:5])
        add(errors, f"Checked-in POT is missing extracted messages: {preview}")
    if extra:
        preview = ", ".join(repr(item[1]) for item in extra[:5])
        add(errors, f"Checked-in POT contains messages not produced by current extraction inputs: {preview}")


def _headless_gtk_env() -> dict[str, str]:
    return {
        "GSK_RENDERER": os.environ.get("GSK_RENDERER", "cairo"),
        "GTK_A11Y": os.environ.get("GTK_A11Y", "none"),
    }


def run_required_commands(root: Path, errors: list[str]) -> None:
    cfg = validation_policy(root)
    for tool in cfg["required_tools"]:
        require_tool(tool)
    for command in cfg["required_commands"]:
        run_checked(shlex.split(command), root, command, env=_headless_gtk_env())

    check_xgettext_completeness(root, errors)

    ran_flatpak_manifest = False
    for item in cfg.get("conditional_validators", []):
        paths = scoped_files(root, [item["when_glob"]])
        if not paths:
            continue
        require_tool(item["tool"])
        mode = item["mode"]
        if mode == "gsettings":
            run_checked(["glib-compile-schemas", "--strict", "--dry-run", "data/schemas"], root, "glib-compile-schemas")
        elif mode == "po":
            for path in paths:
                run_checked(["msgfmt", "--check-format", "--check-header", "-o", "/dev/null", str(path)], root, f"msgfmt {relpath(path, root)}")
        elif mode == "desktop":
            for path in paths:
                run_checked(["desktop-file-validate", str(path)], root, f"desktop-file-validate {relpath(path, root)}")
        elif mode == "metainfo":
            for path in paths:
                run_checked(["appstreamcli", "validate", "--no-net", "--pedantic", str(path)], root, f"appstreamcli {relpath(path, root)}")
        elif mode == "flatpak-manifest" and not ran_flatpak_manifest:
            manifest = find_flatpak_manifest(root)
            if manifest is not None:
                run_checked(["flatpak-builder", "--show-manifest", str(manifest)], root, "flatpak-builder --show-manifest")
                ran_flatpak_manifest = True


def run_update_artifact_index(root: Path, args: argparse.Namespace) -> int:
    if args.root:
        print("[policy-check] --update-artifact-index must not be used with --root", flush=True)
        return 1
    if looks_like_target_repo(root):
        print("[policy-check] --update-artifact-index is maintainer-only for the policy-pack repo", flush=True)
        return 1
    update_artifact_index(root)
    print("[policy-check] Updated artifact_index")
    return 0
