from __future__ import annotations


def textdomain_init_present(text: str) -> bool:
    source = _strip_rust_comments_and_strings(text)
    needle = "TextDomain::new"
    index = source.find(needle)
    while index >= 0:
        if _has_token_boundary(source, index):
            pos = _skip_ws(source, index + len(needle))
            if pos < len(source) and source[pos] == "(":
                end = _matching_paren(source, pos)
                if end is not None and _chain_calls_init(source, end + 1):
                    return True
        index = source.find(needle, index + 1)
    return False


def _has_token_boundary(source: str, index: int) -> bool:
    if index == 0:
        return True
    previous = source[index - 1]
    return not (previous.isalnum() or previous == "_")


def _skip_ws(source: str, index: int) -> int:
    while index < len(source) and source[index].isspace():
        index += 1
    return index


def _matching_paren(source: str, start: int) -> int | None:
    depth = 0
    for index in range(start, len(source)):
        if source[index] == "(":
            depth += 1
        elif source[index] == ")":
            depth -= 1
            if depth == 0:
                return index
    return None


def _chain_calls_init(source: str, index: int) -> bool:
    pos = index
    while True:
        pos = _skip_ws(source, pos)
        if pos >= len(source) or source[pos] != ".":
            return False
        pos += 1
        name_start = pos
        while pos < len(source) and (source[pos].isalnum() or source[pos] == "_"):
            pos += 1
        name = source[name_start:pos]
        pos = _skip_ws(source, pos)
        if pos >= len(source) or source[pos] != "(":
            return False
        end = _matching_paren(source, pos)
        if end is None:
            return False
        if name == "init":
            return True
        pos = end + 1


def _strip_rust_comments_and_strings(text: str) -> str:
    chars = list(text)
    index = 0
    block_depth = 0
    while index < len(chars):
        if block_depth > 0:
            if _starts(chars, index, "/*"):
                chars[index] = chars[index + 1] = " "
                block_depth += 1
                index += 2
            elif _starts(chars, index, "*/"):
                chars[index] = chars[index + 1] = " "
                block_depth -= 1
                index += 2
            else:
                if chars[index] != "\n":
                    chars[index] = " "
                index += 1
            continue
        if _starts(chars, index, "//"):
            index = _blank_line_comment(chars, index)
        elif _starts(chars, index, "/*"):
            chars[index] = chars[index + 1] = " "
            block_depth = 1
            index += 2
        elif chars[index] == "r" and _raw_string_end(chars, index) is not None:
            index = _blank_range(chars, index, _raw_string_end(chars, index) or index)
        elif chars[index] == '"':
            index = _blank_quoted(chars, index, '"')
        elif chars[index] == "'" and _looks_like_char_literal(chars, index):
            index = _blank_quoted(chars, index, "'")
        else:
            index += 1
    return "".join(chars)


def _starts(chars: list[str], index: int, prefix: str) -> bool:
    return "".join(chars[index : index + len(prefix)]) == prefix


def _blank_line_comment(chars: list[str], index: int) -> int:
    while index < len(chars) and chars[index] != "\n":
        chars[index] = " "
        index += 1
    return index


def _blank_range(chars: list[str], start: int, end: int) -> int:
    for index in range(start, end):
        if chars[index] != "\n":
            chars[index] = " "
    return end


def _blank_quoted(chars: list[str], start: int, quote: str) -> int:
    index = start + 1
    while index < len(chars):
        if chars[index] == "\\":
            index += 2
            continue
        if chars[index] == quote:
            return _blank_range(chars, start, index + 1)
        index += 1
    return _blank_range(chars, start, len(chars))


def _looks_like_char_literal(chars: list[str], index: int) -> bool:
    if index + 2 >= len(chars):
        return False
    if chars[index + 1] == "\\":
        return index + 3 < len(chars) and chars[index + 3] == "'"
    return chars[index + 2] == "'"


def _raw_string_end(chars: list[str], start: int) -> int | None:
    index = start + 1
    while index < len(chars) and chars[index] == "#":
        index += 1
    if index >= len(chars) or chars[index] != '"':
        return None
    hashes = index - start - 1
    terminator = '"' + ("#" * hashes)
    search = index + 1
    while search < len(chars):
        if _starts(chars, search, terminator):
            return search + len(terminator)
        search += 1
    return len(chars)
