from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.checks import stress_fuzz
from tools.tests.test_release_stress_policy import _copy_stress_context, _remove_remediation, _write


PAGE_TARGET = "page_text_decode"
PAGE_SEED_CLASSES = {
    "offset-zero.bin": "offset_zero",
    "offset-near-max.bin": "offset_near_max",
    "offset-max.bin": "offset_max",
    "empty.bin": "empty_payload",
    "incomplete-utf8.bin": "incomplete_utf8",
    "continuation-only.bin": "continuation_only",
}


class StressFuzzHardeningTests(unittest.TestCase):
    def test_required_fuzz_target_file_must_exist(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            target = root / "app" / "fuzz" / "fuzz_targets" / "unsupported_scanner.rs"
            target.unlink()
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("required fuzz target is missing" in item for item in errors), errors)

    def test_required_fuzz_target_must_have_exact_cargo_bin(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            manifest = root / "app" / "fuzz" / "Cargo.toml"
            block = (
                '\n[[bin]]\nname = "unsupported_scanner"\npath = "fuzz_targets/unsupported_scanner.rs"\n'
                "test = false\ndoc = false\nbench = false\n"
            )
            text = manifest.read_text(encoding="utf-8")
            self.assertIn(block, text)
            manifest.write_text(text.replace(block, ""), encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("Cargo [[bin]] registration" in item for item in errors), errors)

    def test_page_target_requires_every_policy_owned_seed_class(self) -> None:
        for seed_name, expected_class in PAGE_SEED_CLASSES.items():
            with self.subTest(seed=seed_name), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_stress_context(root)
                baseline_errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, baseline_errors)
                self.assertEqual(baseline_errors, [])
                (root / "app" / "fuzz" / "corpus" / PAGE_TARGET / seed_name).unlink()
                errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, errors)
                self.assertTrue(any(expected_class in item for item in errors), errors)

    def test_page_target_registry_mapping_is_bidirectional(self) -> None:
        for case in ("coverage", "source"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_stress_context(root)
                baseline_errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, baseline_errors)
                self.assertEqual(baseline_errors, [])
                registry = root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json"
                data = json.loads(registry.read_text(encoding="utf-8"))
                boundary = next(
                    item for item in data["boundaries"] if item.get("id") == "large_file_paged_reader"
                )
                if case == "coverage":
                    boundary["coverage"] = [
                        item for item in boundary["coverage"] if item.get("target") != PAGE_TARGET
                    ]
                    expected = "coverage missing required fuzz target page_text_decode"
                else:
                    boundary["source_paths"].remove("app/src/large_file/page_text.rs")
                    expected = "marker large_file_paged_reader is outside registered source_paths"
                registry.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
                errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, errors)
                self.assertTrue(any(expected in item for item in errors), errors)

    def test_ci_fuzz_loop_must_include_every_required_target(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            text = workflow.read_text(encoding="utf-8")
            loop = next(line for line in text.splitlines() if line.strip().startswith("for target in "))
            self.assertIn(f" {PAGE_TARGET}; do", loop)
            workflow.write_text(text.replace(loop, loop.replace(f" {PAGE_TARGET}", "")), encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("CI fuzz loop missing required targets" in item for item in errors), errors)

    def test_ci_fuzz_loop_must_execute_cargo_fuzz(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            invocation = '(cd fuzz && cargo +nightly fuzz run "$target" -- -max_total_time=1800)'
            text = workflow.read_text(encoding="utf-8")
            self.assertIn(invocation, text)
            workflow.write_text(text.replace(invocation, f"# {invocation}"), encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must execute cargo-fuzz" in item for item in errors), errors)

    def test_ci_fuzz_step_cannot_be_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            step = "      - name: Run scheduled stress suite\n        run: |"
            text = workflow.read_text(encoding="utf-8")
            self.assertIn(step, text)
            workflow.write_text(
                text.replace(step, "      - name: Run scheduled stress suite\n        if: false\n        run: |"),
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must not be disabled" in item for item in errors), errors)

    def test_ci_fuzz_job_condition_cannot_hide_required_event_tokens(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            condition = "    if: github.event_name == 'schedule' || github.event_name == 'workflow_dispatch'"
            text = workflow.read_text(encoding="utf-8")
            self.assertIn(condition, text)
            workflow.write_text(
                text.replace(condition, f"    if: false && ({condition.removeprefix('    if: ')})"),
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("policy-owned event condition" in item for item in errors), errors)

    def test_ci_fuzz_workflow_requires_manual_and_scheduled_triggers(self) -> None:
        mutations = {
            "manual": ("  workflow_dispatch:\n", "", "workflow_dispatch"),
            "schedule": ('  schedule:\n    - cron: "37 2 1 * *"\n', "", "schedule"),
            "empty-schedule": ('  schedule:\n    - cron: "37 2 1 * *"\n', "  schedule:\n", "non-empty cron"),
            "null-cron": ('    - cron: "37 2 1 * *"\n', "    - cron:\n", "non-empty cron"),
            "blank-cron": ('    - cron: "37 2 1 * *"\n', '    - cron: ""\n', "non-empty cron"),
            "numeric-cron": ('    - cron: "37 2 1 * *"\n', "    - cron: 37\n", "non-empty cron"),
            "numeric-cron-with-decoy": (
                '    - cron: "37 2 1 * *"\n',
                '    - cron: 37\n\ndecoy:\n  schedule:\n    - cron: "37 2 1 * *"\n',
                "non-empty cron",
            ),
        }
        for case, (original, replacement, expected) in mutations.items():
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_stress_context(root)
                baseline_errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, baseline_errors)
                self.assertEqual(baseline_errors, [])
                workflow = root / ".github" / "workflows" / "validate.yml"
                text = workflow.read_text(encoding="utf-8")
                self.assertIn(original, text)
                workflow.write_text(text.replace(original, replacement), encoding="utf-8")
                errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, errors)
                self.assertTrue(any(expected in item for item in errors), errors)

    def test_ci_fuzz_loop_inside_false_shell_guard_is_not_execution(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            guard = "              if [ -d fuzz ]; then"
            closing = "                done\n              fi\n            '"
            text = workflow.read_text(encoding="utf-8")
            self.assertIn(guard, text)
            self.assertIn(closing, text)
            workflow.write_text(
                text.replace(guard, f"              if false; then\n  {guard}").replace(
                    closing,
                    "                done\n              fi\n              fi\n            '",
                ),
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must execute cargo-fuzz" in item for item in errors), errors)

    def test_ci_fuzz_loop_inside_echoed_docker_command_is_not_execution(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            command = "          docker run --rm \\\n"
            text = workflow.read_text(encoding="utf-8")
            self.assertIn(command, text)
            workflow.write_text(
                text.replace(command, "          echo docker run --rm \\\n"),
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must execute cargo-fuzz" in item for item in errors), errors)

    def test_ci_fuzz_script_must_be_the_container_command(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            baseline_errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            workflow = root / ".github" / "workflows" / "validate.yml"
            command = "            fedora:42 \\\n            bash -lc '"
            text = workflow.read_text(encoding="utf-8")
            self.assertIn(command, text)
            workflow.write_text(
                text.replace(command, "            fedora:42 \\\n            echo \\\n            bash -lc '"),
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must execute cargo-fuzz" in item for item in errors), errors)

    def test_ci_fuzz_step_rejects_continue_on_error_and_custom_shell(self) -> None:
        mutations = {
            "continue": "        continue-on-error: true\n        run: |",
            "shell": "        shell: python\n        run: |",
        }
        for case, replacement in mutations.items():
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_stress_context(root)
                baseline_errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, baseline_errors)
                self.assertEqual(baseline_errors, [])
                workflow = root / ".github" / "workflows" / "validate.yml"
                text = workflow.read_text(encoding="utf-8")
                marker = "      - name: Run scheduled stress suite\n        run: |"
                self.assertIn(marker, text)
                workflow.write_text(
                    text.replace(marker, f"      - name: Run scheduled stress suite\n{replacement}"),
                    encoding="utf-8",
                )
                errors: list[str] = []
                stress_fuzz.check_stress_fuzz(root, errors)
                expected = "must not be disabled" if case == "continue" else "supported bash shell"
                self.assertTrue(any(expected in item for item in errors), errors)

    def test_parser_boundary_markers_are_bidirectional(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-015")
            parser = root / "app" / "src" / "markdown" / "parser.rs"
            parser.write_text(
                parser.read_text(encoding="utf-8").replace("// PARSER-BOUNDARY: id=markdown_parse\n", ""),
                encoding="utf-8",
            )
            unsupported = root / "app" / "src" / "markdown" / "unsupported.rs"
            unsupported.write_text(
                unsupported.read_text(encoding="utf-8") + "// PARSER-BOUNDARY: id=unregistered_parser\n",
                encoding="utf-8",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("markdown_parse must have PARSER-BOUNDARY marker" in item for item in errors), errors)
            self.assertTrue(any("unregistered_parser is not registered" in item for item in errors), errors)

    def test_stress_fidelity_ignores_role_tokens_without_executable_actions(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            _write(
                root / "stress" / "scripts" / "compare-roundtrip.json",
                json.dumps(
                    {
                        "flow": "compare-roundtrip",
                        "description": "role tokens are not actions",
                        "expect_failure": False,
                        "fixtures": [
                            {"role": "compare reference", "path": "stress/corpus/generated/compare-reference.txt"},
                            {"role": "compare current", "path": "stress/corpus/generated/compare-current.txt"},
                        ],
                        "actions": [{"type": "open", "path": "stress/corpus/generated/compare-current.txt"}],
                        "assertions": [{"type": "compare-pane-diff-state", "state": "diff-visible"}],
                        "artifact_dir": "stress/artifacts/compare-roundtrip",
                    }
                )
                + "\n",
            )
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("must_start_compare_workflow" in item for item in errors), errors)

    def test_stress_artifact_dir_rejects_parent_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            _remove_remediation(root, "stress-fuzz.policy.json", "RIT-AUD-008")
            script = root / "stress" / "scripts" / "open-save-search.json"
            data = json.loads(script.read_text(encoding="utf-8"))
            data["artifact_dir"] = "stress/artifacts/../../outside"
            script.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("artifact_dir must be under stress/artifacts/" in item for item in errors), errors)

    def test_future_registry_review_dates_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_stress_context(root)
            registry = root / "app" / "build-aux" / "validation" / "parser-boundaries.v1.json"
            data = json.loads(registry.read_text(encoding="utf-8"))
            data["boundaries"][0]["last_reviewed"] = "2099-01-01"
            data["boundaries"][0]["reviewed_exceptions"] = [
                {
                    "case": "future",
                    "reason": "future",
                    "review_artifact": "docs/audit_report.md",
                    "last_reviewed": "2099-01-01",
                }
            ]
            registry.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            stress_fuzz.check_stress_fuzz(root, errors)
            self.assertTrue(any("last_reviewed must not be after" in item for item in errors), errors)


if __name__ == "__main__":
    unittest.main()
