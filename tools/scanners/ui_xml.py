from __future__ import annotations

import re
from collections.abc import Iterator
from dataclasses import dataclass, field
from pathlib import Path
from xml.parsers import expat

from tools.scanners.sites import ScanHit
from tools.validation_tooling import fail, normalize_path, read_text, scoped_files

SURFACE_CLASS_RE = re.compile(
    r"(?:ApplicationWindow|Window|Dialog|PreferencesWindow|NavigationSplitView|NavigationView)"
)
BUTTON_CLASS_RE = re.compile(r"(?:Button|MenuButton|SplitButton|ToggleButton)")
INTERESTING_PROPERTIES = {
    "label",
    "title",
    "subtitle",
    "tooltip-text",
    "placeholder-text",
    "text",
}
BUTTON_NAMES = {"label", "accessible-name", "tooltip-text"}


@dataclass
class _XmlNode:
    tag: str
    attrs: dict[str, str]
    line: int
    content: list[str | _XmlNode] = field(default_factory=list)


class _UiXmlParseError(ValueError):
    pass


def _parse_ui(path: Path, root: Path) -> tuple[list[_XmlNode], list[str]]:
    source = read_text(path)
    source_lines = source.splitlines()
    roots: list[_XmlNode] = []
    stack: list[_XmlNode] = []
    parser = expat.ParserCreate()

    def start_element(tag: str, attrs: dict[str, str]) -> None:
        node = _XmlNode(tag=tag, attrs=dict(attrs), line=parser.CurrentLineNumber)
        if stack:
            stack[-1].content.append(node)
        else:
            roots.append(node)
        stack.append(node)

    def end_element(_tag: str) -> None:
        stack.pop()

    def character_data(value: str) -> None:
        if stack:
            stack[-1].content.append(value)

    parser.StartElementHandler = start_element
    parser.EndElementHandler = end_element
    parser.CharacterDataHandler = character_data
    try:
        parser.Parse(source, True)
    except expat.ExpatError as exc:
        rel = normalize_path(path.relative_to(root).as_posix())
        raise _UiXmlParseError(f"{rel}:{exc.lineno}: invalid XML ({exc})") from exc
    return roots, source_lines


def _nodes(roots: list[_XmlNode]) -> Iterator[_XmlNode]:
    pending = list(reversed(roots))
    while pending:
        node = pending.pop()
        yield node
        pending.extend(
            reversed([item for item in node.content if isinstance(item, _XmlNode)])
        )


def _text(node: _XmlNode) -> str:
    chunks: list[str] = []
    pending: list[str | _XmlNode] = list(reversed(node.content))
    while pending:
        item = pending.pop()
        if isinstance(item, str):
            chunks.append(item)
        else:
            pending.extend(reversed(item.content))
    return "".join(chunks)


def _children(node: _XmlNode) -> list[_XmlNode]:
    return [item for item in node.content if isinstance(item, _XmlNode)]


def _properties(nodes: list[_XmlNode]) -> list[_XmlNode]:
    return [node for node in _nodes(nodes) if node.tag == "property"]


def _has_nonempty_property(properties: list[_XmlNode], names: set[str]) -> bool:
    return any(
        item.attrs.get("name") in names and _text(item).strip()
        for item in properties
    )


def _opening_line(source_lines: list[str], node: _XmlNode) -> str:
    if 1 <= node.line <= len(source_lines):
        return source_lines[node.line - 1].strip()
    return f"<{node.tag}"


def _report_or_fail(errors: list[str] | None, message: str) -> None:
    if errors is None:
        fail(message)
    errors.append(message)


def ui_surface_hits(root: Path, errors: list[str] | None = None) -> list[ScanHit]:
    hits: list[ScanHit] = []
    for path in scoped_files(root, ["data/**/*.ui"]):
        try:
            roots, source_lines = _parse_ui(path, root)
        except _UiXmlParseError as exc:
            _report_or_fail(errors, str(exc))
            continue
        rel = normalize_path(path.relative_to(root).as_posix())
        seen: set[tuple[int, str]] = set()
        for node in _nodes(roots):
            class_name = node.attrs.get("class", "")
            parent_name = node.attrs.get("parent", "") if node.tag == "template" else ""
            kind = ""
            message = ""
            if node.tag in {"template", "object"} and (
                SURFACE_CLASS_RE.search(class_name) or SURFACE_CLASS_RE.search(parent_name)
            ):
                kind = "surface"
                message = "UI surfaces require adaptive review coverage."
            elif node.tag == "menu":
                kind = "menu"
                message = "Menus require review coverage."
            if not kind:
                continue
            if (node.line, kind) in seen:
                _report_or_fail(
                    errors,
                    f"{rel}:{node.line}: multiple {kind!r} review sites share one source line",
                )
                continue
            seen.add((node.line, kind))
            hits.append(
                ScanHit(
                    path=rel,
                    line=node.line,
                    kind=kind,
                    match=_opening_line(source_lines, node),
                    message=message,
                )
            )
    return hits


def icon_only_buttons(root: Path) -> list[str]:
    findings: list[str] = []
    for path in scoped_files(root, ["data/**/*.ui"]):
        try:
            roots, _source_lines = _parse_ui(path, root)
        except _UiXmlParseError as exc:
            findings.append(str(exc))
            continue
        rel = normalize_path(path.relative_to(root).as_posix())
        for node in _nodes(roots):
            if node.tag != "object" or not BUTTON_CLASS_RE.search(node.attrs.get("class", "")):
                continue
            children = _children(node)
            direct_properties = [item for item in children if item.tag == "property"]
            visible_children = [
                item
                for item in children
                if item.tag == "child"
                or (item.tag == "property" and item.attrs.get("name") == "child")
            ]
            accessibility = [item for item in children if item.tag == "accessibility"]
            visible_properties = _properties(visible_children)
            accessibility_properties = _properties(accessibility)
            has_icon = _has_nonempty_property(
                direct_properties + visible_properties,
                {"icon-name"},
            )
            has_name = _has_nonempty_property(
                direct_properties + visible_properties,
                BUTTON_NAMES,
            ) or _has_nonempty_property(
                accessibility_properties,
                {"label"},
            )
            if has_icon and not has_name:
                findings.append(
                    f"{rel}:{node.line}: icon-only interactive element lacks accessible naming"
                )
    return findings


def translatable_property_errors(root: Path) -> list[str]:
    findings: list[str] = []
    for path in scoped_files(root, ["data/**/*.ui"]):
        try:
            roots, _source_lines = _parse_ui(path, root)
        except _UiXmlParseError as exc:
            findings.append(str(exc))
            continue
        rel = normalize_path(path.relative_to(root).as_posix())
        for node in _nodes(roots):
            if node.tag != "property":
                continue
            name = node.attrs.get("name", "")
            value = _text(node).strip()
            if (
                name in INTERESTING_PROPERTIES
                and value
                and any(char.isalpha() for char in value)
                and node.attrs.get("translatable") != "yes"
            ):
                findings.append(
                    f"{rel}:{node.line}: property {name!r} with text must set translatable='yes'"
                )
    return findings
