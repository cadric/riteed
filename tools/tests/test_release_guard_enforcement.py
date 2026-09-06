from __future__ import annotations

import json
import os
import subprocess
import tempfile
import unittest
from collections.abc import Callable
from pathlib import Path

from tools.checks import release, release_guards, release_workflow
from tools.tests.test_release_stress_policy import _copy_release_context

Mutation = Callable[[str], str]


def _without_block(text: str, start: str, end: str) -> str:
    before, separator, tail = text.partition(start)
    if not separator:
        return text
    _, separator, after = tail.partition(end)
    if not separator:
        return text
    return before + end + after


def _move_block_to_decoy(text: str, start: str, end: str) -> str:
    before, separator, tail = text.partition(start)
    if not separator:
        return text
    body, separator, after = tail.partition(end)
    if not separator:
        return text
    block = start + body
    return (
        before
        + end
        + after
        + "\n  release-guard-decoy:\n"
        + "    runs-on: ubuntu-latest\n"
        + "    steps:\n"
        + "      - run: |\n"
        + block
    )


class ReleaseGuardEnforcementTests(unittest.TestCase):
    def _errors_after(self, mutation: Mutation) -> list[str]:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            baseline_errors: list[str] = []
            release.check_release(root, baseline_errors)
            self.assertEqual(baseline_errors, [], "release fixture baseline must be clean")

            path = root / ".github/workflows/publish-flatpak.yml"
            original = path.read_text(encoding="utf-8")
            mutated = mutation(original)
            self.assertNotEqual(mutated, original, "workflow mutation must change the fixture")
            path.write_text(mutated, encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
        return errors

    def _assert_guard_error(self, mutation: Mutation, diagnostic: str) -> None:
        errors = self._errors_after(mutation)
        self.assertTrue(
            any(diagnostic in error for error in errors),
            f"missing diagnostic {diagnostic!r}: {errors!r}",
        )

    def _assert_guard_mutations(
        self, mutations: dict[str, Mutation], diagnostic: str
    ) -> None:
        for label, mutation in mutations.items():
            with self.subTest(label=label):
                self._assert_guard_error(mutation, diagnostic)

    def _policy_errors_after(self, path: tuple[str, ...], value: object) -> list[str]:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _copy_release_context(root)
            baseline: list[str] = []
            release.check_release(root, baseline)
            self.assertEqual(baseline, [], "release fixture baseline must be clean")
            policy_path = root / "policy/release.policy.json"
            policy = json.loads(policy_path.read_text(encoding="utf-8"))
            owner = policy
            for key in path[:-1]:
                owner = owner[key]
            owner[path[-1]] = value
            policy_path.write_text(json.dumps(policy, indent=2) + "\n", encoding="utf-8")
            errors: list[str] = []
            release.check_release(root, errors)
        return errors

    def test_release_requires_tag_ancestor_guard(self) -> None:
        self._assert_guard_error(
            lambda text: text.replace(
                '          git merge-base --is-ancestor "$tag_commit" origin/main\n',
                "",
                1,
            ),
            "tag commit must be an ancestor of origin/main",
        )

    def test_release_requires_appstream_version_guard(self) -> None:
        self._assert_guard_error(
            lambda text: _without_block(
                text,
                '          VERSION="$version" python3 - <<\'PY\'\n',
                '          remote_state="$(mktemp -d)"\n',
            ),
            "AppStream top release must match the release tag",
        )

    def test_release_requires_private_import_hygiene(self) -> None:
        self._assert_guard_error(
            lambda text: text.replace(
                '          export GNUPGHOME="$(mktemp -d)"\n'
                '          chmod 700 "$GNUPGHOME"\n'
                "          cleanup() {\n"
                '            gpgconf --homedir "$GNUPGHOME" --kill gpg-agent || true\n'
                '            rm -rf "$GNUPGHOME"\n'
                "          }\n"
                "          trap cleanup EXIT\n\n",
                "",
                1,
            ),
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_release_requires_github_hosted_build_runner(self) -> None:
        self._assert_guard_error(
            lambda text: text.replace(
                "  build:\n    runs-on: ubuntu-latest\n",
                "  build:\n    runs-on: self-hosted\n",
                1,
            ),
            "build signing job must use a GitHub-hosted Ubuntu runner",
        )

    def test_ancestry_guard_rejects_decoys_and_inactive_owners(self) -> None:
        merge = '          git merge-base --is-ancestor "$tag_commit" origin/main\n'
        self._assert_guard_mutations(
            {
                "comment": lambda text: text.replace(merge, "          # " + merge.lstrip(), 1),
                "wrong fetch ref": lambda text: text.replace(
                    "+refs/heads/main:refs/remotes/origin/main",
                    "+refs/heads/release:refs/remotes/origin/main",
                    1,
                ),
                "wrong tag source": lambda text: text.replace(
                    '          tag_commit="$(git rev-list -n 1 "$release_ref")"\n',
                    '          tag_commit="$(git rev-list -n 1 HEAD)"\n',
                    1,
                ),
                "unrelated job": lambda text: text.replace(merge, "", 1)
                + "\n  ancestry-decoy:\n    runs-on: ubuntu-latest\n    steps:\n"
                "      - run: |\n"
                + merge,
                "skipped step": lambda text: text.replace(
                    "        id: release\n",
                    "        id: release\n        if: ${{ false }}\n",
                    1,
                ),
                "continue on error": lambda text: text.replace(
                    "  preflight:\n",
                    "  preflight:\n    continue-on-error: true\n",
                    1,
                ),
            },
            "tag commit must be an ancestor of origin/main",
        )

    def test_appstream_guard_rejects_decoys_and_false_comparisons(self) -> None:
        start = '          VERSION="$version" python3 - <<\'PY\'\n'
        end = '          remote_state="$(mktemp -d)"\n'
        mismatch = "          if release_version != version:\n"
        exit_block = (
            "              )\n"
            "              sys.exit(1)\n"
            "          release_date_text = first_release.get(\"date\")\n"
        )
        self._assert_guard_mutations(
            {
                "wrong metadata": lambda text: text.replace(
                    "ET.parse(\"app/data/io.github.cadric.Riteed.metainfo.xml\")",
                    "ET.parse(\"app/data/other.metainfo.xml\")",
                    1,
                ),
                "short-circuited comparison": lambda text: text.replace(
                    mismatch, "          if False and release_version != version:\n", 1
                ),
                "nonfailing mismatch": lambda text: text.replace(
                    exit_block,
                    "              )\n"
                    "              pass\n"
                    "          release_date_text = first_release.get(\"date\")\n",
                    1,
                ),
                "unrelated job": lambda text: _move_block_to_decoy(text, start, end),
                "disabled shell owner": lambda text: text.replace(
                    start, "          if false; then\n" + start, 1
                ).replace(end, "          fi\n\n" + end, 1),
            },
            "AppStream top release must match the release tag",
        )

    def test_signing_hygiene_requires_each_active_component_before_import(self) -> None:
        components = {
            "temporary home": '          export GNUPGHOME="$(mktemp -d)"\n',
            "private mode": '          chmod 700 "$GNUPGHOME"\n',
            "agent kill": '            gpgconf --homedir "$GNUPGHOME" --kill gpg-agent || true\n',
            "home removal": '            rm -rf "$GNUPGHOME"\n',
            "exit trap": "          trap cleanup EXIT\n",
        }
        self._assert_guard_mutations(
            {
                label: (lambda text, line=line: text.replace(line, "", 1))
                for label, line in components.items()
            },
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_signing_hygiene_rejects_decoy_and_inactive_step(self) -> None:
        start = '          export GNUPGHOME="$(mktemp -d)"\n'
        end = (
            '          printf \'%s\' "$FLATPAK_GPG_PRIVATE_KEY" | gpg --batch --import\n'
        )
        self._assert_guard_mutations(
            {
                "unrelated job": lambda text: _move_block_to_decoy(text, start, end),
                "skipped step": lambda text: text.replace(
                    "      - name: Build signed Flatpak repository\n",
                    "      - name: Build signed Flatpak repository\n        if: ${{ false }}\n",
                    1,
                ),
                "continue on error": lambda text: text.replace(
                    "      - name: Build signed Flatpak repository\n",
                    "      - name: Build signed Flatpak repository\n        continue-on-error: true\n",
                    1,
                ),
            },
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_hosted_runner_guard_rejects_shapes_and_broken_dependency(self) -> None:
        self._assert_guard_mutations(
            {
                "runner list": lambda text: text.replace(
                    "    runs-on: ubuntu-latest\n    needs: preflight\n",
                    "    runs-on:\n      - self-hosted\n      - linux\n    needs: preflight\n",
                    1,
                ),
                "runner expression": lambda text: text.replace(
                    "    runs-on: ubuntu-latest\n    needs: preflight\n",
                    "    runs-on: ${{ matrix.runner }}\n    needs: preflight\n",
                    1,
                ),
                "missing preflight": lambda text: text.replace(
                    "    runs-on: ubuntu-latest\n    needs: preflight\n",
                    "    runs-on: ubuntu-latest\n",
                    1,
                ),
                "inactive build": lambda text: text.replace(
                    "  build:\n    runs-on: ubuntu-latest\n",
                    "  build:\n    if: ${{ false }}\n    runs-on: ubuntu-latest\n",
                    1,
                ),
                "unrelated ubuntu": lambda text: text.replace(
                    "  build:\n    runs-on: ubuntu-latest\n",
                    "  build:\n    runs-on: self-hosted\n",
                    1,
                )
                + "\n  runner-decoy:\n    runs-on: ubuntu-latest\n    steps:\n"
                "      - run: echo decoy\n",
            },
            "build signing job must use a GitHub-hosted Ubuntu runner",
        )

    def test_ancestry_guard_cannot_hide_in_short_circuited_subshell(self) -> None:
        start = (
            '          git fetch origin "+refs/tags/$release_ref:refs/tags/$release_ref" '
            "+refs/heads/main:refs/remotes/origin/main\n"
        )
        end = '          git merge-base --is-ancestor "$tag_commit" origin/main\n'
        self._assert_guard_error(
            lambda text: text.replace(start, "          false && (\n" + start, 1).replace(
                end, end + "          )\n", 1
            ),
            "tag commit must be an ancestor of origin/main",
        )

    def test_ancestry_guard_rejects_tag_commit_rebinding(self) -> None:
        derived = '          tag_commit="$(git rev-list -n 1 "$release_ref")"\n'
        self._assert_guard_mutations(
            {
                "assignment": lambda text: text.replace(
                    derived, derived + '          tag_commit="$(git rev-parse HEAD)"\n', 1
                ),
                "export": lambda text: text.replace(
                    derived, derived + "          export tag_commit=HEAD\n", 1
                ),
                "unset": lambda text: text.replace(
                    derived, derived + "          unset tag_commit\n", 1
                ),
                "true control": lambda text: text.replace(
                    derived,
                    derived
                    + "          if true; then\n"
                    + "            tag_commit=HEAD\n"
                    + "          fi\n",
                    1,
                ),
            },
            "tag commit must be an ancestor of origin/main",
        )

    def test_release_guard_rejects_release_inputs_rebinding(self) -> None:
        fetch = (
            '          git fetch origin "+refs/tags/$release_ref:refs/tags/$release_ref" '
            "+refs/heads/main:refs/remotes/origin/main\n"
        )
        version_guard = (
            '          if [[ "$release_ref" != "v$version" ]]; then\n'
            '            echo "Tag $release_ref does not match app/Cargo.toml version $version." >&2\n'
            "            exit 1\n"
            "          fi\n"
        )
        self._assert_guard_mutations(
            {
                "release ref": lambda text: text.replace(
                    fetch, fetch + "          release_ref=v-attacker\n", 1
                ),
                "version": lambda text: text.replace(
                    version_guard, version_guard + "          version=9.9.9\n", 1
                ),
                "release ref true control": lambda text: text.replace(
                    fetch,
                    fetch
                    + "          if true; then\n"
                    + "            release_ref=v-attacker\n"
                    + "          fi\n",
                    1,
                ),
                "version true control": lambda text: text.replace(
                    version_guard,
                    version_guard
                    + "          if true; then\n"
                    + "            version=9.9.9\n"
                    + "          fi\n",
                    1,
                ),
            },
            "tag commit must be an ancestor of origin/main",
        )

    def test_signing_guard_cannot_hide_in_command_substitution(self) -> None:
        start = '          export GNUPGHOME="$(mktemp -d)"\n'
        end = "          trap cleanup EXIT\n"

        def mutate(text: str) -> str:
            before, separator, tail = text.partition(start)
            self.assertTrue(separator)
            body, separator, after = tail.partition(end)
            self.assertTrue(separator)
            return (
                before
                + '          ignored="$(\n'
                + start
                + body
                + end
                + '          )"\n'
                + after
            )

        self._assert_guard_error(
            mutate,
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_signing_guard_rejects_gnupg_home_rebinding(self) -> None:
        chmod = '          chmod 700 "$GNUPGHOME"\n'
        private_import = (
            '          printf \'%s\' "$FLATPAK_GPG_PRIVATE_KEY" | gpg --batch --import\n'
        )
        self._assert_guard_mutations(
            {
                "assignment before import": lambda text: text.replace(
                    chmod, chmod + "          GNUPGHOME=/tmp/unsafe\n", 1
                ),
                "export after import": lambda text: text.replace(
                    private_import,
                    private_import + "          export GNUPGHOME=/tmp/unsafe\n",
                    1,
                ),
                "unset after import": lambda text: text.replace(
                    private_import, private_import + "          unset GNUPGHOME\n", 1
                ),
                "true control": lambda text: text.replace(
                    private_import,
                    private_import
                    + "          if true; then\n"
                    + "            GNUPGHOME=/tmp/unsafe\n"
                    + "          fi\n",
                    1,
                ),
            },
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_signing_cleanup_cannot_be_nested_in_disabled_control(self) -> None:
        cleanup = (
            '            gpgconf --homedir "$GNUPGHOME" --kill gpg-agent || true\n'
            '            rm -rf "$GNUPGHOME"\n'
        )
        self._assert_guard_error(
            lambda text: text.replace(
                cleanup,
                "            if false; then\n" + cleanup + "            fi\n",
                1,
            ),
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_signing_cleanup_cannot_be_overridden_after_import(self) -> None:
        private_import = (
            '          printf \'%s\' "$FLATPAK_GPG_PRIVATE_KEY" | gpg --batch --import\n'
        )
        self._assert_guard_error(
            lambda text: text.replace(
                private_import,
                private_import + "          cleanup() { :; }\n",
                1,
            ),
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_signing_exit_trap_cannot_be_cleared_after_import(self) -> None:
        private_import = (
            '          printf \'%s\' "$FLATPAK_GPG_PRIVATE_KEY" | gpg --batch --import\n'
        )
        self._assert_guard_error(
            lambda text: text.replace(
                private_import,
                private_import + "          trap - EXIT\n",
                1,
            ),
            "private key import requires temporary GNUPGHOME cleanup",
        )

    def test_signing_exit_alias_and_function_cannot_be_rebound(self) -> None:
        private_import = (
            '          printf \'%s\' "$FLATPAK_GPG_PRIVATE_KEY" | gpg --batch --import\n'
        )
        self._assert_guard_mutations(
            {
                "clear signal zero": lambda text: text.replace(
                    private_import, private_import + "          trap - 0\n", 1
                ),
                "replace signal zero": lambda text: text.replace(
                    private_import, private_import + "          trap ':' 0\n", 1
                ),
                "mixed EXIT signals": lambda text: text.replace(
                    private_import,
                    private_import + "          trap - EXIT SIGTERM\n",
                    1,
                ),
                "mixed signal zero": lambda text: text.replace(
                    private_import, private_import + "          trap ':' 0 HUP\n", 1
                ),
                "unset cleanup function": lambda text: text.replace(
                    private_import, private_import + "          unset -f cleanup\n", 1
                ),
            },
            "private key import requires temporary GNUPGHOME cleanup",
        )

        harmless = self._errors_after(
            lambda text: text.replace(
                private_import,
                private_import + "          trap 'echo 0 and EXIT' TERM\n",
                1,
            )
        )
        self.assertFalse(
            any("private key import requires" in error for error in harmless), harmless
        )

    def test_appstream_guard_requires_unique_ordered_inputs(self) -> None:
        assignment = '          version = os.environ["VERSION"]\n'
        self._assert_guard_error(
            lambda text: text.replace(assignment, assignment + assignment, 1),
            "AppStream top release must match the release tag",
        )

    def test_appstream_guard_requires_real_stdlib_imports(self) -> None:
        self._assert_guard_error(
            lambda text: text.replace(
                "          import datetime as dt\n          import os\n",
                "          import datetime as dt\n          # import os\n",
                1,
            ),
            "AppStream top release must match the release tag",
        )

    def test_guard_requirements_are_policy_owned(self) -> None:
        requirements = (
            (
                ("release_identity", "tag_commit_must_be_ancestor_of_main"),
                "release_identity.tag_commit_must_be_ancestor_of_main must be true",
            ),
            (
                ("release_identity", "appstream_top_release_must_match_tag"),
                "release_identity.appstream_top_release_must_match_tag must be true",
            ),
            (
                "signing_key_governance",
                "private_key_import",
                "temporary_gnupg_home_required",
            ),
            (
                "signing_key_governance",
                "private_key_import",
                "kill_agent_on_exit_required",
            ),
            (
                "github_actions_release_safety",
                "mutable_inputs",
                "github_hosted_runner_required_until_self_hosted_policy_exists",
            ),
        )
        for requirement in requirements:
            if isinstance(requirement[0], tuple):
                path, diagnostic = requirement
            else:
                path = requirement
                diagnostic = ".".join(requirement) + " must be true"
            for value in (False, None):
                with self.subTest(path=path, value=value):
                    errors = self._policy_errors_after(path, value)
                    self.assertTrue(
                        any(diagnostic in error for error in errors),
                        errors,
                    )

    def test_extracted_ancestry_guard_accepts_only_main_ancestor(self) -> None:
        with tempfile.TemporaryDirectory() as fixture_tmp:
            fixture = Path(fixture_tmp)
            _copy_release_context(fixture)
            parse_errors: list[str] = []
            workflow = release_workflow.parse(
                ".github/workflows/publish-flatpak.yml",
                (fixture / ".github/workflows/publish-flatpak.yml").read_text(
                    encoding="utf-8"
                ),
                parse_errors,
            )
            self.assertEqual(parse_errors, [])
            self.assertIsNotNone(workflow)
            if workflow is None:
                self.fail("release workflow must parse")
            commands = release_guards.ancestry_commands(workflow)
            self.assertEqual(commands, list(release_guards.ANCESTRY_COMMANDS))

            remote = fixture / "origin.git"
            repository = fixture / "repository"
            self._git(fixture, "init", "--bare", str(remote))
            self._git(fixture, "init", "--initial-branch=main", str(repository))
            self._git(repository, "config", "user.name", "Guard Test")
            self._git(repository, "config", "user.email", "guard@example.invalid")
            (repository / "tracked.txt").write_text("main\n", encoding="utf-8")
            self._git(repository, "add", "tracked.txt")
            self._git(repository, "commit", "-m", "main")
            self._git(repository, "remote", "add", "origin", str(remote))
            self._git(repository, "push", "-u", "origin", "main")
            self._git(repository, "tag", "v-main")
            self._git(repository, "push", "origin", "v-main")
            self._git(repository, "switch", "-c", "unmerged")
            (repository / "tracked.txt").write_text("unmerged\n", encoding="utf-8")
            self._git(repository, "commit", "-am", "unmerged")
            self._git(repository, "tag", "v-unmerged")
            self._git(repository, "push", "origin", "v-unmerged")

            self.assertEqual(self._run_ancestry(repository, commands, "v-main"), 0)
            self.assertNotEqual(
                self._run_ancestry(repository, commands, "v-unmerged"), 0
            )

    def test_extracted_appstream_guard_checks_top_release(self) -> None:
        with tempfile.TemporaryDirectory() as fixture_tmp:
            fixture = Path(fixture_tmp)
            _copy_release_context(fixture)
            errors: list[str] = []
            workflow = release_workflow.parse(
                ".github/workflows/publish-flatpak.yml",
                (fixture / ".github/workflows/publish-flatpak.yml").read_text(
                    encoding="utf-8"
                ),
                errors,
            )
            self.assertEqual(errors, [])
            self.assertIsNotNone(workflow)
            if workflow is None:
                self.fail("release workflow must parse")
            source = release_guards.appstream_python(workflow)
            self.assertIsNotNone(source)
            if source is None:
                self.fail("AppStream guard must be extractable")
            metadata = fixture / "app/data/io.github.cadric.Riteed.metainfo.xml"
            metadata.parent.mkdir(parents=True, exist_ok=True)
            metadata.write_text(
                '<component><releases><release version="1.2.3" date="2026-01-01"/>'
                "</releases></component>\n",
                encoding="utf-8",
            )
            self.assertEqual(self._run_appstream(fixture, source, "1.2.3"), 0)
            self.assertNotEqual(self._run_appstream(fixture, source, "9.9.9"), 0)
            metadata.write_text("<component/>\n", encoding="utf-8")
            self.assertNotEqual(self._run_appstream(fixture, source, "1.2.3"), 0)

    def _git(self, cwd: Path, *arguments: str) -> None:
        result = subprocess.run(
            ["git", *arguments],
            cwd=cwd,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def _run_ancestry(
        self, repository: Path, commands: list[str], release_ref: str
    ) -> int:
        script = "set -euo pipefail\n" + f"release_ref={release_ref}\n" + "\n".join(commands)
        return subprocess.run(
            ["bash", "-c", script],
            cwd=repository,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        ).returncode

    def _run_appstream(self, fixture: Path, source: str, version: str) -> int:
        environment = os.environ.copy()
        environment["VERSION"] = version
        return subprocess.run(
            ["python3", "-c", source],
            cwd=fixture,
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        ).returncode


if __name__ == "__main__":
    unittest.main()
