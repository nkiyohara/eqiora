from __future__ import annotations

import io
import json
import sys
import unittest
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

from check_trust_boundary import (  # noqa: E402
    changed_file_names,
    fetch_changed_files,
    protected_changes,
    protected_path,
)


class Response(io.BytesIO):
    def __enter__(self) -> "Response":
        return self

    def __exit__(self, *arguments: object) -> None:
        self.close()


class ProtectedPathTests(unittest.TestCase):
    def test_merge_and_release_definitions_are_protected(self) -> None:
        for path in (
            "CODEOWNERS",
            ".github/CODEOWNERS",
            ".github/actions/check/action.yml",
            ".github/workflows/ci.yml",
            "crates/eqiora-verify/src/lib.rs",
            "deny.toml",
            "studio/src-tauri/deny.toml",
            "tools/ci/check_gate.py",
            "tools/release/python_candidate.py",
            "tools/xtask/src/main.rs",
        ):
            with self.subTest(path=path):
                self.assertTrue(protected_path(path))

    def test_product_and_documentation_paths_are_not_protected(self) -> None:
        for path in (
            "Cargo.toml",
            "crates/eqiora/src/lib.rs",
            "docs/architecture.md",
            "pyproject.toml",
        ):
            with self.subTest(path=path):
                self.assertFalse(protected_path(path))

    def test_rename_checks_both_names(self) -> None:
        paths = changed_file_names(
            [
                {
                    "filename": "docs/old-ci.md",
                    "previous_filename": "tools/ci/old_gate.py",
                }
            ]
        )
        self.assertEqual(protected_changes(paths), ["tools/ci/old_gate.py"])


class MetadataFetchTests(unittest.TestCase):
    def test_fetches_all_pages_with_read_only_headers(self) -> None:
        first = [{"filename": f"docs/page-{index}.md"} for index in range(100)]
        second = [{"filename": ".github/workflows/ci.yml"}]
        opener = mock.Mock(
            side_effect=[
                Response(json.dumps(first).encode()),
                Response(json.dumps(second).encode()),
            ]
        )

        paths = fetch_changed_files(
            api_url="https://api.github.test",
            repository="owner/project",
            pull_number=7,
            expected_file_count=101,
            token="not-a-real-token",
            opener=opener,
        )

        self.assertEqual(len(paths), 101)
        self.assertEqual(paths[-1], ".github/workflows/ci.yml")
        requests = [call.args[0] for call in opener.call_args_list]
        self.assertTrue(requests[0].full_url.endswith("per_page=100&page=1"))
        self.assertTrue(requests[1].full_url.endswith("per_page=100&page=2"))
        self.assertEqual(
            requests[0].headers["Authorization"], "Bearer not-a-real-token"
        )

    def test_rejects_api_truncation_and_oversized_pull_requests(self) -> None:
        truncated = mock.Mock(
            return_value=Response(
                json.dumps(
                    [{"filename": f"docs/page-{index}.md"} for index in range(99)]
                ).encode()
            )
        )
        with self.assertRaisesRegex(ValueError, "count differs"):
            fetch_changed_files(
                api_url="https://api.github.test",
                repository="owner/project",
                pull_number=7,
                expected_file_count=100,
                token="not-a-real-token",
                opener=truncated,
            )

        unopened = mock.Mock()
        with self.assertRaisesRegex(ValueError, "3000-file API trust boundary"):
            fetch_changed_files(
                api_url="https://api.github.test",
                repository="owner/project",
                pull_number=7,
                expected_file_count=3001,
                token="not-a-real-token",
                opener=unopened,
            )
        unopened.assert_not_called()

    def test_malformed_metadata_fails_closed(self) -> None:
        with self.assertRaisesRegex(ValueError, "no filename"):
            changed_file_names([{"status": "modified"}])


if __name__ == "__main__":
    unittest.main()
