from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

from local_verify import (  # noqa: E402
    WorkspacePackage,
    build_plan,
    changed_case_ids,
    direct_packages,
    local_changed_paths,
    reverse_dependency_closure,
)
from check_docs import check as check_docs  # noqa: E402


def workspace() -> dict[str, WorkspacePackage]:
    return {
        "eqiora-core": WorkspacePackage("eqiora-core", "crates/eqiora-core", frozenset()),
        "eqiora-compiler": WorkspacePackage(
            "eqiora-compiler", "crates/eqiora-compiler", frozenset({"eqiora-core"})
        ),
        "eqiora": WorkspacePackage(
            "eqiora", "crates/eqiora", frozenset({"eqiora-compiler"})
        ),
        "xtask": WorkspacePackage("xtask", "tools/xtask", frozenset()),
    }


class PackageSelectionTests(unittest.TestCase):
    def test_local_change_set_unions_every_git_state(self) -> None:
        with mock.patch(
            "local_verify._git_paths",
            side_effect=[
                {"committed"},
                {"unstaged", "shared"},
                {"staged", "shared"},
                {"untracked"},
            ],
        ):
            self.assertEqual(
                local_changed_paths("base"),
                ("committed", "shared", "staged", "unstaged", "untracked"),
            )

    def test_direct_owner_and_root_manifest_are_fail_closed(self) -> None:
        packages = workspace()
        self.assertEqual(
            direct_packages(["crates/eqiora-core/src/lib.rs"], packages),
            {"eqiora-core"},
        )
        self.assertEqual(direct_packages(["Cargo.lock"], packages), set(packages))

    def test_reverse_dependency_closure_is_transitive(self) -> None:
        self.assertEqual(
            reverse_dependency_closure({"eqiora-core"}, workspace()),
            {"eqiora-core", "eqiora-compiler", "eqiora"},
        )

    def test_changed_verification_paths_select_their_case(self) -> None:
        self.assertEqual(
            changed_case_ids(
                [
                    "verify/packages/example/models/model.eqi",
                    "verify/packages/example/README.md",
                    "docs/architecture.md",
                ]
            ),
            {"packages.example"},
        )


