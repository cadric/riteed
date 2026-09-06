from __future__ import annotations

import copy
import json
import tempfile
import tomllib
import unittest
from collections.abc import Callable
from typing import Any

from tools.checks.dependency_preflight import check_dependency_preflight
from tools.tests.test_dependency_preflight import CHECKSUM_A, CHECKSUM_B, REPO_ROOT, _fixture

Mutation = Callable[[list[Any]], None]

VENDOR_CONFIG = {
    "type": "inline",
    "contents": (
        '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n'
        '[source.crates-io]\nreplace-with = "vendored-sources"\n'
    ),
    "dest": "cargo",
    "dest-filename": "config",
}


def _append(entry: Any) -> Mutation:
    return lambda sources: sources.append(copy.deepcopy(entry))


def _set(index: int, key: str, value: Any) -> Mutation:
    return lambda sources: sources[index].__setitem__(key, value)


def _append_twice(entry: dict[str, Any]) -> Mutation:
    def mutate(sources: list[Any]) -> None:
        sources.extend((copy.deepcopy(entry), copy.deepcopy(entry)))

    return mutate


class CargoSourceInventoryTests(unittest.TestCase):
    def _errors_after(self, mutation: Mutation) -> list[str]:
        with tempfile.TemporaryDirectory() as tmp:
            app = _fixture(tmp)
            baseline_errors: list[str] = []
            check_dependency_preflight(app, baseline_errors)
            self.assertEqual(baseline_errors, [], "cargo source fixture baseline must be clean")

            path = app / "build-aux/cargo/cargo-sources.json"
            sources = json.loads(path.read_text(encoding="utf-8"))
            mutation(sources)
            path.write_text(json.dumps(sources) + "\n", encoding="utf-8")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        return errors

    def _assert_rejected(self, mutation: Mutation, diagnostic: str) -> None:
        errors = self._errors_after(mutation)
        matching = [error for error in errors if diagnostic in error]
        self.assertTrue(matching, f"missing diagnostic {diagnostic!r}: {errors!r}")

    def test_real_inventory_and_reviewed_local_patches_match_contract(self) -> None:
        sources = json.loads(
            (REPO_ROOT / "app/build-aux/cargo/cargo-sources.json").read_text(encoding="utf-8")
        )
        self.assertTrue(all(isinstance(source, dict) for source in sources))
        lock = tomllib.loads((REPO_ROOT / "app/Cargo.lock").read_text(encoding="utf-8"))
        registry = [
            package
            for package in lock["package"]
            if str(package.get("source", "")).startswith(("registry+", "sparse+"))
        ]
        archives = [source for source in sources if source.get("type") == "archive"]
        checksums = [
            source
            for source in sources
            if source.get("type") == "inline"
            and source.get("dest-filename") == ".cargo-checksum.json"
        ]
        configs = [
            source
            for source in sources
            if source.get("type") == "inline" and source.get("dest-filename") == "config"
        ]
        expected_destinations = {
            f"cargo/vendor/{package['name']}-{package['version']}" for package in registry
        }

        self.assertEqual(len(archives), len(registry))
        self.assertEqual(len(checksums), len(registry))
        self.assertEqual(len(configs), 1)
        self.assertEqual(len(sources), len(archives) + len(checksums) + len(configs))
        self.assertEqual({source["dest"] for source in archives}, expected_destinations)
        self.assertEqual({source["dest"] for source in checksums}, expected_destinations)
        self.assertTrue(
            all(
                set(source) == {"type", "archive-type", "url", "sha256", "dest"}
                for source in archives
            )
        )
        self.assertTrue(
            all(
                set(source) == {"type", "contents", "dest", "dest-filename"}
                for source in checksums
            )
        )
        self.assertEqual(configs, [VENDOR_CONFIG])

        manifest = tomllib.loads((REPO_ROOT / "app/Cargo.toml").read_text(encoding="utf-8"))
        patches = manifest["patch"]["crates-io"]
        release_policy = json.loads(
            (REPO_ROOT / "policy/release.policy.json").read_text(encoding="utf-8")
        )
        reviewed_patches = {
            entry["crate"]: entry
            for entry in release_policy["local_patch_policy"]["release_critical_local_patches"]
        }
        self.assertEqual(set(patches), set(reviewed_patches))
        for crate, entry in patches.items():
            with self.subTest(crate=crate):
                patch_dir = REPO_ROOT / "app" / entry["path"]
                reviewed = reviewed_patches[crate]
                self.assertEqual(patch_dir, REPO_ROOT / reviewed["path"])
                patch_manifest_path = REPO_ROOT / reviewed["manifest"]
                self.assertEqual(patch_manifest_path.parent, patch_dir)
                patch_manifest = json.loads(
                    patch_manifest_path.read_text(encoding="utf-8")
                )
                anchor = patch_dir / patch_manifest["upstream_source"]["crate_archive"]
                self.assertEqual(patch_manifest["crate"], crate)
                self.assertTrue(anchor.is_file())

    def test_unknown_kinds_and_non_object_entries_are_rejected(self) -> None:
        cases = [
            ({"type": "shell", "commands": ["false"]}, "has unapproved type or fields"),
            ({}, "has unapproved type or fields"),
            ("cargo-sources-extra.json", "must be a generated source object"),
            ([], "must be a generated source object"),
            (None, "must be a generated source object"),
        ]
        for entry, diagnostic in cases:
            with self.subTest(entry=entry):
                self._assert_rejected(_append(entry), diagnostic)

    def test_extra_materialization_options_are_rejected(self) -> None:
        cases = [
            (0, "commands", ["false"]),
            (0, "only-arches", ["not-this-arch"]),
            (0, "strip-components", 0),
            (1, "commands", ["false"]),
        ]
        for index, key, value in cases:
            with self.subTest(index=index, key=key):
                self._assert_rejected(
                    _set(index, key, value), "has unapproved type or fields"
                )

        config = {**VENDOR_CONFIG, "commands": ["false"]}
        self._assert_rejected(_append(config), "has unapproved type or fields")

    def test_foreign_and_orphan_destinations_are_rejected(self) -> None:
        foreign_archive = {
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": "https://example.invalid/custom.tar.gz",
            "sha256": "e" * 64,
            "dest": "cargo/vendor/custom-1.0.0",
        }
        orphan_checksum = {
            "type": "inline",
            "contents": "{}",
            "dest": "cargo/vendor/orphan-1.0.0",
            "dest-filename": ".cargo-checksum.json",
        }
        self._assert_rejected(
            _append(foreign_archive), "archive destination is absent from Cargo.lock"
        )
        self._assert_rejected(
            _append(orphan_checksum),
            "inline source is not an expected checksum or vendor config",
        )

    def test_archive_url_type_and_hash_must_match_lock(self) -> None:
        cases = [
            (
                "url",
                "https://example.invalid/gtk4.crate",
                "missing Flatpak cargo archive source for gtk4",
            ),
            (
                "url",
                "https://static.crates.io/crates/gtk4/wrong.crate",
                "url 'https://static.crates.io/crates/gtk4/wrong.crate' must match locked value",
            ),
            ("archive-type", "zip", "missing Flatpak cargo archive source for gtk4"),
            (
                "sha256",
                "0" * 64,
                "sha256 '0000000000000000000000000000000000000000000000000000000000000000' "
                "must match locked value",
            ),
        ]
        for key, value, diagnostic in cases:
            with self.subTest(key=key, value=value):
                self._assert_rejected(_set(0, key, value), diagnostic)

    def test_destination_and_filename_types_must_be_strings(self) -> None:
        cases = [
            (0, "dest", []),
            (1, "dest", []),
            (1, "dest-filename", []),
            (0, "url", None),
        ]
        for index, key, value in cases:
            with self.subTest(index=index, key=key):
                self._assert_rejected(_set(index, key, value), "source fields must be strings")

        for key in ("dest", "dest-filename"):
            with self.subTest(config_key=key):
                config = copy.deepcopy(VENDOR_CONFIG)
                config[key] = []
                self._assert_rejected(_append(config), "source fields must be strings")

    def test_paths_cannot_traverse_or_inject_cargo_config(self) -> None:
        cases = [
            (
                _set(0, "dest", "cargo/vendor/../../escape"),
                "archive destination is absent from Cargo.lock",
            ),
            (
                _set(1, "dest", "cargo/vendor/../../escape"),
                "inline source is not an expected checksum or vendor config",
            ),
            (
                _set(1, "dest-filename", "../../config"),
                "inline source is not an expected checksum or vendor config",
            ),
            (
                _append({**VENDOR_CONFIG, "dest": ".cargo", "dest-filename": "config.toml"}),
                "inline source is not an expected checksum or vendor config",
            ),
            (
                _append({**VENDOR_CONFIG, "dest-filename": "../config"}),
                "inline source is not an expected checksum or vendor config",
            ),
        ]
        for mutation, diagnostic in cases:
            with self.subTest(diagnostic=diagnostic):
                self._assert_rejected(mutation, diagnostic)

    def test_duplicate_archive_checksum_and_config_entries_are_rejected(self) -> None:
        mutations: list[tuple[str, Mutation]] = [
            ("archive", lambda sources: sources.append(copy.deepcopy(sources[0]))),
            ("checksum", lambda sources: sources.append(copy.deepcopy(sources[1]))),
            ("config", _append_twice(VENDOR_CONFIG)),
        ]
        for kind, mutation in mutations:
            with self.subTest(kind=kind):
                self._assert_rejected(mutation, "duplicate Flatpak cargo source entry")

    def test_only_the_exact_reviewed_vendor_config_is_accepted(self) -> None:
        self.assertEqual(self._errors_after(_append(VENDOR_CONFIG)), [])
        rejected_contents = [
            '[source.crates-io]\nregistry = "https://example.invalid"\n',
            (
                '[source.vendored-sources]\ndirectory = "cargo/vendor"\n'
                '[source.crates-io]\nreplace-with = "vendored-sources"\n'
                "[net]\ngit-fetch-with-cli = true\n"
            ),
            "this is not toml = [",
        ]
        for contents in rejected_contents:
            diagnostic = (
                "vendor config must be valid TOML"
                if contents == "this is not toml = ["
                else "vendor config must exactly match the reviewed policy"
            )
            with self.subTest(contents=contents):
                self._assert_rejected(
                    _append({**VENDOR_CONFIG, "contents": contents}), diagnostic
                )

    def test_checksum_payload_shape_and_locked_value_are_validated(self) -> None:
        cases = [
            ("not-json", "has invalid JSON"),
            (json.dumps([]), "checksum payload for gtk4 0.11.4 must be a JSON object"),
            (
                json.dumps({"package": "0" * 64, "files": {}}),
                "checksum payload must match locked checksum",
            ),
            (
                json.dumps({"package": CHECKSUM_A, "files": []}),
                "checksum payload files field",
            ),
            (json.dumps({"package": CHECKSUM_A}), "checksum payload files field"),
        ]
        for contents, diagnostic in cases:
            with self.subTest(contents=contents):
                self._assert_rejected(_set(1, "contents", contents), diagnostic)

        populated = json.dumps(
            {"package": CHECKSUM_A, "files": {"src/lib.rs": CHECKSUM_B}}
        )
        self.assertEqual(self._errors_after(_set(1, "contents", populated)), [])


if __name__ == "__main__":
    unittest.main()
