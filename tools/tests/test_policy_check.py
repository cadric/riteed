from __future__ import annotations

import json
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.checks import foundation, hig, libadwaita, runtime
from tools.scanners.sites import ReviewEntry, ScanHit, validate_review_links
from tools.validation_tooling import contract_root, repo_root


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

    def test_runtime_detached_task_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            shutil.copytree(REPO_ROOT / "policy", root / "policy")
            _write(root / "src" / "main.rs", "fn main() {\n    std::thread::spawn(|| println!(\"x\"));\n}\n")
            errors: list[str] = []
            runtime.check_runtime(root, errors)
            self.assertTrue(any("owned or supervised" in item for item in errors))

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

    def test_xgettext_completeness_uses_normalized_sets(self) -> None:
        from tools.checks import commands

        with patch.object(commands, "_xgettext_messages", return_value={(None, "Hello", None)}):
            with patch("tools.checks.commands.normalized_pot_messages", return_value={(None, "Hello", None)}):
                errors: list[str] = []
                commands.check_xgettext_completeness(REPO_ROOT, errors)
                self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
