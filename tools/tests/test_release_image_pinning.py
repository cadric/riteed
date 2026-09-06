from __future__ import annotations

import tempfile
import textwrap
import unittest
from pathlib import Path

from tools.checks import release, release_workflow, workflow_images
from tools.tests.test_release_stress_policy import _copy_release_context


FEDORA_PIN = "fedora:42@sha256:99e203b80b1c3d8f7e161ec10a68fd02b081ef83a3963553e513c82846b97814"
FLATPAK_PIN = (
    "ghcr.io/flathub-infra/flatpak-github-actions:gnome-50"
    "@sha256:1fb2df10a57276f90806e1f35454048e30bf1855b7b4ff4808c9ee55887bd852"
)
OTHER_PIN = "registry.example.test/team/image:v1@sha256:" + "a" * 64


def _policy() -> dict[str, object]:
    return {
        "github_actions_release_safety": {
            "pinning": {"release_critical_container_images": "pin_to_sha256_digest"}
        }
    }


def _workflow(*, container: str = "", run: str = "true") -> str:
    container_line = f"    container: {container}\n" if container else ""
    indented_run = textwrap.indent(textwrap.dedent(run).strip("\n") + "\n", "          ")
    return (
        "name: Image fixture\n"
        "on: push\n"
        "jobs:\n"
        "  validation:\n"
        "    runs-on: ubuntu-latest\n"
        f"{container_line}"
        "    steps:\n"
        "      - name: Check images\n"
        "        run: |\n"
        f"{indented_run}"
    )


def _check(workflow: str, policy: dict[str, object] | None = None) -> list[str]:
    errors: list[str] = []
    model = release_workflow.parse(".github/workflows/image-fixture.yml", workflow, errors)
    workflow_images.check_workflow_images(_policy() if policy is None else policy, [model], errors)
    return errors