class PlanTests(unittest.TestCase):
    def test_periodic_msrv_checks_every_production_feature(self) -> None:
        plan = build_plan("periodic", [], [], workspace())
        msrv = next(item for item in plan.commands if item.label == "MSRV")
        self.assertEqual(msrv.argv[0:2], ("cargo", "+1.89.0"))
        self.assertIn("--all-targets", msrv.argv)
        self.assertIn("--all-features", msrv.argv)
        self.assertIn("--locked", msrv.argv)
        studio = next(
            item for item in plan.commands if item.label == "Studio native MSRV"
        )
        self.assertEqual(studio.argv[0:2], ("cargo", "+1.89.0"))
        self.assertIn("studio/src-tauri/Cargo.toml", studio.argv)
        self.assertIn("--all-targets", studio.argv)
        studio_format = next(
            item for item in plan.commands if item.label == "Studio native formatting"
        )
        self.assertEqual(
            studio_format.argv,
            (
                "cargo",
                "+stable",
                "fmt",
                "--manifest-path",
                "studio/src-tauri/Cargo.toml",
                "--",
                "--check",
            ),
        )

    def test_fast_plan_keeps_direct_package_and_explicit_case(self) -> None:
        plan = build_plan(
            "fast",
            ["crates/eqiora-core/src/lib.rs"],
            ["language.explicit"],
            workspace(),
        )
        self.assertEqual(plan.packages, ("eqiora-core",))
        self.assertEqual(plan.cases, ("language.explicit",))
        rendered = [item.render() for item in plan.commands]
        self.assertTrue(any("cargo test --locked -p eqiora-core" in item for item in rendered))
        self.assertTrue(any("--case language.explicit" in item for item in rendered))

    def test_affected_plan_expands_packages_and_adds_rustdoc(self) -> None:
        plan = build_plan(
            "affected",
            ["crates/eqiora-core/src/lib.rs"],
            ["language.explicit"],
            workspace(),
        )
        self.assertEqual(plan.packages, ("eqiora", "eqiora-compiler", "eqiora-core"))
        self.assertEqual(plan.cases, ("language.explicit",))
        self.assertTrue(any(item.label == "Rustdoc" for item in plan.commands))
        self.assertTrue(
            any(item.label == "Evidence manifest inventory" for item in plan.commands)
        )
        rendered = [item.render() for item in plan.commands]
        self.assertFalse(any("--all-features" in item for item in rendered))
        self.assertTrue(any("--case language.explicit" in item for item in rendered))

    def test_affected_plan_does_not_infer_semantic_cases_from_executor_crates(self) -> None:
        plan = build_plan(
            "affected",
            ["crates/eqiora-core/src/lib.rs"],
            [],
            workspace(),
        )
        self.assertEqual(plan.cases, ())
        labels = {item.label for item in plan.commands}
        self.assertIn("Evidence manifest inventory", labels)
        self.assertNotIn("All registered evidence", labels)
        self.assertFalse(any(label.startswith("Registered evidence ") for label in labels))

    def test_ci_infrastructure_change_reuses_fail_closed_surface_mapping(self) -> None:
        plan = build_plan(
            "affected",
            ["tools/ci/local_verify.py"],
            [],
            workspace(),
        )
        labels = {item.label for item in plan.commands}
        self.assertEqual(plan.packages, tuple(sorted(workspace())))
        self.assertIn("CI contract tests", labels)
        self.assertIn("Root dependency policy", labels)
        self.assertIn("Studio dependency policy", labels)
        self.assertIn("Python isolated wheel and tests", labels)
        self.assertIn("Studio native formatting", labels)
        self.assertIn("Studio unit tests", labels)
        studio_e2e = next(
            item for item in plan.commands if item.label == "Studio interaction tests"
        )
        self.assertEqual(studio_e2e.argv[-2:], ("--", "--workers=1"))
        self.assertIn("Evidence manifest inventory", labels)
        self.assertNotIn("All registered evidence", labels)
        self.assertFalse(any(label.startswith("Registered evidence ") for label in labels))
        python_commands = [
            item for item in plan.commands if item.label.startswith("Python ")
        ]
        self.assertEqual(len(python_commands), 1)
        self.assertEqual(
            {item.argv[0] for item in python_commands},
            {sys.executable},
        )
        self.assertFalse(
            any("maturin develop" in item.render() for item in python_commands)
        )

    def test_studio_lock_change_runs_both_dependency_policies(self) -> None:
        plan = build_plan(
            "affected",
            ["studio/src-tauri/Cargo.lock"],
            [],
            workspace(),
        )
        dependency_commands = {
            item.label: item.render()
            for item in plan.commands
            if item.label.endswith("dependency policy")
        }
        self.assertEqual(
            set(dependency_commands),
            {"Root dependency policy", "Studio dependency policy"},
        )
        self.assertEqual(
            dependency_commands["Root dependency policy"],
            "cargo deny --locked check",
        )
        self.assertIn(
            "cargo deny --all-features --locked --manifest-path "
            "studio/src-tauri/Cargo.toml --config "
            "studio/src-tauri/deny.toml check",
            dependency_commands["Studio dependency policy"],
        )

    def test_adapter_dependency_identity_inputs_schedule_ci_contracts(self) -> None:
        inputs = [
            "Cargo.toml",
            "Cargo.lock",
            "crates/eqiora-backend-faer/Cargo.toml",
            "crates/eqiora-backend-faer/src/lib.rs",
            "crates/eqiora-backend-rayon/Cargo.toml",
            "crates/eqiora-backend-rayon/src/lib.rs",
        ]
        for path in inputs:
            with self.subTest(path=path):
                plan = build_plan("affected", [path], [], workspace())
                labels = {item.label for item in plan.commands}
                self.assertIn("CI contract tests", labels)


class LocalDocumentationTests(unittest.TestCase):
    def test_dependency_trees_are_outside_the_documentation_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "docs/capability-matrix.md").write_text("matrix\n", encoding="utf-8")
            for name in ("README.md", "AGENTS.md", "CONTRIBUTING.md"):
                (root / name).write_text(
                    "[matrix](docs/capability-matrix.md)\n", encoding="utf-8"
                )
            dependency = root / "studio/node_modules/dependency"
            dependency.mkdir(parents=True)
            (dependency / "README.md").write_text(
                "[missing](not-shipped.md)\n", encoding="utf-8"
            )
            self.assertEqual(check_docs(root), [])


if __name__ == "__main__":
    unittest.main()
