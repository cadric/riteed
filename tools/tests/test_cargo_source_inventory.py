from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools.checks.dependency_preflight import check_dependency_preflight
from tools.tests.test_dependency_preflight import _fixture


class CargoSourceInventoryTests(unittest.TestCase):
    def test_every_extra_source_is_accounted_for(self) -> None:
        extras = [
            {'type': 'archive', 'archive-type': 'tar-gzip', 'url': 'https://example.invalid/a.tar.gz',
             'sha256': 'a'*64, 'dest': 'cargo/vendor/extra'},
            {'type': 'inline', 'contents': '{}', 'dest': 'cargo/vendor/orphan', 'dest-filename': '.cargo-checksum.json'},
            {'type': 'shell', 'commands': ['false']},
            'cargo-sources-extra.json',
            {},
            None,
        ]
        for extra in extras:
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as tmp:
                app = _fixture(tmp)
                path = app / 'build-aux/cargo/cargo-sources.json'
                data = json.loads(path.read_text())
                data.append(extra)
                path.write_text(json.dumps(data))
                errors: list[str] = []
                check_dependency_preflight(app, errors)
                self.assertTrue(errors, f'unapproved source accepted: {extra!r}')

    def test_source_options_cannot_modify_an_otherwise_valid_archive(self) -> None:
        for key, value in [('commands', ['false']), ('only-arches', ['not-this-arch']), ('strip-components', 0)]:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as tmp:
                app = _fixture(tmp)
                path = app / 'build-aux/cargo/cargo-sources.json'
                data = json.loads(path.read_text())
                data[0][key] = value
                path.write_text(json.dumps(data))
                errors: list[str] = []
                check_dependency_preflight(app, errors)
                self.assertTrue(errors)

    def test_vendor_config_cannot_add_network_registry_or_commands(self) -> None:
        for contents in ('[source.crates-io]\nregistry="https://example.invalid"\n',
                         '[source.vendored-sources]\ndirectory="cargo/vendor"\n[source.crates-io]\nreplace-with="vendored-sources"\n[net]\ngit-fetch-with-cli=true\n'):
            with self.subTest(contents=contents), tempfile.TemporaryDirectory() as tmp:
                app = _fixture(tmp)
                path = app / 'build-aux/cargo/cargo-sources.json'
                data = json.loads(path.read_text())
                data.append({'type': 'inline', 'dest': 'cargo', 'dest-filename': 'config', 'contents': contents})
                path.write_text(json.dumps(data))
                errors: list[str] = []
                check_dependency_preflight(app, errors)
                self.assertTrue(errors)
