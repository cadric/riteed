from __future__ import annotations

from dataclasses import dataclass
import re
from typing import Any


class WorkflowParseError(ValueError):
    pass


@dataclass(frozen=True)
class Step:
    name: str
    uses: str
    run: str
    env: dict[str, str]
    raw: dict[str, Any]


@dataclass(frozen=True)
class Job:
    job_id: str
    permissions: dict[str, str]
    environment: str
    needs: list[str]
    env: dict[str, str]
    steps: list[Step]
    raw: dict[str, Any]


@dataclass(frozen=True)
class Workflow:
    label: str
    raw: dict[str, Any]
    triggers: dict[str, Any]
    permissions: dict[str, str]
    jobs: dict[str, Job]


@dataclass(frozen=True)
class _Line:
    number: int
    indent: int
    text: str


def parse_workflow(text: str, label: str) -> Workflow:
    parser = _Parser(text, label)
    raw = parser.parse()
    if not isinstance(raw, dict):
        raise WorkflowParseError(f"{label}: workflow root must be a mapping")
    return Workflow(
        label=label,
        raw=raw,
        triggers=_mapping(raw.get("on")),
        permissions=_string_map(raw.get("permissions")),
        jobs=_jobs(raw.get("jobs")),
    )


def _jobs(value: Any) -> dict[str, Job]:
    jobs: dict[str, Job] = {}
    for job_id, raw_job in _mapping(value).items():
        if not isinstance(raw_job, dict):
            continue
        steps: list[Step] = []
        for raw_step in _list(raw_job.get("steps")):
            if not isinstance(raw_step, dict):
                continue
            steps.append(
                Step(
                    name=_scalar(raw_step.get("name")),
                    uses=_scalar(raw_step.get("uses")),
                    run=_scalar(raw_step.get("run")),
                    env=_string_map(raw_step.get("env")),
                    raw=raw_step,
                )
            )
        jobs[str(job_id)] = Job(
            job_id=str(job_id),
            permissions=_string_map(raw_job.get("permissions")),
            environment=_environment_name(raw_job.get("environment")),
            needs=_needs(raw_job.get("needs")),
            env=_string_map(raw_job.get("env")),
            steps=steps,
            raw=raw_job,
        )
    return jobs


def _environment_name(value: Any) -> str:
    if isinstance(value, dict):
        return _scalar(value.get("name"))
    return _scalar(value)


def _needs(value: Any) -> list[str]:
    if isinstance(value, list):
        return [str(item) for item in value if str(item).strip()]
    text = _scalar(value)
    return [text] if text else []


def _mapping(value: Any) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}


def _list(value: Any) -> list[Any]:
    return value if isinstance(value, list) else []


def _string_map(value: Any) -> dict[str, str]:
    return {str(key): _scalar(item) for key, item in _mapping(value).items()}


def _scalar(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, str):
        return value
    return str(value)


