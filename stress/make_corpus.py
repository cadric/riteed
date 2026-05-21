#!/usr/bin/env python3
"""Generate deterministic local stress corpus files for Riteed."""

from __future__ import annotations

import argparse
from pathlib import Path

OPEN_FILE_LIMIT_BYTES = 25 * 1024 * 1024
SEARCH_CHAR_LIMIT = 2_000_000
MARKDOWN_PREVIEW_MAX_BYTES = 1_000_000


def repeat_seed(seed: bytes, size: int) -> bytes:
    if not seed:
        return b"x" * size
    full, partial = divmod(size, len(seed))
    return seed * full + seed[:partial]


def write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def generate(root: Path) -> None:
    seeds = Path(__file__).resolve().parent / "corpus" / "seeds"
    open_seed = (seeds / "open-boundary.txt").read_bytes()
    search_seed = (seeds / "search-boundary.txt").read_text(encoding="utf-8").strip()
    generated = root / "generated"

    write_bytes(
        generated / "open-at-cap.txt",
        repeat_seed(open_seed, OPEN_FILE_LIMIT_BYTES),
    )
    write_bytes(
        generated / "open-over-cap.txt",
        repeat_seed(open_seed, OPEN_FILE_LIMIT_BYTES + 1),
    )
    write_text(
        generated / "search-at-cap.txt",
        search_seed + ("x" * (SEARCH_CHAR_LIMIT - len(search_seed))),
    )
    write_text(
        generated / "markdown-preview-over-cap.md",
        "# Large Markdown\n\n" + ("body\n" * ((MARKDOWN_PREVIEW_MAX_BYTES // 5) + 1)),
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parent / "corpus",
        help="Corpus root to populate.",
    )
    args = parser.parse_args()
    generate(args.root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
