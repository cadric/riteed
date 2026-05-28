from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from tools.checks.dependency_preflight import check_dependency_preflight

REPO_ROOT = Path(__file__).resolve().parents[2]
CHECKSUM_A = "a" * 64
CHECKSUM_B = "b" * 64
CHECKSUM_C = "c" * 64
CHECKSUM_D = "d" * 64
CHECKSUM_E = "e" * 64
CHECKSUM_F = "f" * 64
LEGACY_SOURCE = "registry+https://github.com/rust-lang/crates.io-index"
SPARSE_SOURCE = "sparse+https://index.crates.io/"
GTK4_TARGET = "0.11.3"


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def _package(name: str, version: str, checksum: str, *, source: str = LEGACY_SOURCE) -> str:
    return f"""
[[package]]
name = "{name}"
version = "{version}"
source = "{source}"
checksum = "{checksum}"
"""


def _lock(
    version: str,
    *,
    gtk4_version: str = GTK4_TARGET,
    gtk4_sys_version: str = GTK4_TARGET,
    include_libadwaita: bool = True,
    extra_packages: str = "",
) -> str:
    packages = [
        _package("gtk4", gtk4_version, CHECKSUM_A),
        _package("gtk4-sys", gtk4_sys_version, CHECKSUM_B),
    ]
    dependencies = [' "gtk4",']
    if include_libadwaita:
        packages.extend(
            [
                _package("libadwaita", "0.9.1", CHECKSUM_C),
                _package("libadwaita-sys", "0.9.1", CHECKSUM_D),
            ]
        )
        dependencies.append(' "libadwaita",')
    return f"""version = 4

{''.join(packages)}
{extra_packages}

[[package]]
name = "riteed"
version = "{version}"
dependencies = [
{chr(10).join(dependencies)}
]
"""


