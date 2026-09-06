from __future__ import annotations

import re
from pathlib import Path

from tools.checks.foundation import rust_policy, validation_policy
from tools.scanners.rust import runtime_review_hits, rust_files, source_regex_hits
from tools.scanners.rust_syntax import RustSyntax
from tools.scanners.sites import load_review_entries, validate_review_links
from tools.validation_tooling import read_text


def check_runtime(root: Path, errors: list[str]) -> None:
    policy = rust_policy(root)
    enforcement = policy.get("enforcement", {})
    errors.extend(source_regex_hits(root, enforcement.get("hard_fail_patterns", [])))

    spawn_patterns = [re.compile(item) for item in enforcement.get("spawn_call_patterns", [])]
    blocking_patterns = [re.compile(item) for item in enforcement.get("blocking_call_patterns", [])]
    for path in rust_files(root):
        syntax = RustSyntax(read_text(path))
        rel = path.relative_to(root).as_posix()
        for index, (line, masked_line) in enumerate(
            zip(syntax.source_lines, syntax.masked_lines, strict=True), start=1
        ):
            if any(regex.search(masked_line) for regex in spawn_patterns):
                if "let _ =" in line or ("=" not in line and ".await" not in line):
                    errors.append(f"{rel}:{index}: task handles must be owned or supervised, not detached")
            blocking_matches = (
                match
                for regex in blocking_patterns
                for match in regex.finditer(masked_line)
            )
            if any(syntax.is_async(index, match.start()) for match in blocking_matches):
                errors.append(f"{rel}:{index}: blocking calls inside async contexts require reviewed offloading")

    hits = runtime_review_hits(root, enforcement.get("review_required_patterns", []))
    entries = load_review_entries(root, "runtime", validation_policy(root)["review_artifacts"]["runtime"], errors)
    validate_review_links(root, hits, entries, errors)
