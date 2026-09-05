from __future__ import annotations

import shutil
import tempfile
import unittest
from pathlib import Path

from tools.checks import runtime


REPO_ROOT = Path(__file__).resolve().parents[2]


def _runtime_errors(source: str) -> list[str]:
    with tempfile.TemporaryDirectory() as tmpdir:
        root = Path(tmpdir)
        shutil.copytree(REPO_ROOT / "policy", root / "policy")
        path = root / "src" / "probe.rs"
        path.parent.mkdir(parents=True)
        path.write_text(source, encoding="utf-8")
        errors: list[str] = []
        runtime.check_runtime(root, errors)
        return errors


def _errors_with(errors: list[str], text: str) -> list[str]:
    return [error for error in errors if text in error]


class RustScannerHardeningTests(unittest.TestCase):
    def test_commented_cfg_test_does_not_hide_production_sync_fs(self) -> None:
        errors = _runtime_errors(
            "// #[cfg(test)]\n"
            "pub fn probe(path: &std::path::Path) -> bool {\n"
            "    path.exists()\n"
            "}\n"
        )

        self.assertEqual(
            _errors_with(errors, "runtime-sync-fs"),
            ["src/probe.rs:3: missing review entry for kind 'runtime-sync-fs'"],
        )

    def test_cfg_any_test_or_feature_remains_production_scanned(self) -> None:
        errors = _runtime_errors(
            "#[cfg(any(test, feature = \"stress\"))]\n"
            "fn probe(path: &std::path::Path) -> bool {\n"
            "    path.is_file()\n"
            "}\n"
        )

        self.assertEqual(
            _errors_with(errors, "runtime-sync-fs"),
            ["src/probe.rs:3: missing review entry for kind 'runtime-sync-fs'"],
        )

    def test_multiline_cfg_test_ignores_lexically_nested_test_item_only(self) -> None:
        errors = _runtime_errors(
            "#[cfg(\n"
            "    test\n"
            ")]\n"
            "mod tests {\n"
            "    const TEXT: &str = r#\"} /* #[cfg(any(test, feature = \\\"stress\\\"))] */\"#;\n"
            "    /* outer { /* nested } */ still outer } */\n"
            "    #[cfg(test)]\n"
            "    mod nested {\n"
            "        fn nested_probe(path: &std::path::Path) -> bool {\n"
            "            path.is_file()\n"
            "        }\n"
            "    }\n"
            "    fn test_probe(path: &std::path::Path) -> bool {\n"
            "        path.exists()\n"
            "    }\n"
            "}\n"
            "pub fn production_probe(path: &std::path::Path) -> bool {\n"
            "    path.is_dir()\n"
            "}\n"
        )

        self.assertEqual(
            _errors_with(errors, "runtime-sync-fs"),
            ["src/probe.rs:18: missing review entry for kind 'runtime-sync-fs'"],
        )

    def test_sync_fs_text_in_comments_and_strings_is_not_a_review_site(self) -> None:
        errors = _runtime_errors(
            "fn probe() {\n"
            "    let _standard = \"path.exists() and // still a string\";\n"
            "    let _raw = r###\"std::fs::read_dir(\\\"/tmp\\\") { }\"###;\n"
            "    // path.is_dir()\n"
            "    /* std::fs::metadata(\"/tmp\") */\n"
            "}\n"
        )

        self.assertEqual(_errors_with(errors, "runtime-sync-fs"), [])

    def test_blocking_call_deep_inside_async_fn_is_rejected(self) -> None:
        errors = _runtime_errors(
            "pub async fn probe() {\n"
            + "    // ordinary documentation\n" * 20
            + "    std::thread::sleep(std::time::Duration::from_secs(1));\n"
            + "}\n"
        )

        self.assertEqual(
            _errors_with(errors, "blocking calls inside async contexts"),
            [
                "src/probe.rs:22: blocking calls inside async contexts require reviewed offloading"
            ],
        )

    def test_blocking_call_after_async_block_is_not_rejected(self) -> None:
        errors = _runtime_errors(
            "fn probe() {\n"
            "    let _future = async move {\n"
            "        ready().await;\n"
            "    };\n"
            "    std::thread::sleep(std::time::Duration::from_secs(1));\n"
            "}\n"
        )

        self.assertEqual(_errors_with(errors, "blocking calls inside async contexts"), [])

    def test_blocking_call_inside_nested_async_move_block_is_rejected(self) -> None:
        errors = _runtime_errors(
            "fn probe() {\n"
            "    let _future = async\n"
            "        move\n"
            "    {\n"
            "        if ready() {\n"
            "            std::thread::sleep(std::time::Duration::from_secs(1));\n"
            "        }\n"
            "    };\n"
            "}\n"
        )

        self.assertEqual(
            _errors_with(errors, "blocking calls inside async contexts"),
            [
                "src/probe.rs:6: blocking calls inside async contexts require reviewed offloading"
            ],
        )

    def test_async_text_in_comments_and_strings_does_not_create_async_scope(self) -> None:
        errors = _runtime_errors(
            "fn probe() {\n"
            "    let _text = r#\"async move {\"#;\n"
            "    // async fn imaginary() {\n"
            "    std::thread::sleep(std::time::Duration::from_secs(1));\n"
            "}\n"
        )

        self.assertEqual(_errors_with(errors, "blocking calls inside async contexts"), [])

    def test_raw_async_identifier_does_not_create_async_scope(self) -> None:
        errors = _runtime_errors(
            "fn probe() {\n"
            "    let _value = r#async {\n"
            "        field: {\n"
            "            std::thread::sleep(std::time::Duration::from_secs(1));\n"
            "            1\n"
            "        },\n"
            "    };\n"
            "}\n"
        )

        self.assertEqual(_errors_with(errors, "blocking calls inside async contexts"), [])


if __name__ == "__main__":
    unittest.main()