def _source(name: str, version: str, checksum: str) -> list[dict[str, object]]:
    dest = f"cargo/vendor/{name}-{version}"
    return [
        {
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": checksum,
            "dest": dest,
        },
        {
            "type": "inline",
            "contents": json.dumps({"package": checksum, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        },
    ]


def _fixture(tmpdir: str, *, app_version: str = "1.2.3") -> Path:
    root = Path(tmpdir)
    shutil.copytree(REPO_ROOT / "policy", root / "policy")
    app = root / "app"
    _write(
        app / "Cargo.toml",
        f"""[package]
name = "riteed"
version = "{app_version}"
edition = "2024"

[dependencies]
gtk4 = {{ version = "={GTK4_TARGET}" }}
libadwaita = {{ version = "=0.9.1" }}
""",
    )
    _write(app / "Cargo.lock", _lock(app_version))
    _write(app / "fuzz" / "Cargo.lock", _lock(app_version))
    sources = [
        *_source("gtk4", GTK4_TARGET, CHECKSUM_A),
        *_source("gtk4-sys", GTK4_TARGET, CHECKSUM_B),
        *_source("libadwaita", "0.9.1", CHECKSUM_C),
        *_source("libadwaita-sys", "0.9.1", CHECKSUM_D),
    ]
    _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
    return app


class DependencyPreflightTests(unittest.TestCase):
    def test_current_app_dependency_preflight_passes(self) -> None:
        errors: list[str] = []
        check_dependency_preflight(REPO_ROOT / "app", errors)
        self.assertEqual(errors, [])

    def test_non_riteed_app_is_not_forced_into_riteed_dependency_contract(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            app = root / "demo"
            _write(
                app / "Cargo.toml",
                """[package]
name = "demo"
version = "0.1.0"
edition = "2024"
""",
            )
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertEqual(errors, [])

    def test_cli_entrypoint_passes_current_app(self) -> None:
        result = subprocess.run(
            ["python3", "-m", "tools.checks.dependency_preflight", "--root", "app"],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("[dependency-preflight] OK", result.stdout)

    def test_fuzz_lock_must_match_app_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(app / "fuzz" / "Cargo.lock", _lock("9.9.9"))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("fuzz/Cargo.lock riteed version" in item for item in errors))

    def test_direct_binding_lock_must_match_policy_target(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(app / "Cargo.lock", _lock("1.2.3", gtk4_version="0.11.4"))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("Cargo.lock gtk4 version '0.11.4'" in item for item in errors))

    def test_safe_sys_pairs_must_match_exact_patch_version(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(app / "Cargo.lock", _lock("1.2.3", gtk4_version="0.11.4"))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("gtk4 '0.11.4' must exactly match gtk4-sys '0.11.3'" in item for item in errors))

    def test_duplicate_safe_sys_packages_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            extra = _package("gtk4", "0.10.0", CHECKSUM_E) + _package("gtk4-sys", "0.10.0", CHECKSUM_F)
            _write(app / "Cargo.lock", _lock("1.2.3", extra_packages=extra))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("must contain exactly one gtk4 and one gtk4-sys" in item for item in errors))

    def test_stack_dependencies_must_be_direct_exact_pins(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(
                app / "Cargo.toml",
                """[package]
name = "riteed"
version = "1.2.3"
edition = "2024"

[dependencies]
gtk4 = { version = "0.11.3" }
libadwaita = { version = "=0.9.1" }
sourceview5 = { version = "0.11.0" }

[build-dependencies]
glib-build-tools = "0.22.0"
""",
            )
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("dependencies.gtk4 must be a direct exact pin" in item for item in errors))
        self.assertTrue(any("dependencies.sourceview5 must be a direct exact pin" in item for item in errors))
        self.assertTrue(any("build-dependencies.glib-build-tools must be a direct exact pin" in item for item in errors))

    def test_target_only_stack_dependencies_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            cargo_toml = (app / "Cargo.toml").read_text(encoding="utf-8")
            _write(
                app / "Cargo.toml",
                cargo_toml
                + """
[target.'cfg(unix)'.dependencies]
sourceview5 = { version = "=0.11.0" }
""",
            )
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("target.cfg(unix).dependencies.sourceview5" in item for item in errors))

    def test_fuzz_lock_may_omit_app_only_stack_crates(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(app / "fuzz" / "Cargo.lock", _lock("1.2.3", include_libadwaita=False))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertEqual(errors, [])

    def test_fuzz_only_stack_crates_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            extra = _package("gdk4", "0.11.2", CHECKSUM_E) + _package("gdk4-sys", "0.11.2", CHECKSUM_F)
            _write(app / "fuzz" / "Cargo.lock", _lock("1.2.3", extra_packages=extra))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("fuzz/Cargo.lock contains gdk4" in item for item in errors))

    def test_fuzz_stack_crates_must_match_app_when_present_in_both(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            app_extra = _package("glib", "0.22.5", CHECKSUM_E)
            fuzz_extra = _package("glib", "0.22.7", CHECKSUM_E)
            _write(app / "Cargo.lock", _lock("1.2.3", extra_packages=app_extra))
            _write(app / "fuzz" / "Cargo.lock", _lock("1.2.3", extra_packages=fuzz_extra))
            sources = [
                *_source("gtk4", GTK4_TARGET, CHECKSUM_A),
                *_source("gtk4-sys", GTK4_TARGET, CHECKSUM_B),
                *_source("libadwaita", "0.9.1", CHECKSUM_C),
                *_source("libadwaita-sys", "0.9.1", CHECKSUM_D),
                *_source("glib", "0.22.5", CHECKSUM_E),
            ]
            _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("fuzz/Cargo.lock glib '0.22.7' must match Cargo.lock glib '0.22.5'" in item for item in errors))

    def test_flatpak_cargo_sources_must_match_lockfile(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            sources = _source("gtk4", GTK4_TARGET, CHECKSUM_A)
            _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("missing Flatpak cargo archive source for gtk4-sys" in item for item in errors))
        self.assertTrue(any("Regenerate app/build-aux/cargo/cargo-sources.json" in item for item in errors))

    def test_policy_json_wrong_shape_reports_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(app.parent / "policy" / "gtk4-rs.policy.json", "null\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("gtk4-rs.policy.json must contain a JSON object" in item for item in errors))

    def test_policy_targets_wrong_shape_reports_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            _write(app.parent / "policy" / "gtk4-rs.policy.json", json.dumps({"targets": None}))
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("policy targets.crate must be an object" in item for item in errors))

    def test_missing_lockfile_does_not_cascade_none_version_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            (app / "Cargo.lock").unlink()
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("missing TOML file" in item and "Cargo.lock" in item for item in errors))
        self.assertFalse(any("None" in item for item in errors), errors)

    def test_missing_cargo_sources_reports_one_root_cause(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            (app / "build-aux" / "cargo" / "cargo-sources.json").unlink()
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        matching = [item for item in errors if "cargo-sources.json" in item]
        self.assertEqual(len(matching), 1, errors)
        self.assertIn("missing JSON file", matching[0])

    def test_sparse_crates_io_lock_sources_are_supported(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            lock = (app / "Cargo.lock").read_text(encoding="utf-8").replace(LEGACY_SOURCE, SPARSE_SOURCE)
            _write(app / "Cargo.lock", lock)
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertEqual(errors, [])

    def test_duplicate_cargo_source_dest_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            sources = json.loads((app / "build-aux" / "cargo" / "cargo-sources.json").read_text(encoding="utf-8"))
            sources.append(dict(sources[0]))
            _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertTrue(any("duplicate Flatpak cargo source entry" in item for item in errors))

    def test_cargo_config_inline_entry_does_not_collide_with_checksums(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            sources = json.loads((app / "build-aux" / "cargo" / "cargo-sources.json").read_text(encoding="utf-8"))
            sources.insert(
                0,
                {
                    "type": "inline",
                    "contents": "[source.vendored-sources]\n",
                    "dest": "cargo",
                    "dest-filename": "config.toml",
                },
            )
            _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertEqual(errors, [])

    def test_checksum_payload_allows_populated_files_map(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            sources = json.loads((app / "build-aux" / "cargo" / "cargo-sources.json").read_text(encoding="utf-8"))
            sources[1]["contents"] = json.dumps(
                {"package": CHECKSUM_A, "files": {"src/lib.rs": CHECKSUM_B}}
            )
            _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertEqual(errors, [])

    def test_non_crates_io_archive_source_is_not_stale(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            app = _fixture(tmpdir)
            sources = json.loads((app / "build-aux" / "cargo" / "cargo-sources.json").read_text(encoding="utf-8"))
            sources.append(
                {
                    "type": "archive",
                    "archive-type": "tar-gzip",
                    "url": "https://example.invalid/custom.tar.gz",
                    "sha256": CHECKSUM_E,
                    "dest": "cargo/vendor/custom-1.0.0",
                }
            )
            _write(app / "build-aux" / "cargo" / "cargo-sources.json", json.dumps(sources) + "\n")
            errors: list[str] = []
            check_dependency_preflight(app, errors)
        self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
