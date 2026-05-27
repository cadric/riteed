from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from tools import coverage_check
from tools.checks import i18n
from tools.scanners.sites import ReviewEntry, ScanHit, validate_review_links
from tools.scanners.textdomain import textdomain_init_present

REPO_ROOT = Path(__file__).resolve().parents[2]

CODEQL_PATH_ALERT_TRIAGE = (
    {
        "rule": "py/path-injection",
        "path": "tools/checks/dependency_preflight.py",
        "outcome": "false-positive",
        "rationale": "Paths are fixed repo-contract files under a validated app root.",
    },
    {
        "rule": "py/path-injection",
        "path": "tools/checks/release.py",
        "outcome": "contained",
        "rationale": "Policy-relative paths are normalized and contained under the repo root.",
    },
    {
        "rule": "py/path-injection",
        "path": "tools/checks/stress_fuzz.py",
        "outcome": "contained",
        "rationale": "Policy glob candidates reject absolute and parent-traversing paths.",
    },
    {
        "rule": "py/path-injection",
        "path": "tools/validation_tooling.py",
        "outcome": "false-positive",
        "rationale": "Build-output probes iterate paths discovered by repo-local rglob.",
    },
    {
        "rule": "rust/path-injection",
        "path": "app/src/bin/riteed_stress.rs",
        "outcome": "fixed",
        "rationale": "Stress scripts now use parsed fixture paths and repo-relative containment.",
    },
)


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


class PythonCodeQlTriageTests(unittest.TestCase):
    def test_current_codeql_path_alert_triage_is_documented(self) -> None:
        for entry in CODEQL_PATH_ALERT_TRIAGE:
            with self.subTest(path=entry["path"]):
                self.assertIn(entry["outcome"], {"contained", "fixed", "false-positive"})
                self.assertTrue((REPO_ROOT / entry["path"]).exists())
                self.assertGreaterEqual(len(entry["rationale"]), 20)

    def test_textdomain_scanner_detects_current_app_bootstrap(self) -> None:
        source = (REPO_ROOT / "app" / "src" / "lib.rs").read_text(encoding="utf-8")
        self.assertTrue(textdomain_init_present(source))

    def test_textdomain_scanner_accepts_supported_bootstrap_shapes(self) -> None:
        cases = [
            'TextDomain::new(APP_ID).init();',
            'TextDomain::new(APP_ID).locale("C").init();',
            'TextDomain::new(APP_ID).locale("da_DK.UTF-8").init();',
            'TextDomain::new(APP_ID)\n    .locale("da_DK.UTF-8")\n    .init();',
            "fn borrow<'a>(value: &'a str) -> &'a str { value }\nTextDomain::new(APP_ID).init();",
        ]
        for source in cases:
            with self.subTest(source=source):
                self.assertTrue(textdomain_init_present(source))

    def test_textdomain_scanner_rejects_non_bootstrap_shapes(self) -> None:
        cases = [
            'TextDomain::new(APP_ID).locale("C");',
            'NotTextDomain::new(APP_ID).init();',
            '// TextDomain::new(APP_ID).init();',
            '"TextDomain::new(APP_ID).init();"',
        ]
        for source in cases:
            with self.subTest(source=source):
                self.assertFalse(textdomain_init_present(source))

    def test_linguas_locale_token_validation(self) -> None:
        valid = ["da", "en_GB", "pt_BR", "sr@latin", "zh_Hans_CN", "de_AT.UTF-8"]
        invalid = ["", "../etc", "da/../en", "/tmp/da"]
        for locale in valid:
            with self.subTest(locale=locale):
                self.assertTrue(i18n._valid_linguas_locale(locale))
        for locale in invalid:
            with self.subTest(locale=locale):
                self.assertFalse(i18n._valid_linguas_locale(locale))

    def test_linguas_rejects_traversal_locale_before_reading_catalog(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            _write(root / "po" / "LINGUAS", "../etc\n")
            errors: list[str] = []
            i18n.check_linguas_catalogs(root, errors)
        self.assertTrue(any("invalid locale token" in item for item in errors), errors)

    def test_review_link_validation_rejects_unsafe_paths(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            hits: list[ScanHit] = []
            unsafe = ["", ".", "../secret", "/tmp/secret", "C:/secret"]
            for rel in unsafe:
                with self.subTest(rel=rel):
                    entries = [ReviewEntry(path=rel, line=1, kind="runtime-site", match="", source_file="artifact.json", payload={})]
                    errors: list[str] = []
                    validate_review_links(root, hits, entries, errors)
                    self.assertTrue(any("invalid review entry path" in item for item in errors), errors)

    def test_coverage_summary_loader_requires_valid_json_object(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            summary = root / "coverage.json"
            _write(summary, json.dumps({"totals": {"lines": {"percent": 100.0}}}))
            self.assertEqual(coverage_check.load_json_summary(str(summary))["totals"]["lines"]["percent"], 100.0)

            bad = root / "bad.json"
            _write(bad, "[]")
            with self.assertRaises(SystemExit):
                coverage_check.load_json_summary(str(bad))
            with self.assertRaises(SystemExit):
                coverage_check.load_json_summary(str(root / "missing.json"))


if __name__ == "__main__":
    unittest.main()
