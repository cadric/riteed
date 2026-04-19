from __future__ import annotations

import re
from pathlib import Path

from tools.scanners.sites import ScanHit
from tools.validation_tooling import normalize_path, read_text, scoped_files

SURFACE_RE = re.compile(r"\b(?:Adw|Gtk)\.(?:ApplicationWindow|Window|Dialog|PreferencesWindow|NavigationSplitView|NavigationView)\b")
MENU_RE = re.compile(r"\bmenu\b", re.IGNORECASE)
BUTTON_START_RE = re.compile(r"\b(?:Gtk|Adw)\.(?:Button|MenuButton|SplitButton|ToggleButton)\b")


def blueprint_surface_hits(root: Path) -> list[ScanHit]:
    hits: list[ScanHit] = []
    for path in scoped_files(root, ["data/**/*.blp"]):
        for index, line in enumerate(read_text(path).splitlines(), start=1):
            if SURFACE_RE.search(line):
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="surface",
                        match=line.strip(),
                        message="Blueprint surfaces require adaptive review coverage.",
                    )
                )
            if MENU_RE.search(line) and "primary" in line.lower():
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="menu",
                        match=line.strip(),
                        message="Blueprint menus require review coverage.",
                    )
                )
    return hits


def icon_only_buttons(root: Path) -> list[str]:
    findings: list[str] = []
    for path in scoped_files(root, ["data/**/*.blp"]):
        lines = read_text(path).splitlines()
        stack: list[dict[str, object]] = []
        for index, line in enumerate(lines, start=1):
            if BUTTON_START_RE.search(line):
                stack.append({"line": index, "depth": line.count("{") - line.count("}"), "text": [line]})
                continue
            for block in stack:
                block["text"].append(line)
                block["depth"] += line.count("{") - line.count("}")
            while stack and stack[-1]["depth"] <= 0:
                block = stack.pop()
                text = "\n".join(block["text"])
                if "icon-name:" in text and all(token not in text for token in ("label:", "accessible-name:", "tooltip-text:")):
                    findings.append(
                        f"{path.relative_to(root).as_posix()}:{block['line']}: icon-only interactive element lacks accessible naming"
                    )
    return findings
