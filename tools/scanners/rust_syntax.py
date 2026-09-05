from __future__ import annotations

import re
from dataclasses import dataclass


TOKEN_RE = re.compile(
    r"r#[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*|::|->|=>|[{}()\[\];#!|=,]"
)


@dataclass(frozen=True)
class _Token:
    text: str
    start: int
    end: int


class RustSyntax:
    """Conservative lexical view used by policy scanners.

    This masks comments and literals, then tracks only the brace-delimited
    Rust constructs needed by the policy checks. It is intentionally not a
    complete Rust parser.
    """

    def __init__(self, source: str) -> None:
        self.source = source
        self.masked = _mask_comments_and_literals(source)
        self.source_lines = source.splitlines()
        self.masked_lines = self.masked.splitlines()
        self._line_offsets = _line_offsets(source)
        self._tokens = _tokens(self.masked)
        self._pairs = _delimiter_pairs(self._tokens)
        self._test_only_ranges = self._cfg_test_ranges()
        self._async_ranges = self._async_context_ranges()

    def is_test_only(self, line: int, column: int) -> bool:
        return self._in_ranges(self._offset(line, column), self._test_only_ranges)

    def is_async(self, line: int, column: int) -> bool:
        return self._in_ranges(self._offset(line, column), self._async_ranges)

    def _offset(self, line: int, column: int) -> int:
        if line < 1 or line > len(self._line_offsets):
            return -1
        return self._line_offsets[line - 1] + column

    @staticmethod
    def _in_ranges(offset: int, ranges: list[tuple[int, int]]) -> bool:
        return any(start <= offset < end for start, end in ranges)

    def _cfg_test_ranges(self) -> list[tuple[int, int]]:
        ranges: list[tuple[int, int]] = []
        for index in range(len(self._tokens) - 1):
            if self._tokens[index].text != "#" or self._tokens[index + 1].text != "[":
                continue
            close = self._pairs.get(index + 1)
            if close is None:
                continue
            contents = [token.text for token in self._tokens[index + 2 : close]]
            if contents != ["cfg", "(", "test", ")"]:
                continue
            item_end = self._attached_item_end(close + 1)
            if item_end is not None:
                ranges.append((self._tokens[index].start, self._tokens[item_end].end))
        return ranges

    def _attached_item_end(self, index: int) -> int | None:
        while index + 1 < len(self._tokens):
            if self._tokens[index].text != "#" or self._tokens[index + 1].text != "[":
                break
            close = self._pairs.get(index + 1)
            if close is None:
                return None
            index = close + 1

        while index < len(self._tokens):
            text = self._tokens[index].text
            if text == ";":
                return index
            if text == "{":
                return self._pairs.get(index)
            if text == "}":
                return None
            index += 1
        return None

    def _async_context_ranges(self) -> list[tuple[int, int]]:
        ranges: list[tuple[int, int]] = []
        for index, token in enumerate(self._tokens):
            if token.text != "async":
                continue
            opening = self._async_opening_brace(index + 1)
            if opening is None:
                continue
            closing = self._pairs.get(opening)
            if closing is not None:
                ranges.append((self._tokens[opening].start, self._tokens[closing].end))
        return ranges

    def _async_opening_brace(self, index: int) -> int | None:
        if index < len(self._tokens) and self._tokens[index].text == "move":
            index += 1
        if index >= len(self._tokens):
            return None
        if self._tokens[index].text == "{":
            return index
        if self._tokens[index].text == "|":
            index += 1
            while index < len(self._tokens) and self._tokens[index].text != "|":
                index += 1
            index += 1
            return index if index < len(self._tokens) and self._tokens[index].text == "{" else None
        if self._tokens[index].text != "fn":
            return None
        index += 1
        while index < len(self._tokens):
            text = self._tokens[index].text
            if text == "{":
                return index
            if text in {";", "}"}:
                return None
            index += 1
        return None


def _line_offsets(source: str) -> list[int]:
    offsets: list[int] = []
    offset = 0
    for line in source.splitlines(keepends=True):
        offsets.append(offset)
        offset += len(line)
    return offsets


