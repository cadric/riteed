from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from tools.checks import foundation, hig, libadwaita, runtime
from tools.scanners.rust import runtime_review_hits
from tools.scanners.sites import ReviewEntry, ScanHit, validate_review_links
from tools.validation_tooling import contract_root, repo_root, run_checked


REPO_ROOT = Path(__file__).resolve().parents[2]


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


class PolicyCheckTests(unittest.TestCase):
    def test_module_help_entrypoint(self) -> None:
        result = subprocess.run(
            ["python3", "-m", "tools.policy_check", "--help"],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("--update-artifact-index", result.stdout)

    def test_script_help_entrypoint(self) -> None:
        result = subprocess.run(
            ["python3", "tools/policy_check.py", "--help"],
            cwd=REPO_ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0)
        self.assertIn("--update-artifact-index", result.stdout)

    def test_orphaned_script_fails_clearly(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            script = Path(tmpdir) / "policy_check.py"
            script.write_text((REPO_ROOT / "tools" / "policy_check.py").read_text(encoding="utf-8"), encoding="utf-8")
            result = subprocess.run(
                ["python3", str(script), "--help"],
                cwd=tmpdir,
                text=True,
                capture_output=True,
                check=False,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("failed to resolve the tools package", result.stderr)

    def test_review_link_validation_reports_missing_and_stale(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(root / "src" / "main.rs", "fn main() {\n    println!(\"hi\");\n}\n")
            hits = [ScanHit(path="src/main.rs", line=2, kind="runtime-site", match="println!", message="review me")]
            entries = [ReviewEntry(path="src/main.rs", line=1, kind="runtime-site", match="fn main", source_file="artifact.json", payload={})]
            errors: list[str] = []
            validate_review_links(root, hits, entries, errors)
            self.assertTrue(any("missing review entry" in item for item in errors))
            self.assertTrue(any("stale review entry" in item for item in errors))

    def test_update_artifact_index_refreshes_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "AGENTS.md", "# test\n")
            rust_path = root / "policy" / "rust.policy.json"
            rust_data = json.loads(rust_path.read_text(encoding="utf-8"))
            rust_data["title"] = "Modified"
            rust_path.write_text(json.dumps(rust_data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            foundation.check_policy_stack(root, errors)
            self.assertTrue(any("artifact_index is out of date" in item for item in errors))
            foundation.update_artifact_index(root)
            errors = []
            foundation.check_policy_stack(root, errors)
            self.assertEqual(errors, [])

    def test_repo_root_accepts_embedded_app_subtree(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            shutil.copytree(REPO_ROOT / "tools", root / "tools")
            _write(root / "AGENTS.md", "# root\n")
            app = root / "app"
            _write(app / "Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
            _write(app / "Cargo.lock", "")
            _write(app / "rust-toolchain.toml", "[toolchain]\nchannel = \"1.95.0\"\n")
            _write(app / "src" / "main.rs", "fn main() {}\n")
            _write(app / "data" / "demo.desktop", "[Desktop Entry]\nName=Demo\n")
            _write(app / "po" / "demo.po", "")
            _write(app / "build-aux" / "demo.yml", "id: io.example.Demo\nruntime: org.gnome.Platform\nruntime-version: '50'\nsdk: org.gnome.Sdk\ncommand: demo\n")
            detected = repo_root(str(app))
            self.assertEqual(detected, app.resolve())
            self.assertEqual(contract_root(detected), root.resolve())

    def test_check_repo_layout_allows_embedded_app_contract_from_ancestor(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            shutil.copytree(REPO_ROOT / "tools", root / "tools")
            _write(root / "AGENTS.md", "# root\n")
            app = root / "app"
            _write(app / "Cargo.toml", "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
            _write(app / "Cargo.lock", "")
            _write(app / "rust-toolchain.toml", "[toolchain]\nchannel = \"1.95.0\"\ncomponents = [\"rustfmt\", \"clippy\"]\n")
            _write(app / "src" / "main.rs", "fn main() {}\n")
            _write(app / "data" / "demo.desktop", "[Desktop Entry]\nName=Demo\n")
            _write(app / "po" / "demo.po", "")
            _write(app / "build-aux" / "demo.yml", "id: io.example.Demo\nruntime: org.gnome.Platform\nruntime-version: '50'\nsdk: org.gnome.Sdk\ncommand: demo\n")
            errors: list[str] = []
            foundation.check_repo_layout(app, errors)
            self.assertEqual(errors, [])

    def test_libadwaita_requires_surface_review(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "data" / "ui" / "main.ui", '<object class="AdwApplicationWindow" id="win">\n</object>\n')
            errors: list[str] = []
            libadwaita.check_libadwaita(root, errors)
            self.assertTrue(any("missing review entry" in item for item in errors))

    def test_libadwaita_requires_css_review_under_data_ui(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "data" / "ui" / "app.css", ".demo-specific {\n  margin: 6px;\n}\n")
            errors: list[str] = []
            libadwaita.check_libadwaita(root, errors)
            self.assertTrue(any("css-file" in item and "data/ui/app.css" in item for item in errors))

    def test_resources_require_data_ui_css_entries(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(root / "data" / "ui" / "app.css", ".demo-specific {\n  margin: 6px;\n}\n")
            _write(
                root / "data" / "resources.gresource.xml",
                """<?xml version="1.0" encoding="UTF-8"?>
<gresources>
  <gresource prefix="/io/example/Demo">
  </gresource>
</gresources>
""",
            )
            errors: list[str] = []
            foundation.check_resources(root, "io.example.Demo", errors)
            self.assertTrue(any("ui/app.css" in item for item in errors))

    def test_runtime_detached_task_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "main.rs", "fn main() {\n    std::thread::spawn(|| println!(\"x\"));\n}\n")
            errors: list[str] = []
            runtime.check_runtime(root, errors)
            self.assertTrue(any("owned or supervised" in item for item in errors))

    def test_runtime_sync_fs_requires_review(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "main.rs", "fn main() {\n    let _exists = path.exists();\n}\n")
            errors: list[str] = []
            runtime.check_runtime(root, errors)
            self.assertTrue(any("runtime-sync-fs" in item for item in errors))

    def test_runtime_sync_fs_review_artifact_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "main.rs", "fn main() {\n    let _exists = path.exists();\n}\n")
            _write(
                root / "build-aux" / "validation" / "runtime-review-sync-fs.v1.json",
                json.dumps(
                    {
                        "version": 1,
                        "sites": [
                            {
                                "path": "src/main.rs",
                                "line": 2,
                                "match": "let _exists = path.exists();",
                                "kind": "runtime-sync-fs",
                                "ownership": "A native-only reviewed runtime path owns this probe.",
                                "justification": "The path is guarded away from portal and other FUSE roots.",
                            }
                        ],
                    }
                )
                + "\n",
            )
            errors: list[str] = []
            runtime.check_runtime(root, errors)
            self.assertEqual(errors, [])

    def test_runtime_sync_fs_ignores_test_only_sites(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "tests.rs", "fn test_probe() {\n    let _exists = path.exists();\n}\n")
            _write(
                root / "src" / "main.rs",
                "#[cfg(test)]\nmod tests {\n    fn probe() {\n        let _exists = path.exists();\n    }\n}\n",
            )
            errors: list[str] = []
            runtime.check_runtime(root, errors)
            self.assertFalse(any("runtime-sync-fs" in item for item in errors))

    def test_git_subprocess_exception_is_path_scoped(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "git_process.rs", "fn ok() { let _flags = gio::SubprocessFlags::STDOUT_PIPE; }\n")
            _write(root / "src" / "other.rs", "fn bad() { let _flags = gio::SubprocessFlags::STDOUT_PIPE; }\n")
            errors: list[str] = []
            foundation.check_forbidden_patterns(root, errors)
            self.assertTrue(any("Gio subprocess outside" in item and "src/other.rs" in item for item in errors))
            self.assertFalse(any("src/git_process.rs" in item for item in errors))

    def test_validation_color_scheme_exception_is_path_scoped(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "settings" / "appearance.rs", "fn ok() { adw::ColorScheme::ForceLight; }\n")
            _write(root / "src" / "other.rs", "fn bad() { adw::ColorScheme::ForceDark; }\n")
            errors: list[str] = []
            foundation.check_forbidden_patterns(root, errors)
            self.assertTrue(any("custom color scheme" in item and "src/other.rs" in item for item in errors))
            self.assertFalse(any("src/settings/appearance.rs" in item for item in errors))

    def test_libadwaita_color_scheme_exception_is_path_scoped(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "settings" / "appearance.rs", "fn ok() { adw::ColorScheme::ForceLight; }\n")
            _write(root / "src" / "other.rs", "fn bad() { adw::ColorScheme::ForceDark; }\n")
            errors: list[str] = []
            libadwaita.check_libadwaita(root, errors)
            self.assertTrue(any("custom color scheme" in item and "src/other.rs" in item for item in errors))
            self.assertFalse(any("src/settings/appearance.rs" in item for item in errors))

    def test_flatpak_spawn_and_std_command_have_no_git_process_exception(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(
                root / "src" / "git_process.rs",
                "fn bad() {\n    let _cmd = std::process::Command::new(\"git\");\n    let _name = \"flatpak-spawn\";\n}\n",
            )
            errors: list[str] = []
            foundation.check_forbidden_patterns(root, errors)
            self.assertTrue(any("external commands" in item for item in errors))
            self.assertTrue(any("flatpak-spawn" in item for item in errors))

    def test_runtime_review_patterns_respect_paths(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(root / "src" / "git_process.rs", "fn ok() { let _flags = gio::SubprocessFlags::STDOUT_PIPE; }\n")
            _write(root / "src" / "other.rs", "fn ignored() { let _flags = gio::SubprocessFlags::STDOUT_PIPE; }\n")
            hits = runtime_review_hits(
                root,
                [
                    {
                        "kind": "runtime-git-subprocess",
                        "message": "review",
                        "pattern": r"\bgio::SubprocessFlags\b",
                        "paths": ["src/git_process.rs"],
                    }
                ],
            )
            self.assertEqual([hit.path for hit in hits], ["src/git_process.rs"])

    def test_artifact_index_message_includes_remediation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "AGENTS.md", "# test\n")
            rust_path = root / "policy" / "rust.policy.json"
            rust_path.write_text(rust_path.read_text(encoding="utf-8") + "\n", encoding="utf-8")
            errors: list[str] = []
            foundation.check_policy_stack(root, errors)
            self.assertTrue(any("--update-artifact-index" in item for item in errors))


    def test_ffi_regex_matches_extern_c(self) -> None:
        pattern = next(
            rule["pattern"]
            for rule in foundation.validation_policy(REPO_ROOT)["forbidden_source_patterns"]
            if rule["message"] == "Rust source must not declare C FFI."
        )
        self.assertTrue(re.search(pattern, 'extern "C" {'))

    def test_crate_root_patterns_allow_combined_lints(self) -> None:
        patterns = foundation.validation_policy(REPO_ROOT)["required_source_patterns"][:2]
        self.assertTrue(re.search(patterns[0]["pattern"], '#![forbid(unsafe_code, clippy::unwrap_used)]'))
        self.assertTrue(re.search(patterns[1]["pattern"], '#![deny(unsafe_op_in_unsafe_fn, warnings)]'))

    def test_gettext_catalogs_are_only_exempt_from_line_limits(self) -> None:
        policy = foundation.validation_policy(REPO_ROOT)
        self.assertNotIn("po/*.po", policy["line_limit_globs"])
        self.assertNotIn("po/*.pot", policy["line_limit_globs"])
        self.assertIn("po/**", policy["applies_to"])
        self.assertTrue(
            any(rule.get("when_glob") == "po/*.po" for rule in policy["conditional_validators"])
        )

    def test_gettext_system_feature_is_required_by_manifest_policy(self) -> None:
        cases = [
            ({}, True),
            ({"features": []}, True),
            ({"features": ["other-feature"]}, True),
            ({"features": ["gettext-system"]}, False),
        ]
        for gettext_extra, should_error in cases:
            with self.subTest(gettext_extra=gettext_extra):
                errors = self._manifest_errors_for_gettext_dep(gettext_extra)
                found = any("gettext-system" in item for item in errors)
                self.assertEqual(found, should_error)

    def _manifest_errors_for_gettext_dep(self, gettext_extra: dict[str, object]) -> list[str]:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(
                root / "Cargo.toml",
                '[package]\nname = "demo"\nversion = "0.1.0"\nedition = "2024"\n\n[lints.rust]\nwarnings = "deny"\n',
            )
            deps = [
                {"name": "gettext-rs", "kind": None, **gettext_extra},
                {"name": "gtk4", "kind": None, "features": []},
                {"name": "libadwaita", "kind": None, "features": []},
            ]
            packages = [
                {
                    "name": "demo",
                    "edition": "2024",
                    "manifest_path": str(root / "Cargo.toml"),
                    "dependencies": deps,
                }
            ]
            with patch.object(foundation, "cargo_packages", return_value=packages):
                with patch.object(
                    foundation,
                    "validation_policy",
                    return_value={
                        "dependency_policy": {
                            "required_runtime_crates": ["gtk4", "libadwaita", "gettext-rs"],
                            "forbidden_crates": [],
                        }
                    },
                ):
                    with patch.object(
                        foundation,
                        "rust_policy",
                        return_value={"lint_baseline": {"rust_lints_required": {}, "clippy_lints_required": {}}},
                    ):
                        with patch.object(
                            foundation,
                            "gettext_policy",
                            return_value={
                                "linking_and_distribution": {
                                    "gettext_system_feature_required_on_linux_and_flatpak_targets": True
                                }
                            },
                        ):
                            errors: list[str] = []
                            foundation.check_manifests(root, errors)
                            return errors

    def test_find_flatpak_manifest_ignores_non_manifest_json(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "build-aux" / "artifact-index.json", '{\"id\":\"not.a.manifest\"}\n')
            self.assertIsNone(foundation.find_flatpak_manifest(root))

    def test_hig_menu_allows_twelve_items_but_forbids_quit(self) -> None:
        entry = ReviewEntry(
            path="data/ui/main.ui",
            line=1,
            kind="menu",
            match="<menu id=\"primary-menu\">",
            source_file="build-aux/validation/ui-review.json",
            payload={"items": 12, "standard_items": ["about", "preferences", "help", "quit"]},
        )
        errors: list[str] = []
        hig.check_hig(REPO_ROOT, errors, [entry])
        self.assertTrue(any("forbidden standard items" in item for item in errors))
        self.assertFalse(any("exceeds max items" in item for item in errors))

    def test_gsettings_enum_keys_are_valid_schema_keys(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(
                root / "data" / "schemas" / "demo.gschema.xml",
                """<schemalist>
  <enum id="demo.Color">
    <value nick="system" value="0"/>
  </enum>
  <schema id="demo.App" path="/demo/App/">
    <key name="color" enum="demo.Color">
      <default>'system'</default>
      <summary>Color</summary>
      <description>Color preference.</description>
    </key>
  </schema>
</schemalist>
""",
            )
            errors: list[str] = []
            foundation.check_ui_localization(root, errors)
            self.assertEqual(errors, [])

    def test_xgettext_completeness_uses_normalized_sets(self) -> None:
        from tools.checks import commands

        with patch.object(commands, "_xgettext_messages", return_value={(None, "Hello", None)}):
            with patch("tools.checks.commands.normalized_pot_messages", return_value={(None, "Hello", None)}):
                errors: list[str] = []
                commands.check_xgettext_completeness(REPO_ROOT, errors)
                self.assertEqual(errors, [])

    def test_xgettext_completeness_sorts_mixed_contexts(self) -> None:
        from tools.checks import commands

        generated = {(None, "Plain", None), ("menu item", "Open", None)}
        with patch.object(commands, "_xgettext_messages", return_value=generated):
            with patch("tools.checks.commands.normalized_pot_messages", return_value=set()):
                errors: list[str] = []
                commands.check_xgettext_completeness(REPO_ROOT, errors)
                self.assertTrue(errors)

    def test_rust_extraction_falls_back_to_xtr_when_xgettext_lacks_rust(self) -> None:
        from tools.checks import commands

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            rust_path = root / "src" / "lib.rs"
            _write(rust_path, 'fn demo() { gettext("Hello"); }\n')
            with patch.object(commands, "rust_files", return_value=[rust_path]):
                with patch.object(commands, "scoped_files", return_value=[]):
                    with patch.object(
                        commands,
                        "run_capture",
                        return_value=subprocess.CompletedProcess(["xgettext", "--help"], 0, "C\n", ""),
                    ):
                        with patch.object(commands, "require_tool") as require_tool:
                            with patch.object(commands, "run_checked") as run_checked:
                                run_checked.side_effect = lambda cmd, root, label: Path(
                                    cmd[cmd.index("--output") + 1]
                                ).write_text("", encoding="utf-8")
                                with patch.object(commands, "message_keys", return_value={(None, "Hello", None)}):
                                    self.assertEqual(commands._xgettext_messages(root), {(None, "Hello", None)})

        require_tool.assert_called_once_with("xtr")
        self.assertEqual(run_checked.call_args.args[0][0], "xtr")
        self.assertIn("gettext", run_checked.call_args.args[0])

    def test_metainfo_messages_respect_translate_no(self) -> None:
        from tools.checks import commands

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            (root / "data").mkdir()
            _write(
                root / "data" / "demo.metainfo.xml",
                """<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>org.example.Demo</id>
  <name>cadric</name>
  <summary translate="yes">Visible summary</summary>
  <description>
    <p translate="">Visible paragraph</p>
    <p translate="no">Hidden paragraph</p>
  </description>
  <developer>
    <name translate="no">Hidden developer</name>
  </developer>
</component>
""",
            )

            messages = commands._metainfo_messages(root)

        self.assertIn((None, "cadric", None), messages)
        self.assertIn((None, "Visible summary", None), messages)
        self.assertIn((None, "Visible paragraph", None), messages)
        self.assertNotIn((None, "Hidden paragraph", None), messages)
        self.assertNotIn((None, "Hidden developer", None), messages)

    def test_required_commands_use_headless_gtk_environment(self) -> None:
        from tools.checks import commands

        captured: dict[str, object] = {}

        def fake_run_checked(
            cmd: list[str],
            cwd: Path,
            label: str | None = None,
            env: dict[str, str] | None = None,
        ) -> str:
            captured["cmd"] = cmd
            captured["cwd"] = cwd
            captured["label"] = label
            captured["env"] = env
            return ""

        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            with patch.object(
                commands,
                "validation_policy",
                return_value={
                    "required_tools": [],
                    "required_commands": ["cargo test"],
                    "conditional_validators": [],
                },
            ):
                with patch.object(commands, "run_checked", side_effect=fake_run_checked):
                    with patch.object(commands, "check_xgettext_completeness"):
                        errors: list[str] = []
                        commands.run_required_commands(root, errors)

        self.assertEqual(captured["cmd"], ["cargo", "test"])
        self.assertEqual(captured["label"], "cargo test")
        self.assertEqual(
            captured["env"],
            {
                "GSK_RENDERER": os.environ.get("GSK_RENDERER", "cairo"),
                "GTK_A11Y": os.environ.get("GTK_A11Y", "none"),
            },
        )

    def test_run_checked_reports_stdout_and_stderr_on_failure(self) -> None:
        stderr = StringIO()
        with redirect_stderr(stderr):
            with self.assertRaises(SystemExit):
                run_checked(
                    [
                        "python3",
                        "-c",
                        "import sys; print('visible stdout'); print('visible stderr', file=sys.stderr); sys.exit(7)",
                    ],
                    REPO_ROOT,
                    "failing command",
                )

        output = stderr.getvalue()
        self.assertIn("stdout:\nvisible stdout", output)
        self.assertIn("stderr:\nvisible stderr", output)


if __name__ == "__main__":
    unittest.main()