class _Parser:
    def __init__(self, text: str, label: str) -> None:
        self.label = label
        self.lines = [_Line(index, _indent(line), line) for index, line in enumerate(text.splitlines(), start=1)]
        self.index = 0

    def parse(self) -> Any:
        self._skip_ignored()
        if self.index >= len(self.lines):
            return {}
        value = self._parse_block(self.lines[self.index].indent)
        self._skip_ignored()
        if self.index < len(self.lines):
            line = self.lines[self.index]
            self._error(line, "unexpected trailing content")
        return value

    def _parse_block(self, indent: int) -> Any:
        self._skip_ignored()
        if self.index >= len(self.lines):
            return {}
        line = self.lines[self.index]
        if line.indent < indent:
            return {}
        if line.indent != indent:
            self._error(line, f"unexpected indentation, expected {indent} spaces")
        if line.text.lstrip().startswith("- "):
            return self._parse_sequence(indent)
        return self._parse_mapping(indent)

    def _parse_mapping(self, indent: int) -> dict[str, Any]:
        result: dict[str, Any] = {}
        while self.index < len(self.lines):
            self._skip_ignored()
            if self.index >= len(self.lines):
                break
            line = self.lines[self.index]
            if line.indent < indent:
                break
            if line.indent > indent:
                self._error(line, f"unexpected nested mapping content at {line.indent} spaces")
            text = line.text.lstrip()
            if text.startswith("- "):
                break
            key, raw_value = self._split_key_value(line, text)
            self.index += 1
            result[key] = self._value_for(line, raw_value, indent)
        return result

    def _parse_sequence(self, indent: int) -> list[Any]:
        result: list[Any] = []
        while self.index < len(self.lines):
            self._skip_ignored()
            if self.index >= len(self.lines):
                break
            line = self.lines[self.index]
            if line.indent < indent:
                break
            if line.indent != indent:
                self._error(line, f"unexpected sequence indentation at {line.indent} spaces")
            text = line.text.lstrip()
            if not text.startswith("- "):
                break
            body = text[2:].strip()
            self._reject_unsupported(line, body)
            self.index += 1
            if not body:
                result.append(self._nested_or_empty(indent))
                continue
            split = self._try_split_key_value(line, body)
            if split is not None:
                key, raw_value = split
                item = {key: self._value_for(line, raw_value, indent)}
                if self._next_is_child(indent):
                    nested = self._parse_block(self.lines[self.index].indent)
                    if isinstance(nested, dict):
                        item.update(nested)
                    else:
                        self._error(self.lines[self.index - 1], "sequence mapping child must be a mapping")
                result.append(item)
            else:
                result.append(self._parse_scalar(line, body))
        return result

    def _value_for(self, line: _Line, raw_value: str | None, indent: int) -> Any:
        if raw_value is None or raw_value == "":
            return self._nested_or_empty(indent)
        value = raw_value.strip()
        self._reject_unsupported(line, value)
        if value in {"|", "|-", ">", ">-"}:
            return self._parse_block_scalar(indent, folded=value.startswith(">"))
        return self._parse_scalar(line, value)

    def _nested_or_empty(self, indent: int) -> Any:
        if self._next_is_child(indent):
            return self._parse_block(self.lines[self.index].indent)
        return None

    def _next_is_child(self, indent: int) -> bool:
        saved = self.index
        self._skip_ignored()
        has_child = self.index < len(self.lines) and self.lines[self.index].indent > indent
        self.index = saved
        return has_child

    def _parse_block_scalar(self, indent: int, *, folded: bool) -> str:
        collected: list[str] = []
        while self.index < len(self.lines):
            line = self.lines[self.index]
            if line.text.strip() and line.indent <= indent:
                break
            if not line.text.strip():
                collected.append("")
            else:
                collected.append(line.text[indent + 2 :] if len(line.text) >= indent + 2 else "")
            self.index += 1
        if folded:
            return "\n".join(part if not part else part.rstrip() for part in collected)
        return "\n".join(collected) + ("\n" if collected else "")

    def _split_key_value(self, line: _Line, text: str) -> tuple[str, str | None]:
        split = self._try_split_key_value(line, text)
        if split is None:
            self._error(line, "expected mapping entry")
        return split

    def _try_split_key_value(self, line: _Line, text: str) -> tuple[str, str | None] | None:
        pos = _find_unquoted_colon(text)
        if pos < 0:
            return None
        raw_key = text[:pos].strip()
        if not raw_key:
            self._error(line, "mapping key must not be empty")
        key = self._parse_scalar(line, raw_key)
        if not isinstance(key, str) or not key:
            self._error(line, "mapping key must be a string")
        raw_value = _strip_comment(text[pos + 1 :]).strip()
        return key, raw_value if raw_value else None

    def _parse_scalar(self, line: _Line, value: str) -> str:
        self._reject_unsupported(line, value)
        value = _strip_comment(value).strip()
        if not value:
            return ""
        if value[0] in {"{", "["}:
            self._error(line, "workflow validator does not support flow-style collections")
        if value.startswith("'"):
            if not value.endswith("'") or len(value) == 1:
                self._error(line, "unterminated single-quoted string")
            return value[1:-1].replace("''", "'")
        if value.startswith('"'):
            if not value.endswith('"') or len(value) == 1:
                self._error(line, "unterminated double-quoted string")
            return _decode_double_quoted(value[1:-1], line, self)
        return value

    def _skip_ignored(self) -> None:
        while self.index < len(self.lines):
            text = self.lines[self.index].text.strip()
            if not text or text.startswith("#"):
                self.index += 1
                continue
            self._reject_document_marker(self.lines[self.index])
            break

    def _reject_document_marker(self, line: _Line) -> None:
        text = line.text.strip()
        if text.startswith("---") or text.startswith("..."):
            self._error(line, "workflow validator does not support multi-document YAML")

    def _reject_unsupported(self, line: _Line, text: str) -> None:
        scan = _strip_comment(text)
        if re.search(r"(^|[\s:])([&*][A-Za-z0-9_-]+)", scan):
            self._error(line, "workflow validator does not support anchors or aliases")
        if re.search(r"(^|[\s:])![A-Za-z!]", scan):
            self._error(line, "workflow validator does not support tag annotations")

    def _error(self, line: _Line, message: str) -> None:
        raise WorkflowParseError(f"{self.label}: workflow validator does not support construct at line {line.number}: {message}")


def _indent(line: str) -> int:
    if "\t" in line[: len(line) - len(line.lstrip())]:
        return -1
    return len(line) - len(line.lstrip(" "))


def _find_unquoted_colon(text: str) -> int:
    quote = ""
    escaped = False
    for index, char in enumerate(text):
        if escaped:
            escaped = False
            continue
        if quote == '"' and char == "\\":
            escaped = True
            continue
        if char in {"'", '"'}:
            if not quote:
                quote = char
            elif quote == char:
                quote = ""
            continue
        if char == ":" and not quote:
            if index + 1 == len(text) or text[index + 1].isspace():
                return index
    return -1


def _strip_comment(text: str) -> str:
    quote = ""
    escaped = False
    for index, char in enumerate(text):
        if escaped:
            escaped = False
            continue
        if quote == '"' and char == "\\":
            escaped = True
            continue
        if char in {"'", '"'}:
            if not quote:
                quote = char
            elif quote == char:
                quote = ""
            continue
        if char == "#" and not quote and (index == 0 or text[index - 1].isspace()):
            return text[:index]
    return text


def _decode_double_quoted(value: str, line: _Line, parser: _Parser) -> str:
    escapes = {'"': '"', "\\": "\\", "/": "/", "n": "\n", "r": "\r", "t": "\t", "0": "\0"}
    output: list[str] = []
    index = 0
    while index < len(value):
        char = value[index]
        if char != "\\":
            output.append(char)
            index += 1
            continue
        index += 1
        if index >= len(value):
            parser._error(line, "unterminated double-quoted escape")
        escape = value[index]
        if escape not in escapes:
            parser._error(line, f"unsupported double-quoted escape \\{escape}")
        output.append(escapes[escape])
        index += 1
    return "".join(output)