def _tokens(masked: str) -> list[_Token]:
    tokens: list[_Token] = []
    offset = 0
    for line in masked.splitlines(keepends=True):
        for match in TOKEN_RE.finditer(line):
            tokens.append(
                _Token(
                    text=match.group(),
                    start=offset + match.start(),
                    end=offset + match.end(),
                )
            )
        offset += len(line)
    return tokens


def _delimiter_pairs(tokens: list[_Token]) -> dict[int, int]:
    pairs: dict[int, int] = {}
    stack: list[tuple[str, int]] = []
    opening = {"{": "}", "(": ")", "[": "]"}
    for index, token in enumerate(tokens):
        if token.text in opening:
            stack.append((token.text, index))
            continue
        if not stack or token.text != opening[stack[-1][0]]:
            continue
        _, start = stack.pop()
        pairs[start] = index
        pairs[index] = start
    return pairs


def _mask_comments_and_literals(source: str) -> str:
    masked = list(source)
    index = 0
    block_depth = 0
    while index < len(source):
        if block_depth:
            if source.startswith("/*", index):
                _blank(masked, index, index + 2)
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                _blank(masked, index, index + 2)
                block_depth -= 1
                index += 2
            else:
                _blank(masked, index, index + 1)
                index += 1
            continue
        if source.startswith("//", index):
            end = source.find("\n", index)
            end = len(source) if end == -1 else end
            _blank(masked, index, end)
            index = end
            continue
        if source.startswith("/*", index):
            _blank(masked, index, index + 2)
            block_depth = 1
            index += 2
            continue
        literal_end = _literal_end(source, index)
        if literal_end is not None:
            _blank(masked, index, literal_end)
            index = literal_end
            continue
        index += 1
    return "".join(masked)


def _literal_end(source: str, start: int) -> int | None:
    raw_end = _raw_literal_end(source, start)
    if raw_end is not None:
        return raw_end
    prefix_length = 1 if source[start : start + 1] in {"b", "c"} else 0
    quote_at = start + prefix_length
    if source[quote_at : quote_at + 1] == '"':
        return _quoted_end(source, quote_at, '"')
    if source[quote_at : quote_at + 1] == "'":
        return _char_end(source, quote_at)
    return None


def _raw_literal_end(source: str, start: int) -> int | None:
    prefix_length = 0
    for prefix in ("br", "cr", "r"):
        if source.startswith(prefix, start):
            prefix_length = len(prefix)
            break
    if not prefix_length:
        return None
    cursor = start + prefix_length
    while cursor < len(source) and source[cursor] == "#":
        cursor += 1
    if cursor >= len(source) or source[cursor] != '"':
        return None
    hashes = source[start + prefix_length : cursor]
    closing = '"' + hashes
    end = source.find(closing, cursor + 1)
    return len(source) if end == -1 else end + len(closing)


def _quoted_end(source: str, quote_at: int, quote: str) -> int:
    cursor = quote_at + 1
    while cursor < len(source):
        if source[cursor] == "\\":
            cursor += 2
            continue
        if source[cursor] == quote:
            return cursor + 1
        cursor += 1
    return len(source)


def _char_end(source: str, quote_at: int) -> int | None:
    cursor = quote_at + 1
    if cursor >= len(source):
        return None
    if source[cursor] == "\\":
        cursor += 1
        if cursor < len(source) and source[cursor] == "u" and source[cursor + 1 : cursor + 2] == "{":
            closing = source.find("}", cursor + 2)
            if closing == -1:
                return None
            cursor = closing + 1
        elif cursor < len(source) and source[cursor] == "x":
            cursor += 3
        else:
            cursor += 1
    else:
        cursor += 1
    return cursor + 1 if source[cursor : cursor + 1] == "'" else None


def _blank(masked: list[str], start: int, end: int) -> None:
    for index in range(start, min(end, len(masked))):
        if masked[index] not in {"\n", "\r"}:
            masked[index] = " "
