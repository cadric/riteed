from __future__ import annotations

import ast
from typing import Any


APPSTREAM_PATH = "app/data/io.github.cadric.Riteed.metainfo.xml"


def exact_guard_owner(scanned: list[Any], block: str) -> int | None:
    expected = [line.strip() for line in block.splitlines()]
    owners = [
        line
        for line in scanned
        if line.text == expected[0] and not line.controls and not line.substituted
    ]
    if len(owners) != 1:
        return None
    by_index = {line.index: line for line in scanned}
    for offset, text in enumerate(expected):
        line = by_index.get(owners[0].index + offset)
        controls = () if offset == 0 else ("if",)
        if (
            line is None
            or line.text != text
            or line.controls != controls
            or line.substituted
            or line.heredoc_marker
        ):
            return None
    return owners[0].index


def identity_outputs_are_unique(scanned: list[Any]) -> bool:
    for name in ("version", "release_ref", "tag_commit"):
        expected = f'echo "{name}=${name}" >> "$GITHUB_OUTPUT"'
        matches = [
            line
            for line in scanned
            if "$GITHUB_OUTPUT" in line.text and f"{name}=" in line.text
        ]
        if len(matches) != 1 or matches[0].text != expected or matches[0].controls:
            return False
    return True


def appstream_ast_is_guard(source: str) -> bool:
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return False
    expected_imports = {
        ast.dump(ast.parse("import os").body[0]),
        ast.dump(ast.parse("import subprocess").body[0]),
        ast.dump(ast.parse("import sys").body[0]),
        ast.dump(ast.parse("import xml.etree.ElementTree as ET").body[0]),
    }
    actual_imports = {
        ast.dump(node)
        for node in tree.body
        if isinstance(node, (ast.Import, ast.ImportFrom))
    }
    if not expected_imports.issubset(actual_imports):
        return False
    names = (
        "version",
        "tag_commit",
        "metadata",
        "root",
        "releases",
        "first_release",
        "release_version",
    )
    writes = {
        name: [
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.Name)
            and node.id == name
            and isinstance(node.ctx, ast.Store)
        ]
        for name in names
    }
    if any(len(writes[name]) != 1 for name in names):
        return False
    if any(
        isinstance(node, ast.Name)
        and node.id in {"os", "subprocess", "sys", "ET"}
        and isinstance(node.ctx, ast.Store)
        for node in ast.walk(tree)
    ):
        return False
    assignments = {
        node.targets[0].id: (index, node.value)
        for index, node in enumerate(tree.body)
        if isinstance(node, ast.Assign)
        and len(node.targets) == 1
        and isinstance(node.targets[0], ast.Name)
        and node.targets[0].id in names
    }
    if not (
        len(assignments) == len(names)
        and _is_env_value(assignments["version"][1], "VERSION")
        and _is_env_value(assignments["tag_commit"][1], "TAG_COMMIT")
        and _is_metadata_bytes(assignments["metadata"][1])
        and _is_appstream_root(assignments["root"][1])
        and _is_method_call(assignments["releases"][1], "root", "find", "releases")
        and _is_method_call(
            assignments["first_release"][1], "releases", "find", "release"
        )
        and _is_method_call(
            assignments["release_version"][1], "first_release", "get", "version"
        )
    ):
        return False
    assignment_positions = [assignments[name][0] for name in names]
    if assignment_positions != sorted(assignment_positions):
        return False
    guards = [
        (index, node)
        for index, node in enumerate(tree.body)
        if isinstance(node, ast.If) and _is_version_mismatch(node.test)
    ]
    return (
        len(guards) == 1
        and guards[0][0] > assignment_positions[-1]
        and any(_is_exit_one(node) for node in guards[0][1].body)
    )


def _is_env_value(node: ast.expr | None, name: str) -> bool:
    return (
        isinstance(node, ast.Subscript)
        and isinstance(node.value, ast.Attribute)
        and isinstance(node.value.value, ast.Name)
        and node.value.value.id == "os"
        and node.value.attr == "environ"
        and isinstance(node.slice, ast.Constant)
        and node.slice.value == name
    )


def _is_appstream_root(node: ast.expr | None) -> bool:
    return (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == "fromstring"
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "ET"
        and len(node.args) == 1
        and isinstance(node.args[0], ast.Name)
        and node.args[0].id == "metadata"
    )


def _is_metadata_bytes(node: ast.expr | None) -> bool:
    return (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and node.func.attr == "check_output"
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "subprocess"
        and len(node.args) == 1
        and isinstance(node.args[0], ast.List)
        and len(node.args[0].elts) == 3
        and _is_string(node.args[0].elts[0], "git")
        and _is_string(node.args[0].elts[1], "show")
        and _is_metadata_object(node.args[0].elts[2])
    )


def _is_string(node: ast.expr, value: str) -> bool:
    return isinstance(node, ast.Constant) and node.value == value


def _is_metadata_object(node: ast.expr) -> bool:
    return (
        isinstance(node, ast.JoinedStr)
        and len(node.values) == 2
        and isinstance(node.values[0], ast.FormattedValue)
        and isinstance(node.values[0].value, ast.Name)
        and node.values[0].value.id == "tag_commit"
        and isinstance(node.values[1], ast.Constant)
        and node.values[1].value == f":{APPSTREAM_PATH}"
    )


def _is_method_call(
    node: ast.expr | None, receiver: str, method: str, argument: str
) -> bool:
    return (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == receiver
        and node.func.attr == method
        and len(node.args) == 1
        and isinstance(node.args[0], ast.Constant)
        and node.args[0].value == argument
    )


def _is_version_mismatch(node: ast.expr) -> bool:
    return (
        isinstance(node, ast.Compare)
        and isinstance(node.left, ast.Name)
        and node.left.id == "release_version"
        and len(node.ops) == 1
        and isinstance(node.ops[0], ast.NotEq)
        and len(node.comparators) == 1
        and isinstance(node.comparators[0], ast.Name)
        and node.comparators[0].id == "version"
    )


def _is_exit_one(node: ast.stmt) -> bool:
    return (
        isinstance(node, ast.Expr)
        and isinstance(node.value, ast.Call)
        and isinstance(node.value.func, ast.Attribute)
        and isinstance(node.value.func.value, ast.Name)
        and node.value.func.value.id == "sys"
        and node.value.func.attr == "exit"
        and len(node.value.args) == 1
        and isinstance(node.value.args[0], ast.Constant)
        and node.value.args[0].value == 1
    )
