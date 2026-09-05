"""Focused tests for conservative hosted Rust package selection."""

import json
import os
import sys
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import rust_quality
from local_verify import WorkspacePackage


class RustQualityTests(unittest.TestCase):
    def setUp(self):
        self.packages = {
            "core": WorkspacePackage("core", "crates/core", frozenset()),
            "consumer": WorkspacePackage(
                "consumer", "crates/consumer", frozenset({"core"})
            ),
            "leaf": WorkspacePackage("leaf", "crates/leaf", frozenset()),
        }

    def test_reverse_closure_includes_consumers_but_not_unrelated_packages(self):
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/core/src/lib.rs", "docs/guide.md"], self.packages
            ),
            ("-p", "consumer", "-p", "core"),
        )
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/leaf/tests/feature.rs"], self.packages
            ),
            ("-p", "leaf"),
        )

    def test_shared_unknown_and_topology_changes_use_the_workspace(self):
        for path in (
            "Cargo.toml",
            "Cargo.lock",
            "crates/core/Cargo.toml",
            ".cargo/config.toml",
            "crates/core/tests/input.json",
            "packages/physics/src/model.eqi",
            "tools/ci/rust_quality.py",
            "crates/deleted/src/lib.rs",
            "unknown.rs",
        ):
            with self.subTest(path=path):
                self.assertEqual(
                    rust_quality.package_selectors(
                        ["crates/leaf/src/lib.rs", path], self.packages
                    ),
                    ("--workspace",),
                )
        for paths, unsafe in (
            ([], False),
            (["docs/guide.md"], False),
            (["crates/leaf/src/lib.rs"], True),
        ):
            self.assertEqual(
                rust_quality.package_selectors(
                    paths, self.packages, unsafe_mode=unsafe
                ),
                ("--workspace",),
            )

    def test_exact_event_commits_bind_the_merge_base_diff(self):
        event = {"pull_request": {"base": {"sha": "a" * 40}, "head": {"sha": "b" * 40}}}
        with (
            mock.patch.dict(
                os.environ,
                {
                    "GITHUB_EVENT_NAME": "pull_request",
                    "GITHUB_EVENT_PATH": "event.json",
                },
            ),
            mock.patch.object(Path, "read_text", return_value=json.dumps(event)),
            mock.patch.object(
                rust_quality.subprocess, "check_output", return_value="b" * 40 + "\n"
            ) as head,
            mock.patch.object(
                rust_quality,
                "changed_paths",
                return_value=(["crates/leaf/src/lib.rs"], False),
            ) as diff,
            mock.patch.object(
                rust_quality, "load_workspace", return_value=self.packages
            ),
        ):
            self.assertEqual(rust_quality.hosted_selectors(), ("-p", "leaf"))
            diff.assert_called_once_with("a" * 40, "b" * 40)
            head.return_value = "c" * 40
            self.assertEqual(rust_quality.hosted_selectors(), ("--workspace",))

    def test_manual_or_missing_authority_keeps_full_checks(self):
        with mock.patch.dict(
            os.environ, {"GITHUB_EVENT_NAME": "workflow_dispatch"}, clear=True
        ):
            self.assertEqual(rust_quality.hosted_selectors(), ("--workspace",))
        with mock.patch.dict(
            os.environ, {"GITHUB_EVENT_NAME": "pull_request"}, clear=True
        ):
            self.assertEqual(rust_quality.hosted_selectors(), ("--workspace",))

    def test_commands_preserve_check_options_and_locked_resolution(self):
        for check in ("clippy", "test", "doc"):
            command = rust_quality.cargo_command(check, ("-p", "leaf"))
            self.assertEqual(
                command[:6], ["cargo", "+stable", check, "--locked", "-p", "leaf"]
            )
        self.assertEqual(
            rust_quality.cargo_command("test", ("--workspace",))[-1], "--all-targets"
        )
        self.assertEqual(
            rust_quality.cargo_command("clippy", ("--workspace",))[-6:],
            ["--all-targets", "--all-features", "--keep-going", "--", "-D", "warnings"],
        )


if __name__ == "__main__":
    unittest.main()
