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
            "eqiora": WorkspacePackage("eqiora", "crates/eqiora", frozenset()),
            "eqiora-language-server": WorkspacePackage(
                "eqiora-language-server",
                "crates/eqiora-language-server",
                frozenset({"eqiora"}),
            ),
            "eqiora-python": WorkspacePackage(
                "eqiora-python", "crates/eqiora-python", frozenset()
            ),
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

    def test_mixed_python_bindings_keep_the_rust_consumer_closure(self):
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/core/src/lib.rs", "bindings/python/python/eqiora/__init__.pyi",
                 "bindings/python/tests/test_coupled_scalar_example.py",
                 "examples/python/coupled_scalar.py", "examples/README.md",
                 "docs/python/api.md", "README.md"], self.packages
            ),
            ("-p", "consumer", "-p", "core", "-p", "eqiora",
             "-p", "eqiora-language-server", "-p", "eqiora-python"),
        )

    def test_shared_unknown_and_topology_changes_use_the_workspace(self):
        for path in (
            "Cargo.toml",
            "Cargo.lock",
            "crates/core/Cargo.toml",
            ".cargo/config.toml",
            "crates/core/tests/input.json",
            "packages/physics/README.md",
            "crates/eqiora-python/Cargo.toml",
            "packages/physics/src/model.eqi",
            "tools/ci/rust_quality.py",
            "crates/deleted/src/lib.rs",
            "unknown.rs",
            "crates/core/build.rs",
            "rust-toolchain.toml",
            "bindings/python/pyproject.toml",
            "bindings/python/unknown.json",
            "examples/python/unknown.json",
            "examples/unknown.py",
            "examples/steady-flow-past-cylinder.eqi",
            "examples/mixed-boundary-elasticity.eqi",
            "verify/fluid/case/README.md",
            "unknown/README.md",
            "docs/../Cargo.toml",
            "/crates/core/src/lib.rs",
            "crates/core//src/lib.rs",
            "crates/core/./src/lib.rs",
            "crates/core/../leaf/src/lib.rs",
            "crates\\core\\src\\lib.rs",
            "crates/core/src/bad\n.rs",
            "",
            None,
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

    def test_python_example_selects_its_adapter_and_root_docs_add_no_owner(self):
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/leaf/src/lib.rs", "examples/python/exact_cylinder_mesh.py",
                 "README.md", "rfcs/design.md"], self.packages
            ),
            ("-p", "eqiora", "-p", "eqiora-language-server", "-p", "eqiora-python", "-p", "leaf"),
        )

    def test_crate_readme_retains_its_rustdoc_owner(self):
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/leaf/src/lib.rs", "crates/core/README.md"], self.packages
            ),
            ("-p", "consumer", "-p", "core", "-p", "leaf"),
        )

    def test_python_bindings_alone_include_native_source_readers(self):
        for path in ("bindings/python/python/eqiora/__init__.py",
                     "bindings/python/python/eqiora/__init__.pyi",
                     "bindings/python/tests/test_common_plan.py"):
            with self.subTest(path=path):
                self.assertEqual(
                    rust_quality.package_selectors([path], self.packages),
                    ("-p", "eqiora", "-p", "eqiora-language-server", "-p", "eqiora-python"),
                )
        for missing in ("eqiora", "eqiora-python"):
            packages = dict(self.packages)
            del packages[missing]
            self.assertEqual(
                rust_quality.package_selectors(
                    ["bindings/python/python/eqiora/__init__.pyi"], packages
                ),
                ("--workspace",),
            )

    def test_python_source_includes_the_non_cargo_reader_and_its_consumers(self):
        for source in ("src/lib.rs", "src/common_plan/tests.rs", "tests/python_control_plane.rs"):
            with self.subTest(source=source):
                self.assertEqual(
                    rust_quality.package_selectors(
                        [f"crates/eqiora-python/{source}", "docs/python/api.md"],
                        self.packages,
                    ),
                    ("-p", "eqiora", "-p", "eqiora-language-server", "-p", "eqiora-python"),
                )

    def test_python_source_unions_other_changed_package_consumers(self):
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/eqiora-python/src/lib.rs", "crates/core/src/lib.rs"],
                self.packages,
            ),
            (
                "-p", "consumer", "-p", "core", "-p", "eqiora",
                "-p", "eqiora-language-server", "-p", "eqiora-python",
            ),
        )

    def test_missing_python_source_reader_keeps_workspace_checks(self):
        del self.packages["eqiora"]
        self.assertEqual(
            rust_quality.package_selectors(
                ["crates/eqiora-python/src/lib.rs"], self.packages
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

    def test_cli_targets_remain_enabled_only_when_the_facade_is_tested(self):
        for selectors in (("--workspace",), ("-p", "eqiora")):
            command = rust_quality.cargo_command("test", selectors)
            self.assertIn("--features", command)
            self.assertIn("eqiora/cli", command)
        self.assertNotIn(
            "eqiora/cli", rust_quality.cargo_command("test", ("-p", "eqiora-core"))
        )

    def test_build_timings_are_collected_only_for_tests(self):
        for selectors in (("--workspace",), ("-p", "eqiora-core")):
            for check in ("test", "clippy", "doc"):
                self.assertEqual(
                    "--timings" in rust_quality.cargo_command(check, selectors),
                    check == "test",
                )


if __name__ == "__main__":
    unittest.main()
