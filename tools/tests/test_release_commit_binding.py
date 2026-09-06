from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path

from tools.checks import release, release_guards, release_workflow
from tools.tests.test_release_stress_policy import _copy_release_context


REPO_ROOT = Path(__file__).resolve().parents[2]
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "publish-flatpak.yml"


class ReleaseCommitBindingTests(unittest.TestCase):
    def test_release_outputs_are_unique_and_owned_by_preflight(self) -> None:
        correct = {
            "version": 'echo "version=$version" >> "$GITHUB_OUTPUT"',
            "release_ref": 'echo "release_ref=$release_ref" >> "$GITHUB_OUTPUT"',
            "tag_commit": 'echo "tag_commit=$tag_commit" >> "$GITHUB_OUTPUT"',
        }
        for name, command in correct.items():
            with self.subTest(name=name, case="duplicate"):
                errors = self._errors_after(
                    lambda text, command=command, name=name: text.replace(
                        f"          {command}\n",
                        f"          {command}\n"
                        f'          echo "{name}=attacker" >> "$GITHUB_OUTPUT"\n',
                        1,
                    )
                )
                self.assertTrue(any("one stable peeled tag commit SHA" in item for item in errors), errors)
            with self.subTest(name=name, case="job output owner"):
                errors = self._errors_after(
                    lambda text, name=name: text.replace(
                        f"      {name}: ${{{{ steps.release.outputs.{name} }}}}\n",
                        f"      {name}: ${{{{ steps.other.outputs.{name} }}}}\n",
                        1,
                    )
                )
                self.assertTrue(any("one stable peeled tag commit SHA" in item for item in errors), errors)

    def test_head_verifier_precedes_every_signing_secret_exposure(self) -> None:
        verifier = "      - name: Verify checked-out release commit\n"
        secret_action = (
            "      - uses: actions/configure-pages@"
            "45bfe0192ca1faeb007ade9deae92b16b8254a0d\n"
            "        with:\n"
            "          token: ${{ secrets.FLATPAK_GPG_PRIVATE_KEY }}\n"
        )
        mutations = {
            "action input": lambda text: text.replace(
                verifier, secret_action + verifier, 1
            ),
            "job environment": lambda text: text.replace(
                "    permissions:\n      contents: read\n    steps:\n",
                "    permissions:\n      contents: read\n"
                "    env:\n"
                "      EARLY_KEY: ${{ secrets.FLATPAK_GPG_PRIVATE_KEY }}\n"
                "    steps:\n",
                1,
            ),
            "workflow environment": lambda text: text.replace(
                "env:\n  APP_ID:",
                "env:\n  EARLY_KEY: ${{ secrets.FLATPAK_GPG_PRIVATE_KEY }}\n  APP_ID:",
                1,
            ),
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                errors = self._errors_after(mutation)
                self.assertTrue(any("verify checked-out HEAD" in item for item in errors), errors)

    def test_sha_guard_exit_must_belong_to_the_exact_guard(self) -> None:
        guard_exit = (
            '          if [[ ! "$tag_commit" =~ ^[0-9a-f]{40}$ ]]; then\n'
            '            echo "Release tag must resolve to one full commit SHA." >&2\n'
            "            exit 1\n"
            "          fi\n"
        )
        decoy = (
            '          if [[ ! "$tag_commit" =~ ^[0-9a-f]{40}$ ]]; then\n'
            '            echo "Release tag must resolve to one full commit SHA." >&2\n'
            "            true\n"
            "          fi\n"
            "          if false; then\n"
            "            exit 1\n"
            "          fi\n"
        )
        errors = self._errors_after(lambda text: text.replace(guard_exit, decoy, 1))
        self.assertTrue(any("one stable peeled tag commit SHA" in item for item in errors), errors)

    def test_sha_guard_cannot_borrow_an_inert_heredoc_body(self) -> None:
        guard = (
            '          if [[ ! "$tag_commit" =~ ^[0-9a-f]{40}$ ]]; then\n'
            '            echo "Release tag must resolve to one full commit SHA." >&2\n'
            "            exit 1\n"
            "          fi\n"
        )
        decoy = (
            '          if [[ ! "$tag_commit" =~ ^[0-9a-f]{40}$ ]]; then\n'
            "            true\n"
            "          fi\n"
            "          : <<'SHA_DECOY'\n"
            + guard
            + "          SHA_DECOY\n"
        )
        errors = self._errors_after(lambda text: text.replace(guard, decoy, 1))
        self.assertTrue(any("one stable peeled tag commit SHA" in item for item in errors), errors)

    def test_release_ref_guard_cannot_borrow_an_inert_heredoc_body(self) -> None:
        guard = (
            '          if [[ ! "$release_ref" =~ ^v[0-9]+[.][0-9]+[.][0-9]+'
            '([-.+][A-Za-z0-9.-]+)?$ ]]; then\n'
            '            echo "Flatpak publish target must be a SemVer version tag." >&2\n'
            "            exit 1\n"
            "          fi\n"
        )
        decoy = (
            '          if [[ ! "$release_ref" =~ ^v[0-9]+[.][0-9]+[.][0-9]+'
            '([-.+][A-Za-z0-9.-]+)?$ ]]; then\n'
            "            true\n"
            "          fi\n"
            "          : <<'REF_DECOY'\n"
            + guard
            + "          REF_DECOY\n"
        )
        errors = self._errors_after(lambda text: text.replace(guard, decoy, 1))
        self.assertTrue(any("one stable peeled tag commit SHA" in item for item in errors), errors)

    def test_every_release_identity_consumer_uses_the_stable_sha(self) -> None:
        mutations = {
            "Cargo object": lambda text: text.replace(
                'git show "$tag_commit:app/Cargo.toml"',
                'git show "$release_ref:app/Cargo.toml"',
                1,
            ),
            "AppStream input": lambda text: text.replace(
                'TAG_COMMIT="$tag_commit" VERSION="$version" python3',
                'TAG_COMMIT="$release_ref" VERSION="$version" python3',
                1,
            ),
            "check collector": lambda text: text.replace(
                'CHECK_RUNS_JSON="$checks_json" TAG_COMMIT="$tag_commit" python3',
                'CHECK_RUNS_JSON="$checks_json" TAG_COMMIT="$release_ref" python3',
                1,
            ),
            "rollback candidate": lambda text: text.replace(
                'CANDIDATE_COMMIT="$tag_commit"',
                'CANDIDATE_COMMIT="$release_ref"',
                1,
            ),
            "published output": lambda text: text.replace(
                'echo "tag_commit=$tag_commit" >> "$GITHUB_OUTPUT"',
                'echo "tag_commit=$release_ref" >> "$GITHUB_OUTPUT"',
                1,
            ),
            "late SHA rebind": lambda text: text.replace(
                'TAG_COMMIT="$tag_commit" VERSION="$version" python3',
                'tag_commit="$(git rev-parse HEAD)"\n'
                '          TAG_COMMIT="$tag_commit" VERSION="$version" python3',
                1,
            ),
            "unchecked release ref rebind": lambda text: text.replace(
                "          git fetch origin",
                "          release_ref=v9.9.9\n          git fetch origin",
                1,
            ),
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                errors = self._errors_after(mutation)
                self.assertTrue(any("one stable peeled tag commit SHA" in item for item in errors), errors)

    def test_build_sha_binding_rejects_inactive_or_decoy_verifiers(self) -> None:
        verifier_name = "      - name: Verify checked-out release commit\n"
        verifier_block = (
            verifier_name
            + "        env:\n"
            + "          TAG_COMMIT: ${{ needs.preflight.outputs.tag_commit }}\n"
            + "        run: |\n"
            + "          set -euo pipefail\n"
            + '          actual_head="$(git rev-parse HEAD)"\n'
            + '          test "$actual_head" = "$TAG_COMMIT"\n'
        )
        mutations = {
            "condition": lambda text: text.replace(
                verifier_name, verifier_name + "        if: ${{ false }}\n", 1
            ),
            "continue on error": lambda text: text.replace(
                verifier_name, verifier_name + "        continue-on-error: true\n", 1
            ),
            "custom shell": lambda text: text.replace(
                "        run: |\n          set -euo pipefail\n"
                '          actual_head="$(git rev-parse HEAD)"\n',
                "        shell: bash -n {0}\n"
                "        run: |\n          set -euo pipefail\n"
                '          actual_head="$(git rev-parse HEAD)"\n',
                1,
            ),
            "success fallback": lambda text: text.replace(
                '          test "$actual_head" = "$TAG_COMMIT"\n',
                '          test "$actual_head" = "$TAG_COMMIT" || true\n',
                1,
            ),
            "wrong value": lambda text: text.replace(
                '          test "$actual_head" = "$TAG_COMMIT"\n',
                '          test "$actual_head" != "$TAG_COMMIT"\n',
                1,
            ),
            "after secrets": lambda text: text.replace(verifier_block, "", 1).replace(
                "      - name: Check Pages artifact shape\n",
                verifier_block + "      - name: Check Pages artifact shape\n",
                1,
            ),
            "unrelated job": lambda text: text.replace(verifier_block, "", 1)
            + "\n  verify-decoy:\n    runs-on: ubuntu-latest\n    steps:\n"
            + verifier_block.replace("      ", "      ", 1),
        }
        for name, mutation in mutations.items():
            with self.subTest(name=name):
                errors = self._errors_after(mutation)
                self.assertTrue(any("verify checked-out HEAD" in item for item in errors), errors)

    def test_bad_tag_metadata_is_not_replaced_by_good_worktree_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repository = self._repository(Path(tmpdir), valid_metadata=False)
            self._git(repository, "tag", "v1.2.3")
            self._write_metadata(repository, valid=True)

            self.assertNotEqual(self._run_extracted_appstream(repository, "1.2.3"), 0)

    def test_good_tag_metadata_is_not_replaced_by_bad_worktree_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repository = self._repository(Path(tmpdir), valid_metadata=True)
            self._git(repository, "tag", "v1.2.3")
            self._write_metadata(repository, valid=False)

            self.assertEqual(self._run_extracted_appstream(repository, "1.2.3"), 0)

    def test_cargo_version_and_sha_are_read_from_the_tag_object(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repository = self._repository(Path(tmpdir), valid_metadata=True)
            self._git(repository, "tag", "-a", "v1.2.3", "-m", "release")
            (repository / "app" / "Cargo.toml").write_text(
                '[package]\nname = "fixture"\nversion = "9.9.9"\n',
                encoding="utf-8",
            )
            expected_sha = self._git_output(
                repository, "rev-parse", "refs/tags/v1.2.3^{commit}"
            )

            tag_commit, version = self._run_extracted_identity(repository, "v1.2.3")

            self.assertEqual(tag_commit, expected_sha)
            self.assertEqual(version, "1.2.3")

    def test_moved_tag_cannot_change_the_build_checkout(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repository = self._repository(Path(tmpdir), valid_metadata=True)
            self._git(repository, "tag", "-a", "v1.2.3", "-m", "release")
            validated_sha = self._git_output(
                repository, "rev-parse", "refs/tags/v1.2.3^{commit}"
            )
            (repository / "tracked.txt").write_text("moved\n", encoding="utf-8")
            self._git(repository, "add", "tracked.txt")
            self._git(repository, "commit", "-m", "move tag target")
            self._git(
                repository,
                "tag",
                "--force",
                "-a",
                "v1.2.3",
                "-m",
                "moved release",
            )

            checkout_ref = self._build_checkout_ref()
            selected_ref = {
                "${{ needs.preflight.outputs.release_ref }}": "v1.2.3",
                "${{ needs.preflight.outputs.tag_commit }}": validated_sha,
            }.get(checkout_ref, checkout_ref)
            moved_sha = self._git_output(repository, "rev-parse", "HEAD")
            self._git(repository, "checkout", "--detach", selected_ref)
            checked_out_sha = self._git_output(repository, "rev-parse", "HEAD")

            self.assertEqual(checked_out_sha, validated_sha)
            verifier = self._head_verifier()
            self.assertEqual(
                self._run_shell(repository, verifier, TAG_COMMIT=validated_sha).returncode,
                0,
            )
            self.assertNotEqual(
                self._run_shell(repository, verifier, TAG_COMMIT=moved_sha).returncode,
                0,
            )

    def _workflow(self) -> release_workflow.Workflow:
        errors: list[str] = []
        workflow = release_workflow.parse(
            WORKFLOW_PATH.relative_to(REPO_ROOT).as_posix(),
            WORKFLOW_PATH.read_text(encoding="utf-8"),
            errors,
        )
        self.assertEqual(errors, [])
        self.assertIsNotNone(workflow)
        if workflow is None:
            self.fail("release workflow must parse")
        return workflow

    def _errors_after(self, mutation: Callable[[str], str]) -> list[str]:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            workflow_path = root / ".github" / "workflows" / "publish-flatpak.yml"
            original = workflow_path.read_text(encoding="utf-8")
            changed = mutation(original)
            self.assertNotEqual(changed, original)
            workflow_path.write_text(changed, encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
            return errors

    def _build_checkout_ref(self) -> str:
        build = self._workflow().jobs["build"]
        checkouts = [step for step in build.steps if step.uses.startswith("actions/checkout@")]
        self.assertEqual(len(checkouts), 1)
        raw_with = checkouts[0].raw.get("with")
        self.assertIsInstance(raw_with, dict)
        return str(raw_with.get("ref", ""))

    def _head_verifier(self) -> str:
        build = self._workflow().jobs["build"]
        verifiers = [
            step.run
            for step in build.steps
            if step.env.get("TAG_COMMIT")
            == "${{ needs.preflight.outputs.tag_commit }}"
            and "git rev-parse HEAD" in step.run
        ]
        self.assertEqual(len(verifiers), 1)
        return verifiers[0]

    def _run_extracted_appstream(self, repository: Path, version: str) -> int:
        source = release_guards.appstream_python(self._workflow())
        self.assertIsNotNone(source)
        if source is None:
            self.fail("AppStream release guard must be extractable")
        environment = os.environ.copy()
        environment["VERSION"] = version
        environment["TAG_COMMIT"] = self._git_output(
            repository, "rev-parse", "refs/tags/v1.2.3^{commit}"
        )
        return subprocess.run(
            ["python3", "-c", source],
            cwd=repository,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=10,
        ).returncode

    def _run_extracted_identity(
        self, repository: Path, release_ref: str
    ) -> tuple[str, str]:
        commands = release_guards.release_identity_commands(self._workflow())
        self.assertEqual(len(commands), 2)
        script = (
            "set -euo pipefail\n"
            + f"release_ref={release_ref}\n"
            + "\n".join(commands)
            + '\nprintf \'%s\\n%s\\n\' "$tag_commit" "$version"\n'
        )
        result = self._run_shell(repository, script)
        self.assertEqual(result.returncode, 0, result.stderr)
        lines = result.stdout.splitlines()
        self.assertEqual(len(lines), 2)
        return lines[0], lines[1]

    def _run_shell(
        self, repository: Path, source: str, **environment_updates: str
    ) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment.update(environment_updates)
        return subprocess.run(
            ["bash", "-c", source],
            cwd=repository,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=10,
        )

    def _repository(self, parent: Path, *, valid_metadata: bool) -> Path:
        repository = parent / "repository"
        self._git(parent, "init", "--initial-branch=main", str(repository))
        self._git(repository, "config", "user.name", "Release Test")
        self._git(repository, "config", "user.email", "release@example.invalid")
        (repository / "app").mkdir()
        (repository / "app" / "Cargo.toml").write_text(
            '[package]\nname = "fixture"\nversion = "1.2.3"\n',
            encoding="utf-8",
        )
        self._write_metadata(repository, valid=valid_metadata)
        (repository / "tracked.txt").write_text("first\n", encoding="utf-8")
        self._git(repository, "add", ".")
        self._git(repository, "commit", "-m", "release target")
        return repository

    def _write_metadata(self, repository: Path, *, valid: bool) -> None:
        path = repository / "app" / "data" / "io.github.cadric.Riteed.metainfo.xml"
        path.parent.mkdir(parents=True, exist_ok=True)
        version = "1.2.3" if valid else "9.9.9"
        path.write_text(
            f'<component><releases><release version="{version}" date="2026-01-01"/>'
            "</releases></component>\n",
            encoding="utf-8",
        )

    def _git(self, cwd: Path, *arguments: str) -> None:
        result = subprocess.run(
            ["git", *arguments],
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def _git_output(self, cwd: Path, *arguments: str) -> str:
        result = subprocess.run(
            ["git", *arguments],
            cwd=cwd,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
            timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()


if __name__ == "__main__":
    unittest.main()
