from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from tools.checks import foundation, runtime
from tools.tests.test_release_stress_policy import _copy_policy, _write
from tools.validation_tooling import repo_root

REPO = Path(__file__).resolve().parents[2]


class PolicyAuditTests(unittest.TestCase):
    def test_explicit_missing_root_never_falls_back_to_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with patch('tools.validation_tooling.Path.cwd', return_value=REPO / 'app'):
                with self.assertRaises(SystemExit):
                    repo_root(str(Path(tmp) / 'missing'))

    def test_explicit_existing_non_app_never_falls_back_to_cwd(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            with patch('tools.validation_tooling.Path.cwd', return_value=REPO / 'app'):
                with self.assertRaises(SystemExit):
                    repo_root(tmp)

    def test_explicit_missing_descendant_is_not_its_parent_app(self) -> None:
        with self.assertRaises(SystemExit):
            repo_root(str(REPO / 'app' / 'intentionally-missing-audit-target'))

    def test_existing_app_root_remains_supported(self) -> None:
        self.assertEqual(repo_root(str(REPO / 'app')), REPO / 'app')

    def test_review_fields_must_contain_typed_evidence(self) -> None:
        invalid = [[], {}, False, 12, '   ']
        for field in ('ownership', 'justification', 'path', 'match', 'kind'):
            for value in invalid:
                with self.subTest(field=field, value=value), tempfile.TemporaryDirectory() as tmp:
                    root = Path(tmp)
                    _copy_policy(root)
                    _write(root / 'src/probe.rs', 'fn probe(p: &std::path::Path) -> bool {\n    p.exists()\n}\n')
                    entry = {'path': 'src/probe.rs', 'line': 2, 'match': 'p.exists()',
                             'kind': 'runtime-sync-fs', 'ownership': 'native local path',
                             'justification': 'Only local native paths reach this call.'}
                    entry[field] = value
                    _write(root / 'build-aux/validation/runtime-review-audit.json',
                           json.dumps({'version': 1, 'sites': [entry]}))
                    errors: list[str] = []
                    runtime.check_runtime(root, errors)
                    self.assertTrue(errors, f'{field}={value!r} accepted')

    def test_review_boolean_version_is_invalid(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _copy_policy(root)
            _write(root / 'build-aux/validation/runtime-review-audit.json',
                   json.dumps({'version': True, 'sites': []}))
            errors: list[str] = []
            runtime.check_runtime(root, errors)
            self.assertTrue(errors)

    def test_toolchain_family_is_read_from_policy(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _copy_policy(root)
            _write(root / 'rust-toolchain.toml', '[toolchain]\nchannel="1.95.0"\ncomponents=["rustfmt", "clippy"]\n')
            errors: list[str] = []
            foundation.check_toolchain(root, errors)
            self.assertEqual(errors, [])
            policy_path = root / 'policy/rust.policy.json'
            policy = json.loads(policy_path.read_text())
            policy['targets']['rust']['target_rust_family'] = '1.96.x'
            policy_path.write_text(json.dumps(policy))
            foundation.check_toolchain(root, errors)
            self.assertTrue(errors, 'policy/toolchain version drift was accepted')
            _write(root / 'rust-toolchain.toml', '[toolchain]\nchannel="1.96.1"\ncomponents=["rustfmt", "clippy"]\n')
            errors.clear()
            foundation.check_toolchain(root, errors)
            self.assertEqual(errors, [])

    def test_toolchain_rejects_prefix_lookalikes_and_unstable_suffix(self) -> None:
        for channel in ('1.950.0', '1.95.0-nightly', '1.95', '1.95.x'):
            with self.subTest(channel=channel), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                _copy_policy(root)
                _write(root / 'rust-toolchain.toml', f'[toolchain]\nchannel="{channel}"\ncomponents=["rustfmt", "clippy"]\n')
                errors: list[str] = []
                foundation.check_toolchain(root, errors)
                self.assertTrue(errors)

    def test_toolchain_components_are_read_from_policy(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _copy_policy(root)
            _write(root / 'rust-toolchain.toml', '[toolchain]\nchannel="1.95.0"\ncomponents=["rustfmt", "clippy"]\n')
            path = root / 'policy/rust.policy.json'
            policy = json.loads(path.read_text())
            policy['toolchain']['required_components'].append('llvm-tools-preview')
            path.write_text(json.dumps(policy))
            errors: list[str] = []
            foundation.check_toolchain(root, errors)
            self.assertTrue(errors)

    def test_wrapper_works_from_an_unrelated_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            result = subprocess.run([str(REPO / 'scripts/policy-check'), '--help'],
                                    cwd=tmp, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('--root', result.stdout)
