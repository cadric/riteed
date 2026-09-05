from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.checks.source_scope import build_source_inventory, check_source_scope


def _write_rust(root: Path, rel: str) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("fn main() {}\n", encoding="utf-8")


def _category(category_id: str, paths: list[str], checks: list[str]) -> dict[str, object]:
    return {"id": category_id, "paths": paths, "checks": checks}


def _source_scope(*categories: dict[str, object]) -> dict[str, object]:
    return {"categories": list(categories)}


def _write_validation_policy(root: Path, payload: dict[str, object]) -> None:
    path = root / "policy" / "validation-tooling.policy.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload) + "\n", encoding="utf-8")


def _write_json(root: Path, rel: str, payload: dict[str, object]) -> None:
    path = root / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload) + "\n", encoding="utf-8")


class SourceScopeTests(unittest.TestCase):
    def test_inventory_classifies_all_rust_sources_and_reports_check_owners(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            for rel in (
                "src/main.rs",
                "tests/ui_smoke.rs",
                "build.rs",
                "fuzz/fuzz_targets/parser.rs",
                "build-aux/cargo-patches/sourceview5/src/lib.rs",
            ):
                _write_rust(root, rel)
            _write_rust(root, "target/debug/build/generated.rs")
            config = _source_scope(
                _category(
                    "application",
                    ["src/**/*.rs", "crates/**/*.rs"],
                    ["cargo-workspace", "source-patterns", "runtime-review", "coverage"],
                ),
                _category("integration", ["tests/**/*.rs"], ["cargo-workspace"]),
                _category("build-script", ["build.rs"], ["cargo-workspace"]),
                _category("fuzz-targets", ["fuzz/fuzz_targets/**/*.rs"], ["stress-fuzz"]),
                _category(
                    "sourceview5-patch",
                    ["build-aux/cargo-patches/sourceview5/**/*.rs"],
                    ["release-local-patches", "dependency-preflight"],
                ),
            )
            errors: list[str] = []

            inventory = build_source_inventory(root, config, errors)

        self.assertEqual(errors, [])
        self.assertEqual(
            inventory["application"],
            {
                "checks": ("cargo-workspace", "source-patterns", "runtime-review", "coverage"),
                "files": ("src/main.rs",),
            },
        )
        self.assertEqual(inventory["integration"]["files"], ("tests/ui_smoke.rs",))
        self.assertEqual(inventory["build-script"]["files"], ("build.rs",))
        self.assertNotIn("target/debug/build/generated.rs", str(inventory))

    def test_untracked_rust_source_outside_configured_paths_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "src/main.rs")
            _write_rust(root, "other/forgotten.rs")
            errors: list[str] = []

            build_source_inventory(
                root,
                _source_scope(_category("application", ["src/**/*.rs"], ["cargo-workspace"])),
                errors,
            )

        self.assertIn("Unclassified Rust source: other/forgotten.rs", errors)

    def test_omitting_an_existing_source_category_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "src/main.rs")
            _write_rust(root, "fuzz/fuzz_targets/parser.rs")
            errors: list[str] = []

            build_source_inventory(
                root,
                _source_scope(_category("application", ["src/**/*.rs"], ["cargo-workspace"])),
                errors,
            )

        self.assertIn("Unclassified Rust source: fuzz/fuzz_targets/parser.rs", errors)

    def test_source_matching_multiple_categories_is_ambiguous(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "src/main.rs")
            errors: list[str] = []

            build_source_inventory(
                root,
                _source_scope(
                    _category("application", ["src/**/*.rs"], ["cargo-workspace"]),
                    _category("all-src", ["src/main.rs"], ["source-patterns"]),
                ),
                errors,
            )

        self.assertIn(
            "Rust source matches multiple source_scope categories (all-src, application): src/main.rs",
            errors,
        )

    def test_missing_source_scope_configuration_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_validation_policy(root, {})
            errors: list[str] = []

            inventory = check_source_scope(root, errors)

        self.assertEqual(inventory, {})
        self.assertIn("validation-tooling source_scope must be an object", errors)

    def test_unknown_checker_owner_is_an_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "src/main.rs")
            errors: list[str] = []

            inventory = build_source_inventory(
                root,
                _source_scope(_category("application", ["src/**/*.rs"], ["made-up-check"])),
                errors,
            )

        self.assertEqual(inventory, {})
        self.assertTrue(any("unknown checker owner 'made-up-check'" in error for error in errors), errors)

    def test_claimed_source_and_line_limit_owners_must_cover_the_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "other/tool.rs")
            source_scope = _source_scope(
                _category(
                    "other",
                    ["other/**/*.rs"],
                    ["source-patterns", "line-limits"],
                )
            )
            _write_validation_policy(
                root,
                {
                    "source_scope": source_scope,
                    "forbidden_source_patterns": [
                        {"paths": ["src/**/*.rs"], "pattern": "unsafe"}
                    ],
                    "required_source_patterns": [],
                    "line_limit_globs": ["src/**/*.rs"],
                },
            )
            errors: list[str] = []

            check_source_scope(root, errors)

        self.assertTrue(any("claims source-patterns" in error for error in errors), errors)
        self.assertTrue(any("claims line-limits" in error for error in errors), errors)

    def test_claimed_runtime_owner_must_match_a_runtime_scanner_scope(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "other/tool.rs")
            _write_validation_policy(
                root,
                {
                    "source_scope": _source_scope(
                        _category("other", ["other/**/*.rs"], ["runtime-review"])
                    )
                },
            )
            _write_json(root, "policy/rust.policy.json", {"enforcement": {}})
            errors: list[str] = []

            check_source_scope(root, errors)

        self.assertTrue(any("claims runtime-review" in error for error in errors), errors)

    def test_claimed_patch_owner_must_match_a_release_policy_patch_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "build-aux/cargo-patches/unlisted/src/lib.rs")
            _write_rust(root, "build-aux/cargo-patches/listed/src/lib.rs")
            _write_validation_policy(
                root,
                {
                    "source_scope": _source_scope(
                        _category(
                            "unlisted-patch",
                            ["build-aux/cargo-patches/unlisted/**/*.rs"],
                            ["release-local-patches"],
                        ),
                        _category(
                            "listed-patch",
                            ["build-aux/cargo-patches/listed/**/*.rs"],
                            ["release-local-patches"],
                        ),
                    )
                },
            )
            _write_json(
                root,
                "policy/release.policy.json",
                {
                    "local_patch_policy": {
                        "release_critical_local_patches": [
                            {"path": "build-aux/cargo-patches/listed"}
                        ]
                    }
                },
            )
            errors: list[str] = []

            check_source_scope(root, errors)

        self.assertTrue(any("unlisted/src/lib.rs" in error for error in errors), errors)
        self.assertFalse(any(error.startswith("build-aux/cargo-patches/listed/src/lib.rs:") for error in errors), errors)

    def test_local_patch_owner_requires_an_exact_patch_subtree(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "build-aux/cargo-patches/sourceview5/src/lib.rs")
            errors: list[str] = []

            inventory = build_source_inventory(
                root,
                _source_scope(
                    _category(
                        "all-patches",
                        ["build-aux/cargo-patches/**/*.rs"],
                        ["release-local-patches"],
                    )
                ),
                errors,
            )

        self.assertEqual(inventory, {})
        self.assertTrue(any("must name one exact patch subtree" in error for error in errors), errors)

    def test_vendored_patch_cannot_be_owned_by_application_source_patterns(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "build-aux/cargo-patches/sourceview5/src/lib.rs")
            errors: list[str] = []

            build_source_inventory(
                root,
                _source_scope(
                    _category(
                        "sourceview5-patch",
                        ["build-aux/cargo-patches/sourceview5/**/*.rs"],
                        ["source-patterns"],
                    )
                ),
                errors,
            )

        self.assertTrue(any("must not use source-patterns" in error for error in errors), errors)
        self.assertTrue(any("require release-local-patches" in error for error in errors), errors)

    def test_category_schema_requires_nonempty_ids_globs_and_checks(self) -> None:
        invalid_categories = (
            _category("", ["src/**/*.rs"], ["cargo-workspace"]),
            _category("application", [], ["cargo-workspace"]),
            _category("application", [""], ["cargo-workspace"]),
            _category("application", ["src/**/*.rs"], []),
            {"id": "application", "paths": ["src/**/*.rs"]},
        )
        for category in invalid_categories:
            with self.subTest(category=category), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _write_rust(root, "src/main.rs")
                errors: list[str] = []

                inventory = build_source_inventory(root, _source_scope(category), errors)

                self.assertEqual(inventory, {})
                self.assertTrue(errors)

    def test_path_globs_must_be_target_relative_rust_sources(self) -> None:
        invalid_paths = ("../outside.rs", "/absolute.rs", "src/**")
        for invalid_path in invalid_paths:
            with self.subTest(path=invalid_path), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _write_rust(root, "src/main.rs")
                errors: list[str] = []

                inventory = build_source_inventory(
                    root,
                    _source_scope(
                        _category("application", [invalid_path], ["cargo-workspace"])
                    ),
                    errors,
                )

                self.assertEqual(inventory, {})
                self.assertTrue(errors)

    def test_duplicate_category_ids_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write_rust(root, "src/main.rs")
            errors: list[str] = []

            inventory = build_source_inventory(
                root,
                _source_scope(
                    _category("application", ["src/**/*.rs"], ["cargo-workspace"]),
                    _category("application", ["tests/**/*.rs"], ["cargo-workspace"]),
                ),
                errors,
            )

        self.assertEqual(inventory, {})
        self.assertTrue(any("duplicate category id 'application'" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