class ReleaseImagePinningTests(unittest.TestCase):
    def test_release_checker_rejects_floating_images_in_current_workflow(self) -> None:
        mutations = ((FEDORA_PIN, "fedora:42"), (FLATPAK_PIN, "ghcr.io/example/image:latest"))
        for pinned, floating in mutations:
            with self.subTest(floating=floating), tempfile.TemporaryDirectory() as tmpdir:
                root = Path(tmpdir)
                _copy_release_context(root)
                baseline: list[str] = []
                release.check_release(root, baseline)
                self.assertEqual(baseline, [])

                workflow_path = root / ".github" / "workflows" / "validate.yml"
                workflow = workflow_path.read_text(encoding="utf-8")
                self.assertIn(pinned, workflow)
                workflow_path.write_text(workflow.replace(pinned, floating, 1), encoding="utf-8")
                errors: list[str] = []
                release.check_release(root, errors)
                self.assertTrue(
                    any("must use an exact sha256 digest" in item for item in errors),
                    errors,
                )

    def test_job_container_mapping_and_scalar_require_exact_digest(self) -> None:
        mapping = _workflow().replace(
            "    steps:\n",
            f"    container:\n      image: {OTHER_PIN}\n    steps:\n",
        )
        self.assertEqual(_check(mapping), [])
        self.assertEqual(_check(_workflow(container=OTHER_PIN)), [])
        for workflow in (
            mapping.replace(OTHER_PIN, "registry.example.test/team/image:latest"),
            _workflow(container="${{ matrix.image }}"),
        ):
            with self.subTest(workflow=workflow):
                errors = _check(workflow)
                self.assertTrue(any("container image must use an exact sha256 digest" in item for item in errors), errors)

    def test_malformed_job_container_shapes_fail_closed(self) -> None:
        missing_image = _workflow().replace(
            "    steps:\n",
            "    container:\n      options: --privileged\n    steps:\n",
        )
        sequence = _workflow().replace(
            "    steps:\n",
            f"    container:\n      - image: {OTHER_PIN}\n    steps:\n",
        )
        for workflow in (missing_image, sequence):
            with self.subTest(workflow=workflow):
                errors = _check(workflow)
                self.assertTrue(any("container must declare a static image" in item for item in errors), errors)

    def test_actual_multiline_run_and_timeout_pull_support_known_options(self) -> None:
        workflow = _workflow(
            run=f"""
                timeout --foreground --kill-after=1m 10m docker --context ci pull \\
                  --platform=linux/amd64 {OTHER_PIN}
                docker pull --platform linux/arm64 {OTHER_PIN}
                docker run --rm --pull=never --privileged \\
                  -e A=B --env C=D --env=E=F \\
                  -v /one:/one --volume /two:/two --volume=/three:/three \\
                  -w /one --workdir /two --workdir=/three \\
                  {OTHER_PIN} \\
                  bash -lc '
                    cat <<INNER
                    docker pull payload-heredoc:latest
                    INNER
                    echo "docker run payload-decoy:latest"
                    printf "%s\\n" "docker pull quoted-decoy:latest"
                  '
            """
        )
        self.assertEqual(_check(workflow), [])

    def test_comments_echo_printf_assignments_and_heredoc_data_are_decoys(self) -> None:
        workflow = _workflow(
            run=f"""
                # docker pull comment-decoy:latest
                echo docker run echo-decoy:latest
                printf '%s\\n' docker pull printf-decoy:latest
                MESSAGE='docker run assignment-decoy:latest'
                cat <<'EOF'
                docker pull heredoc-decoy:latest
                EOF
                docker pull {OTHER_PIN}
            """
        )
        self.assertEqual(_check(workflow), [])

    def test_absolute_docker_executable_cannot_bypass_image_check(self) -> None:
        self.assertEqual(_check(_workflow(run=f"/usr/bin/docker run {OTHER_PIN} true")), [])
        errors = _check(_workflow(run="/usr/bin/docker pull registry.example.test/team/image:latest"))
        self.assertTrue(any("must use an exact sha256 digest" in item for item in errors), errors)

    def test_echo_and_printf_command_substitutions_are_executable(self) -> None:
        scripts = (
            'echo "$(docker pull registry.example.test/team/image:latest)"',
            'printf "%s\\n" "`docker run registry.example.test/team/image:latest true`"',
        )
        for script in scripts:
            with self.subTest(script=script):
                errors = _check(_workflow(run=script))
                self.assertTrue(any("cannot determine Docker image safely" in item for item in errors), errors)

    def test_shell_interpreter_heredoc_cannot_hide_docker_execution(self) -> None:
        scripts = (
            """
                bash <<'SCRIPT'
                docker pull registry.example.test/team/image:latest
                SCRIPT
            """,
            """
                bash <<'SCRIPT'
                docker \\
                  pull registry.example.test/team/image:latest
                SCRIPT
            """,
            """
                sh <<'SCRIPT'
                /usr/bin/docker \\
                  run registry.example.test/team/image:latest true
                SCRIPT
            """,
            """
                cat <<SCRIPT
                $(docker pull registry.example.test/team/image:latest)
                SCRIPT
            """,
            """
                cat <<'SCRIPT' | bash
                /usr/bin/docker pull registry.example.test/team/image:latest
                SCRIPT
            """,
        )
        for script in scripts:
            with self.subTest(script=script):
                errors = _check(_workflow(run=script))
                self.assertTrue(any("cannot determine Docker image safely" in item for item in errors), errors)

    def test_floating_actual_image_is_not_hidden_by_pinned_option_value_or_decoy(self) -> None:
        workflow = _workflow(
            run=f"""
                echo docker run {OTHER_PIN}
                docker run --env PINNED={OTHER_PIN} registry.example.test/team/image:latest sh -c true
            """
        )
        errors = _check(workflow)
        self.assertTrue(any("docker run image must use an exact sha256 digest" in item for item in errors), errors)

    def test_every_actual_docker_command_is_checked(self) -> None:
        workflow = _workflow(
            run=f"""
                docker pull {OTHER_PIN} && docker run {OTHER_PIN} true
                docker pull registry.example.test/team/second:latest
            """
        )
        errors = _check(workflow)
        self.assertEqual(sum("must use an exact sha256 digest" in item for item in errors), 1, errors)

    def test_digest_must_be_static_lowercase_and_exactly_64_hex(self) -> None:
        invalid_images = (
            "registry.example.test/team/image:v1@sha256:" + "a" * 63,
            "registry.example.test/team/image:v1@sha256:" + "a" * 65,
            "registry.example.test/team/image:v1@sha256:" + "A" * 64,
            "${{ env.IMAGE }}@sha256:" + "a" * 64,
            OTHER_PIN + ":extra",
        )
        for image in invalid_images:
            with self.subTest(image=image):
                errors = _check(_workflow(run=f"docker pull {image}"))
                self.assertTrue(any("must use an exact sha256 digest" in item for item in errors), errors)

    def test_unknown_missing_dynamic_and_wrapped_image_shapes_fail_closed(self) -> None:
        scripts = (
            f"docker run --mystery operand {OTHER_PIN} true",
            "docker run --env",
            "docker pull $IMAGE",
            f'docker "$SUBCOMMAND" {OTHER_PIN}',
            f'"$DOCKER" pull {OTHER_PIN}',
            f'timeout 10m "$DOCKER" run {OTHER_PIN} true',
            f"/opt/tools/docker pull {OTHER_PIN}",
            f"env PURPOSE=test docker pull {OTHER_PIN}",
            f"sh -c 'docker pull {OTHER_PIN}'",
            f"$(docker pull {OTHER_PIN})",
            f"`docker pull {OTHER_PIN}`",
        )
        for script in scripts:
            with self.subTest(script=script):
                errors = _check(_workflow(run=script))
                self.assertTrue(errors, script)
                self.assertTrue(
                    any(
                        "cannot determine Docker image safely" in item
                        or "must use an exact sha256 digest" in item
                        for item in errors
                    ),
                    errors,
                )

    def test_policy_requires_container_image_digest_rule(self) -> None:
        errors = _check(_workflow(run=f"docker pull {OTHER_PIN}"), policy={})
        self.assertTrue(any("release_critical_container_images must be pin_to_sha256_digest" in item for item in errors), errors)


if __name__ == "__main__":
    unittest.main()
