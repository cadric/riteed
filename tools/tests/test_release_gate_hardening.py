from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

from tools import release_check_runs
from tools.checks import release, release_workflow
from tools.tests.test_release_stress_policy import _copy_release_context


HEAD_SHA = "0123456789abcdef0123456789abcdef01234567"


def _run(
    name: str,
    *,
    run_id: int = 100,
    status: str = "completed",
    conclusion: str | None = "success",
    head_sha: str = HEAD_SHA,
    app_slug: str = "github-actions",
) -> dict[str, object]:
    return {
        "id": run_id,
        "name": name,
        "status": status,
        "conclusion": conclusion,
        "head_sha": head_sha,
        "app": {"slug": app_slug},
        "started_at": "2026-09-05T10:00:00Z",
        "completed_at": "2026-09-05T10:01:00Z" if status == "completed" else None,
    }


class ReleaseCheckRunsDecisionTests(unittest.TestCase):
    def test_failed_latest_check_run_is_rejected(self) -> None:
        payloads = [{"total_count": 1, "check_runs": [_run("policy-pack", conclusion="failure")]}]

        errors = release_check_runs.check_required_runs(
            payloads,
            required_checks=["policy-pack"],
            head_sha=HEAD_SHA,
            app_slug="github-actions",
        )

        self.assertEqual(
            errors,
            [f"Required Validate check policy-pack for {HEAD_SHA} is failure, not success."],
        )

    def test_missing_skipped_wrong_sha_and_wrong_app_are_rejected(self) -> None:
        cases = {
            "missing": [],
            "skipped": [_run("policy-pack", conclusion="skipped")],
            "wrong sha": [_run("policy-pack", head_sha="f" * 40)],
            "wrong app": [_run("policy-pack", app_slug="third-party")],
        }
        for label, runs in cases.items():
            with self.subTest(label=label):
                errors = release_check_runs.check_required_runs(
                    [{"total_count": len(runs), "check_runs": runs}],
                    required_checks=["policy-pack"],
                    head_sha=HEAD_SHA,
                    app_slug="github-actions",
                )
                self.assertEqual(len(errors), 1, errors)

    def test_new_incomplete_rerun_invalidates_stale_success(self) -> None:
        old_success = _run("policy-pack", run_id=100)
        new_queued = _run("policy-pack", run_id=101, status="queued", conclusion=None)

        errors = release_check_runs.check_required_runs(
            [{"total_count": 2, "check_runs": [old_success, new_queued]}],
            required_checks=["policy-pack"],
            head_sha=HEAD_SHA,
            app_slug="github-actions",
        )

        self.assertEqual(
            errors,
            [f"Required Validate check policy-pack for {HEAD_SHA} is queued, not completed successfully."],
        )

    def test_new_successful_rerun_replaces_stale_failure(self) -> None:
        old_failure = _run("policy-pack", run_id=100, conclusion="failure")
        new_success = _run("policy-pack", run_id=101)

        errors = release_check_runs.check_required_runs(
            [{"total_count": 2, "check_runs": [old_failure, new_success]}],
            required_checks=["policy-pack"],
            head_sha=HEAD_SHA,
            app_slug="github-actions",
        )

        self.assertEqual(errors, [])

    def test_incomplete_pagination_cannot_accept_an_old_success(self) -> None:
        errors = release_check_runs.check_required_runs(
            [{"total_count": 2, "check_runs": [_run("policy-pack")]}],
            required_checks=["policy-pack"], head_sha=HEAD_SHA, app_slug="github-actions",
        )
        self.assertTrue(errors)

    def test_malformed_or_duplicate_check_run_ids_fail_closed(self) -> None:
        for bad_id in (True, -1, "100", None):
            with self.subTest(bad_id=bad_id):
                run = _run("policy-pack")
                run["id"] = bad_id
                errors = release_check_runs.check_required_runs(
                    [{"total_count": 1, "check_runs": [run]}],
                    required_checks=["policy-pack"], head_sha=HEAD_SHA, app_slug="github-actions",
                )
                self.assertTrue(errors)
        run = _run("policy-pack")
        errors = release_check_runs.check_required_runs(
            [{"total_count": 2, "check_runs": [run, run]}],
            required_checks=["policy-pack"], head_sha=HEAD_SHA, app_slug="github-actions",
        )
        self.assertTrue(errors)

    def test_release_sha_and_required_contexts_cannot_be_empty(self) -> None:
        for sha, required in (("", ["policy-pack"]), (HEAD_SHA, []), (HEAD_SHA, ["policy-pack", "policy-pack"])):
            with self.subTest(sha=sha, required=required):
                errors = release_check_runs.check_required_runs(
                    [{"total_count": 1, "check_runs": [_run("policy-pack")]}],
                    required_checks=required, head_sha=sha, app_slug="github-actions",
                )
                self.assertTrue(errors)

    def test_cli_reads_required_checks_and_app_from_policy(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            payload_path = root / "checks.json"
            policy_path = root / "release.policy.json"
            payload_path.write_text(
                json.dumps([{"total_count": 1, "check_runs": [_run("policy-pack", conclusion="failure")]}]),
                encoding="utf-8",
            )
            policy_path.write_text(
                json.dumps(
                    {
                        "signed_flatpak_publish": {
                            "hard_requirements": {
                                "required_validate_check_contexts": ["policy-pack"],
                                "required_check_app_slug": "github-actions",
                            }
                        }
                    }
                ),
                encoding="utf-8",
            )
            stderr = io.StringIO()

            with contextlib.redirect_stderr(stderr):
                status = release_check_runs.main(
                    [
                        "--input",
                        str(payload_path),
                        "--policy",
                        str(policy_path),
                        "--head-sha",
                        HEAD_SHA,
                    ]
                )

        self.assertEqual(status, 1)
        self.assertIn("is failure, not success", stderr.getvalue())


class ReleaseWorkflowGateTests(unittest.TestCase):
    def test_suite_tokens_cannot_substitute_for_an_executable_release_gate(self) -> None:
        policy = {"signed_flatpak_publish": {"hard_requirements": {
            "required_validate_check_contexts": ["policy-pack"],
            "required_check_app_slug": "github-actions",
            "release_critical_validation_suite": ["true"],
        }}}
        workflow = "jobs:\n  preflight:\n    steps:\n      - run: true\n"
        self.assertFalse(release_workflow.has_validation_before_secret(policy, workflow))

    def test_publish_gate_requires_dedicated_helper_invocation(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "publish-flatpak.yml"
            path.write_text(
                path.read_text(encoding="utf-8").replace(
                    "python3 -m tools.release_check_runs \\",
                    "python3 -m tools.release_check_runs_decoy \\",
                ),
                encoding="utf-8",
            )

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("strict release-check-runs invocation" in item for item in errors), errors)

    def test_publish_helper_cannot_be_hidden_in_shell_condition(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "publish-flatpak.yml"
            workflow = path.read_text(encoding="utf-8").replace(
                "          python3 -m tools.release_check_runs \\",
                "          if false; then\n            python3 -m tools.release_check_runs \\",
            )
            workflow = workflow.replace(
                "            --head-sha \"$tag_commit\"\n",
                "            --head-sha \"$tag_commit\"\n          fi\n",
            )
            path.write_text(workflow, encoding="utf-8")

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("strict release-check-runs invocation" in item for item in errors), errors)

    def test_signing_cannot_bypass_failed_preflight(self) -> None:
        for old, new in (
            ("  build:\n", "  build:\n    if: ${{ always() }}\n"),
            ("  preflight:\n", "  preflight:\n    continue-on-error: true\n"),
            ("    needs: preflight\n", "    needs: preflight\n    continue-on-error: true\n"),
        ):
            with self.subTest(new=new), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                _copy_release_context(root)
                path = root / '.github/workflows/publish-flatpak.yml'
                before = path.read_text()
                self.assertIn(old, before)
                path.write_text(before.replace(old, new))
                errors = []
                release.check_release(root, errors)
                self.assertTrue(any('exact-commit validation gate' in error for error in errors), errors)

    def test_each_signing_job_requires_its_own_validated_dependency_chain(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _copy_release_context(root)
            path = root / '.github/workflows/publish-flatpak.yml'
            path.write_text(path.read_text() + "\n  second-signing-job:\n    runs-on: ubuntu-latest\n    environment: flatpak-beta-signing\n    steps:\n      - run: echo FLATPAK_GPG_PRIVATE_KEY\n")
            errors = []
            release.check_release(root, errors)
            self.assertTrue(any('exact-commit validation gate' in error for error in errors), errors)

    def test_folded_yaml_cannot_turn_helper_into_set_arguments(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _copy_release_context(root)
            path = root / '.github/workflows/publish-flatpak.yml'
            before = path.read_text()
            old = '        run: |\n          set -euo pipefail\n          python3 -m tools.release_check_runs'
            self.assertIn(old, before)
            path.write_text(before.replace(old, old.replace('run: |', 'run: >')))
            errors = []
            release.check_release(root, errors)
            self.assertTrue(errors)

    def test_helper_and_governance_must_execute_in_supported_shell(self) -> None:
        for workflow, old, new in (
            ('publish-flatpak.yml', '        run: |\n          set -euo pipefail\n          python3 -m tools.release_check_runs',
             '        shell: bash -n {0}\n        run: |\n          set -euo pipefail\n          python3 -m tools.release_check_runs'),
            ('publish-flatpak.yml', 'jobs:\n', 'defaults:\n  run:\n    shell: bash -n {0}\njobs:\n'),
            ('publish-flatpak.yml', '  preflight:\n', '  preflight:\n    defaults:\n      run:\n        shell: bash -n {0}\n'),
            ('validate.yml', '      - name: Verify GitHub ruleset governance\n',
             '      - name: Verify GitHub ruleset governance\n        shell: bash -n {0}\n'),
            ('publish-flatpak.yml', 'jobs:\n', 'defaults:\n  run:\n    working-directory: fake-validator\njobs:\n'),
        ):
            with self.subTest(workflow=workflow, new=new), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                _copy_release_context(root)
                path = root / '.github/workflows' / workflow
                before = path.read_text()
                self.assertIn(old, before)
                path.write_text(before.replace(old, new))
                errors = []
                release.check_release(root, errors)
                self.assertTrue(errors)

    def test_build_checkout_ref_must_be_exact_tag_commit_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "publish-flatpak.yml"
            original = path.read_text(encoding="utf-8")
            changed = original.replace(
                "ref: ${{ needs.preflight.outputs.tag_commit }}",
                "ref: ${{ needs.preflight.outputs.tag_commit }}-decoy",
            )
            self.assertNotEqual(changed, original)
            path.write_text(changed, encoding="utf-8")

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("build checkout must target" in item for item in errors), errors)

    def test_rollback_candidate_ref_must_use_validated_release_ref(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            baseline_errors: list[str] = []
            release.check_release(root, baseline_errors)
            self.assertEqual(baseline_errors, [])
            path = root / ".github" / "workflows" / "publish-flatpak.yml"
            lines = path.read_text(encoding="utf-8").splitlines()
            assignments = [
                index for index, line in enumerate(lines) if line.strip().startswith("CANDIDATE_REF=")
            ]
            self.assertEqual(len(assignments), 1, assignments)
            indent = lines[assignments[0]][: -len(lines[assignments[0]].lstrip())]
            lines[assignments[0]] = f'{indent}CANDIDATE_REF="$GITHUB_REF_NAME" \\'
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("candidate ref must use validated release_ref" in item for item in errors), errors)

    def test_rollback_candidate_binding_rejects_detached_and_controlled_decoys(self) -> None:
        correct = '          CANDIDATE_REF="$release_ref" \\\n'
        wrong = '          CANDIDATE_REF="$GITHUB_REF_NAME" \\\n'
        block_start = '          CANDIDATE_VERSION="$version" \\\n'
        block_end = '          PY\n\n          if [[ -n "$published_version" ]]; then\n'
        with tempfile.TemporaryDirectory() as source_tmp:
            source = Path(source_tmp)
            _copy_release_context(source)
            source_path = source / ".github" / "workflows" / "publish-flatpak.yml"
            baseline = source_path.read_text(encoding="utf-8")
        self.assertEqual(baseline.count(correct), 1)
        self.assertEqual(baseline.count(block_end), 1)
        start = baseline.index(block_start)
        end = baseline.index(block_end, start) + len("          PY\n")
        owner = baseline[start:end]
        invocation = "          python3 - <<'PY'\n"
        prefix, body = owner.split(invocation, 1)
        comment_body = "".join(
            "          # " + line.strip() + "\n"
            for line in body.splitlines()[:-1]
        ) + "          pass\n          PY\n"
        mutations = {
            "missing prefix": baseline.replace(correct, "", 1),
            "duplicate prefix": baseline.replace(correct, correct + correct, 1),
            "comment decoy": baseline.replace(correct, wrong + "          # " + correct.lstrip(), 1),
            "later assignment": baseline.replace(correct, wrong, 1).replace(
                block_end,
                '          PY\n          CANDIDATE_REF="$release_ref"\n\n'
                '          if [[ -n "$published_version" ]]; then\n',
                1,
            ),
            "unindented false block": baseline.replace(
                block_start,
                "          if false; then\n" + block_start,
                1,
            ).replace(block_end, "          PY\n          fi\n\n          if [[ -n \"$published_version\" ]]; then\n", 1),
            "unrelated heredoc": baseline.replace(correct, wrong, 1).replace(
                block_start, correct + invocation + "          pass\n          PY\n" + block_start, 1,
            ),
            "duplicate owning block": baseline[:end] + owner + baseline[end:],
            "comment-only ownership markers": baseline.replace(owner, prefix + invocation + comment_body, 1),
            "wrong Python input with comment decoy": baseline.replace(
                'candidate_ref = os.environ["CANDIDATE_REF"]',
                'candidate_ref = os.environ["GITHUB_REF_NAME"]  # candidate_ref = os.environ["CANDIDATE_REF"]',
                1,
            ),
            "Python input overwritten": baseline.replace(
                '          candidate_ref = os.environ["CANDIDATE_REF"]\n',
                '          candidate_ref = os.environ["CANDIDATE_REF"]\n'
                '          candidate_ref = os.environ["GITHUB_REF_NAME"]\n', 1,
            ),
            "skipped owning step": baseline.replace("        id: release\n", "        id: release\n        if: ${{ false }}\n", 1),
            "multiline false control": baseline.replace(owner, "          if false\n          then\n" + owner + "          fi\n", 1),
            "short-circuited subshell": baseline.replace(owner, "          false && (\n" + owner + "          )\n", 1),
            "short-circuited command substitution": baseline.replace(
                owner, '          true || captured="$(\n' + owner + '          )"\n', 1,
            ),
            "unsupported select loop": baseline.replace(
                owner, "          select choice in skipped; do\n" + owner + "          done\n", 1,
            ),
        }
        for label, workflow in mutations.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_release_context(root)
                baseline_errors: list[str] = []
                release.check_release(root, baseline_errors)
                self.assertEqual(baseline_errors, [])
                path = root / ".github" / "workflows" / "publish-flatpak.yml"
                path.write_text(workflow, encoding="utf-8")
                errors: list[str] = []
                release.check_release(root, errors)
                self.assertTrue(
                    any("candidate ref must use validated release_ref" in item for item in errors),
                    errors,
                )

    def test_rollback_candidate_ref_source_is_policy_owned(self) -> None:
        for value in (None, "github_ref_name"):
            with self.subTest(value=value), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_release_context(root)
                baseline_errors: list[str] = []
                release.check_release(root, baseline_errors)
                self.assertEqual(baseline_errors, [])
                path = root / "policy" / "release.policy.json"
                policy = json.loads(path.read_text(encoding="utf-8"))
                monotonic = policy["signed_flatpak_publish"]["monotonic_remote_update"]
                if value is None:
                    monotonic.pop("candidate_ref_source")
                else:
                    monotonic["candidate_ref_source"] = value
                path.write_text(json.dumps(policy, indent=2) + "\n", encoding="utf-8")
                errors: list[str] = []
                release.check_release(root, errors)
                self.assertTrue(
                    any("candidate_ref_source must be validated_release_ref" in item for item in errors),
                    errors,
                )

    def test_governance_step_condition_must_match_approved_scope(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "validate.yml"
            lines = path.read_text(encoding="utf-8").splitlines()
            index = lines.index("      - name: Verify GitHub ruleset governance")
            lines.insert(index + 1, "        if: ${{ false }}")
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("governance-live" in item for item in errors), errors)

    def test_required_validate_job_cannot_be_conditionally_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "validate.yml"
            workflow = path.read_text(encoding="utf-8").replace(
                "  policy-pack:\n",
                "  policy-pack:\n    if: ${{ false }}\n",
            )
            path.write_text(workflow, encoding="utf-8")

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("policy-pack job must run unconditionally" in item for item in errors), errors)

    def test_required_validate_step_cannot_be_conditionally_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            path = root / ".github" / "workflows" / "validate.yml"
            workflow = path.read_text(encoding="utf-8").replace(
                "      - name: Run fast dependency preflight\n",
                "      - name: Run fast dependency preflight\n        if: ${{ false }}\n",
            )
            path.write_text(workflow, encoding="utf-8")

            errors: list[str] = []
            release.check_release(root, errors)

        self.assertTrue(any("dependency-preflight gate step must run unconditionally" in item for item in errors), errors)

    def test_required_validate_job_and_step_reject_continue_on_error(self) -> None:
        mutations = {
            "job": ("  policy-pack:\n", "  policy-pack:\n    continue-on-error: true\n"),
            "step": (
                "      - name: Run fast dependency preflight\n",
                "      - name: Run fast dependency preflight\n        continue-on-error: true\n",
            ),
        }
        for label, (old, new) in mutations.items():
            with self.subTest(label=label), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_release_context(root)
                path = root / ".github" / "workflows" / "validate.yml"
                path.write_text(path.read_text(encoding="utf-8").replace(old, new), encoding="utf-8")

                errors: list[str] = []
                release.check_release(root, errors)

                self.assertTrue(any("must not continue on error" in item for item in errors), errors)


if __name__ == "__main__":
    unittest.main()
