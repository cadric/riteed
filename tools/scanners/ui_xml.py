from __future__ import annotations

import re
from pathlib import Path

from tools.scanners.sites import ScanHit
from tools.validation_tooling import normalize_path, read_text, scoped_files

SURFACE_RE = re.compile(
    r'<(?:template|object)\b[^>]*(?:class="(?P<class>[^"]*(?:ApplicationWindow|Window|Dialog|PreferencesWindow|NavigationSplitView|NavigationView)[^"]*)")'
)
MENU_RE = re.compile(r"<menu\b")
BUTTON_RE = re.compile(r'<object\b[^>]*class="(?P<class>[^"]*(?:Button|MenuButton|SplitButton|ToggleButton)[^"]*)"')


def ui_surface_hits(root: Path) -> list[ScanHit]:
    hits: list[ScanHit] = []
    for path in scoped_files(root, ["data/**/*.ui"]):
        for index, line in enumerate(read_text(path).splitlines(), start=1):
            if SURFACE_RE.search(line):
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="surface",
                        match=line.strip(),
                        message="UI surfaces require adaptive review coverage.",
                    )
                )
            if MENU_RE.search(line):
                hits.append(
                    ScanHit(
                        path=normalize_path(path.relative_to(root).as_posix()),
                        line=index,
                        kind="menu",
                        match=line.strip(),
                        message="Menus require review coverage.",
                    )
                )
    return hits


def icon_only_buttons(root: Path) -> list[str]:
    findings: list[str] = []
    for path in scoped_files(root, ["data/**/*.ui"]):
        lines = read_text(path).splitlines()
        stack: list[dict[str, object]] = []
        for index, line in enumerate(lines, start=1):
            start = BUTTON_RE.search(line)
            if start:
                stack.append({"line": index, "text": [line]})
            for block in stack[:-1] if start else stack:
                block["text"].append(line)
            if start:
                continue
            if "</object>" in line and stack:
                block = stack.pop()
                text = "\n".join(block["text"])
                if "icon-name" in text and all(token not in text for token in ("label", "accessible-name", "tooltip-text")):
                    findings.append(
                        f"{path.relative_to(root).as_posix()}:{block['line']}: icon-only interactive element lacks accessible naming"
                    )
    return findings


def translatable_property_errors(root: Path) -> list[str]:
    findings: list[str] = []
    interesting = {"label", "title", "subtitle", "tooltip-text", "placeholder-text", "text"}
    prop_re = re.compile(r'<property\b[^>]*name="(?P<name>[^"]+)"(?P<attrs>[^>]*)>(?P<value>.*?)</property>')
    for path in scoped_files(root, ["data/**/*.ui"]):
        for index, line in enumerate(read_text(path).splitlines(), start=1):
            match = prop_re.search(line)
            if not match:
                continue
            name = match.group("name")
            value = match.group("value").strip()
            attrs = match.group("attrs")
            if name in interesting and value and re.search(r"[A-Za-z]", value) and 'translatable="yes"' not in attrs:
                findings.append(
                    f"{path.relative_to(root).as_posix()}:{index}: property {name!r} with text must set translatable='yes'"
                )
    return findings
