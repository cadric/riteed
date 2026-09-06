from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from tools.checks import release, release_workflow
from tools.tests.test_release_stress_policy import _copy_release_context


REPO_ROOT = Path(__file__).resolve().parents[2]


class GovernanceTruthfulRedTests(unittest.TestCase):
    def _errors_after(self, old: str, new: str) -> list[str]:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "validate.yml"
            text = path.read_text(encoding="utf-8")
            self.assertIn(old, text)
            path.write_text(text.replace(old, new, 1), encoding="utf-8")
            errors: list[str] = []
            release.check_release(root / "app", errors)
            return errors

    def test_release_static_cli_checks_release_contract_without_full_gate(self) -> None:
        result = subprocess.run(
            [
                sys.executable,
                "-m",
                "tools.policy_check",
                "--release-static-check",
                "--root",
                "app",
                "--strict",
            ],
            cwd=REPO_ROOT,
            check=False,
            encoding="utf-8",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=20,
        )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(result.stdout.strip(), "[policy-check] OK")

    def test_validate_uses_distinct_static_and_protected_live_jobs(self) -> None:
        path = REPO_ROOT / ".github" / "workflows" / "validate.yml"
        errors: list[str] = []
        workflow = release_workflow.parse(
            path.relative_to(REPO_ROOT).as_posix(),
            path.read_text(encoding="utf-8"),
            errors,
        )

        self.assertEqual(errors, [])
        self.assertIsNotNone(workflow)
        self.assertIn("governance-static", workflow.jobs)
        self.assertIn("governance-live", workflow.jobs)
        self.assertNotIn("ruleset-governance", workflow.jobs)
        self.assertEqual(workflow.jobs["governance-live"].environment, "ruleset-governance-live")

    def test_static_dependency_cannot_inherit_live_skip(self) -> None:
        errors = self._errors_after(
            "  governance-static:\n    needs: dependency-preflight\n",
            "  governance-static:\n    needs: governance-live\n",
        )
        self.assertTrue(any("governance-static" in error for error in errors), errors)

    def test_static_gate_runs_only_after_checkout_identity(self) -> None:
        identity_then_gate = (
            "      - name: Verify candidate checkout\n"
            "        run: |\n"
            "          set -euo pipefail\n"
            '          actual_head="$(git rev-parse HEAD)"\n'
            '          test "$actual_head" = "$GITHUB_SHA"\n'
            "      - name: Validate release governance statically\n"
            "        run: python3 -m tools.policy_check --release-static-check --root app --strict\n"
        )
        gate_then_identity = (
            "      - name: Validate release governance statically\n"
            "        run: python3 -m tools.policy_check --release-static-check --root app --strict\n"
            "      - name: Verify candidate checkout\n"
            "        run: |\n"
            "          set -euo pipefail\n"
            '          actual_head="$(git rev-parse HEAD)"\n'
            '          test "$actual_head" = "$GITHUB_SHA"\n'
        )
        errors = self._errors_after(
            identity_then_gate,
            gate_then_identity,
        )
        self.assertTrue(any("governance-static" in error for error in errors), errors)

    def test_live_checkout_cannot_be_skipped_or_ignored(self) -> None:
        checkout = (
            "  governance-live:\n"
            "    if: (github.event_name == 'push' || github.event_name == 'schedule' || github.event_name == 'workflow_dispatch') && github.ref == 'refs/heads/main'\n"
            "    needs: dependency-preflight\n"
            "    runs-on: ubuntu-latest\n"
            "    environment: ruleset-governance-live\n"
            "    permissions:\n"
            "      contents: read\n"
            "    steps:\n"
            "      - uses: actions/checkout@"
            "3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1\n"
        )
        for label, addition in (("condition", "        if: ${{ false }}\n"), ("continue", "        continue-on-error: true\n")):
            with self.subTest(label=label):
                errors = self._errors_after(checkout, checkout + addition)
                self.assertTrue(any("governance-live" in error for error in errors), errors)

    def test_static_job_rejects_skip_secret_and_identity_bypasses(self) -> None:
        cases = {
            "job condition": (
                "  governance-static:\n    needs: dependency-preflight\n",
                "  governance-static:\n    if: ${{ false }}\n    needs: dependency-preflight\n",
            ),
            "wrong checkout": (
                "          ref: ${{ github.sha }}\n      - uses: actions/setup-python@",
                "          ref: ${{ github.ref }}\n      - uses: actions/setup-python@",
            ),
            "identity fallback": (
                '          test "$actual_head" = "$GITHUB_SHA"\n'
                "      - name: Validate release governance statically",
                '          test "$actual_head" = "$GITHUB_SHA" || true\n'
                "      - name: Validate release governance statically",
            ),
            "gate condition": (
                "      - name: Validate release governance statically\n",
                "      - name: Validate release governance statically\n        if: ${{ false }}\n",
            ),
            "secret input": (
                "          python-version: \"3.12\"\n      - name: Verify candidate checkout",
                "          python-version: \"3.12\"\n"
                "          token: ${{ secrets.RULESET_GOVERNANCE_TOKEN }}\n"
                "      - name: Verify candidate checkout",
            ),
            "extra mutator": (
                "      - name: Validate release governance statically\n",
                "      - name: Mutate checked policy\n        run: touch policy/release.policy.json\n"
                "      - name: Validate release governance statically\n",
            ),
            "write permission": (
                "  governance-static:\n    needs: dependency-preflight\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n",
                "  governance-static:\n    needs: dependency-preflight\n    runs-on: ubuntu-latest\n    permissions:\n      contents: write\n",
            ),
        }
        for label, (old, new) in cases.items():
            with self.subTest(label=label):
                errors = self._errors_after(old, new)
                self.assertTrue(any("governance-static" in error for error in errors), errors)

    def test_live_job_rejects_producer_secret_and_command_bypasses(self) -> None:
        cases = {
            "wrong producer": (
                ") && github.ref == 'refs/heads/main'\n    needs: dependency-preflight",
                ") || github.ref == 'refs/heads/main'\n    needs: dependency-preflight",
            ),
            "wrong environment": (
                "    environment: ruleset-governance-live\n",
                "    environment: unprotected\n",
            ),
            "identity fallback": (
                '          test "$GITHUB_REPOSITORY" = "cadric/riteed"\n',
                '          test "$GITHUB_REPOSITORY" = "cadric/riteed" || true\n',
            ),
            "decisive condition": (
                "      - name: Verify GitHub ruleset governance\n",
                "      - name: Verify GitHub ruleset governance\n        if: ${{ false }}\n",
            ),
            "decisive fallback": (
                "        run: python3 -m tools.ruleset_governance_check\n\n  flatpak-tests:",
                "        run: python3 -m tools.ruleset_governance_check || true\n\n  flatpak-tests:",
            ),
            "early other secret": (
                "      - name: Verify trusted main checkout\n",
                "      - name: Early secret\n"
                "        env:\n          TOKEN: ${{ secrets.OTHER }}\n"
                "        run: true\n"
                "      - name: Verify trusted main checkout\n",
            ),
            "custom shell": (
                "        run: python3 -m tools.ruleset_governance_check\n\n  flatpak-tests:",
                "        shell: bash -n {0}\n"
                "        run: python3 -m tools.ruleset_governance_check\n\n  flatpak-tests:",
            ),
            "extra mutator": (
                "      - name: Verify GitHub ruleset governance\n",
                "      - name: Mutate checker\n        run: touch tools/ruleset_governance_check.py\n"
                "      - name: Verify GitHub ruleset governance\n",
            ),
        }
        for label, (old, new) in cases.items():
            with self.subTest(label=label):
                errors = self._errors_after(old, new)
                self.assertTrue(any("governance-live" in error for error in errors), errors)

    def test_governance_producer_triggers_are_exact(self) -> None:
        errors = self._errors_after(
            "  pull_request:\n  workflow_dispatch:\n",
            "  workflow_dispatch:\n",
        )
        self.assertTrue(any("governance producers" in error for error in errors), errors)


if __name__ == "__main__":
    unittest.main()
