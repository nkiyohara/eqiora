from __future__ import annotations

import contextlib
import importlib
import inspect
import io
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from unittest import mock


CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

from local_verify import (  # noqa: E402
    PlannedCommand,
    ResourceBudget,
    ResourceRequest,
    VerificationFailure,
    VerificationLane,
    VerificationPlan,
    WorkspacePackage,
    build_plan,
    changed_case_ids,
    direct_packages,
    local_changed_paths,
    reverse_dependency_closure,
    run_plan,
)
from check_docs import check as check_docs  # noqa: E402
from verification_scheduler import (  # noqa: E402
    _available_memory_mib,
    _scratch_base,
    cpu_allocations,
)


EVIDENCE_RUN_PREFIX = (
    "cargo",
    "run",
    "--locked",
    "-p",
    "eqiora-verify",
    "--",
    "run",
)


def evidence_runs(plan: VerificationPlan) -> list[PlannedCommand]:
    return [
        item
        for item in plan.commands
        if item.argv[: len(EVIDENCE_RUN_PREFIX)] == EVIDENCE_RUN_PREFIX
    ]


def workspace() -> dict[str, WorkspacePackage]:
    return {
        "eqiora-core": WorkspacePackage(
            "eqiora-core", "crates/eqiora-core", frozenset()
        ),
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
        self.assertTrue(
            any("cargo test --locked -p eqiora-core" in item for item in rendered)
        )
        evidence = evidence_runs(plan)
        self.assertEqual(len(evidence), 1)
        self.assertEqual(evidence[0].label, "Registered evidence (1 case)")
        self.assertEqual(
            evidence[0].argv,
            (*EVIDENCE_RUN_PREFIX, "--case", "language.explicit"),
        )

    def test_selected_cases_share_one_canonical_runner_invocation(self) -> None:
        first = build_plan(
            "fast",
            [],
            ["language.second", "language.first", "language.second"],
            workspace(),
        )
        second = build_plan(
            "fast",
            [],
            ["language.first", "language.second"],
            workspace(),
        )

        self.assertEqual(first.cases, ("language.first", "language.second"))
        evidence = evidence_runs(first)
        self.assertEqual(len(evidence), 1)
        self.assertEqual(evidence[0].label, "Registered evidence (2 cases)")
        self.assertEqual(
            evidence[0].argv,
            (
                *EVIDENCE_RUN_PREFIX,
                "--case",
                "language.first",
                "--case",
                "language.second",
            ),
        )
        self.assertEqual(first, second)

    def test_no_selected_case_schedules_no_runner_invocation(self) -> None:
        plan = build_plan("fast", [], [], workspace())
        self.assertEqual(evidence_runs(plan), [])

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

    def test_affected_plan_does_not_infer_semantic_cases_from_executor_crates(
        self,
    ) -> None:
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
        self.assertFalse(
            any(label.startswith("Registered evidence ") for label in labels)
        )

    def assert_documentation_contract(
        self, tier: str, changed_paths: list[str]
    ) -> None:
        checker_paths = {
            "tools/ci/check_docs.py",
            "tools/ci/check_public_release_tree.py",
        }
        plan = build_plan(tier, changed_paths, [], workspace())
        documentation_checks = tuple(
            (index, item)
            for index, item in enumerate(plan.commands)
            if len(item.argv) > 1 and item.argv[1] in checker_paths
        )
        self.assertEqual(
            tuple(item.argv for _index, item in documentation_checks),
            (
                (sys.executable, "tools/ci/check_docs.py", "."),
                (sys.executable, "tools/ci/check_public_release_tree.py", "."),
            ),
        )
        docs_index, docs_check = documentation_checks[0]
        public_tree_index, public_tree_check = documentation_checks[1]
        self.assertEqual(public_tree_index, docs_index + 1)
        self.assertEqual(
            (docs_check.label, public_tree_check.label),
            ("Documentation contract", "Public release tree"),
        )
        self.assertEqual(docs_check.lane.name, "repository")
        self.assertIs(public_tree_check.lane, docs_check.lane)

    def test_fast_documentation_contract_runs_both_hosted_checks_once(self) -> None:
        self.assert_documentation_contract("fast", ["README.md"])

    def test_affected_documentation_contract_runs_both_hosted_checks_once(self) -> None:
        self.assert_documentation_contract("affected", ["README.md"])

    def test_periodic_documentation_contract_runs_both_hosted_checks_once(
        self,
    ) -> None:
        self.assert_documentation_contract("periodic", [])

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
        self.assertFalse(
            any(label.startswith("Registered evidence ") for label in labels)
        )
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
            "pyproject.toml",
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

    def test_ci_infrastructure_dependency_policies_share_one_lane(self) -> None:
        plan = build_plan(
            "affected",
            ["tools/ci/local_verify.py"],
            [],
            workspace(),
        )
        lanes = {item.label: item.lane for item in plan.commands}
        self.assertEqual(lanes["Root dependency policy"].name, "dependency-policy")
        self.assertEqual(
            lanes["Root dependency policy"], lanes["Studio dependency policy"]
        )
        self.assertEqual(lanes["Rust tests"].name, "root-cargo")
        self.assertEqual(lanes["Studio unit tests"].name, "studio")
        self.assertNotIn(
            lanes["Root dependency policy"].name,
            {lanes["Rust tests"].name, lanes["Studio unit tests"].name},
        )


class Issue496UnixSocketScratchTests(unittest.TestCase):
    SYNTHETIC_HOME = Path("/home/user1")
    SOCKET_RELATIVE = Path("eqiora-cli-filesystem-4294967295-8") / "socket"
    UNIX_PATHNAME_MAX = 107

    @staticmethod
    def plan(*commands: PlannedCommand) -> VerificationPlan:
        return VerificationPlan("affected", (), (), (), commands, ())

    @staticmethod
    def lane(name: str) -> VerificationLane:
        return VerificationLane(name, ResourceRequest(1, 1))

    @classmethod
    def socket_candidate(cls, tmpdir: Path) -> Path:
        return tmpdir / cls.SOCKET_RELATIVE

    @classmethod
    def utf8_boundary_paths(cls) -> tuple[Path, Path, Path]:
        configured_root = cls.SYNTHETIC_HOME / "é"
        tmpdir_107 = configured_root / ("a" * 50)
        tmpdir_108 = configured_root / ("a" * 51)
        return configured_root, tmpdir_107, tmpdir_108

    def test_00_default_path_plan_is_host_neutral_and_bounded(self) -> None:
        observed: list[dict[str, str]] = []

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            observed.append(dict(kwargs["env"]))
            return subprocess.CompletedProcess(argv, 0)

        plan = self.plan(
            PlannedCommand(
                "default path witness",
                ("default-path-witness",),
                lane=self.lane("root-cargo"),
            )
        )
        home = Path.home().resolve()
        with tempfile.TemporaryDirectory(
            prefix="issue496-worktree-", dir=home
        ) as directory:
            worktree = Path(directory) / ("w" * 120)
            worktree.mkdir()
            base = _scratch_base(worktree, None)
            self.addCleanup(shutil.rmtree, base, True)
            with mock.patch(
                "verification_scheduler.subprocess.run", side_effect=execute
            ) as child:
                run_plan(plan, worktree, budget=ResourceBudget(1, 1))

        child.assert_called_once()
        self.assertEqual(len(observed), 1)
        tmpdir = Path(observed[0]["TMPDIR"])
        self.assertTrue(tmpdir.is_absolute())
        self.assertTrue(tmpdir.resolve().is_relative_to(home))
        self.assertFalse(tmpdir.exists())
        self.assertNotIn(worktree.name, str(tmpdir))
        lexical_relative = tmpdir.relative_to(Path.home())
        synthetic_tmpdir = self.SYNTHETIC_HOME / lexical_relative
        candidate = self.socket_candidate(synthetic_tmpdir)
        self.assertEqual(len(os.fsencode(self.SYNTHETIC_HOME)), 11)
        self.assertLessEqual(
            len(os.fsencode(candidate)),
            self.UNIX_PATHNAME_MAX,
            f"default TMPDIR shape leaves a {len(os.fsencode(candidate))}-byte "
            "Unix-socket candidate",
        )

    def test_01_default_positive_binds_once_and_cleans_tmpdir(self) -> None:
        observed_tmpdirs: list[Path] = []

        def bind_socket(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            tmpdir = Path(dict(kwargs["env"])["TMPDIR"])
            observed_tmpdirs.append(tmpdir)
            self.assertTrue(tmpdir.exists())
            socket_path = self.socket_candidate(tmpdir)
            socket_path.parent.mkdir()
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as listener:
                listener.bind(str(socket_path))
            return subprocess.CompletedProcess(argv, 0)

        plan = self.plan(
            PlannedCommand(
                "default socket bind",
                ("bind-default-socket",),
                lane=self.lane("root-cargo"),
            )
        )
        with tempfile.TemporaryDirectory(
            prefix="issue496-worktree-", dir=Path.home()
        ) as directory:
            worktree = Path(directory) / ("w" * 120)
            worktree.mkdir()
            base = _scratch_base(worktree, None)
            self.addCleanup(shutil.rmtree, base, True)
            with mock.patch(
                "verification_scheduler.subprocess.run", side_effect=bind_socket
            ) as child:
                run_plan(plan, worktree, budget=ResourceBudget(1, 1))

        child.assert_called_once()
        self.assertEqual(len(observed_tmpdirs), 1)
        tmpdir = observed_tmpdirs[0]
        self.assertTrue(tmpdir.resolve().is_relative_to(Path.home().resolve()))
        self.assertLessEqual(
            len(os.fsencode(self.socket_candidate(tmpdir))),
            self.UNIX_PATHNAME_MAX,
        )
        self.assertFalse(tmpdir.exists())

    def test_02_two_lanes_get_disjoint_tmp_and_persistent_cargo_roots(self) -> None:
        barrier = threading.Barrier(2)
        environments: dict[str, dict[str, str]] = {}
        environment_lock = threading.Lock()

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            environment = dict(kwargs["env"])
            with environment_lock:
                environments[argv[0]] = environment
            self.assertTrue(Path(environment["TMPDIR"]).exists())
            barrier.wait(timeout=1.0)
            return subprocess.CompletedProcess(argv, 0)

        plan = self.plan(
            PlannedCommand("first lane", ("first",), lane=self.lane("first")),
            PlannedCommand("second lane", ("second",), lane=self.lane("second")),
        )
        with (
            tempfile.TemporaryDirectory(prefix="e496-", dir=Path.home()) as directory,
            mock.patch("verification_scheduler.subprocess.run", side_effect=execute),
        ):
            scratch_root = Path(directory)
            run_plan(
                plan,
                scratch_root,
                budget=ResourceBudget(2, 2),
                scratch_root=scratch_root,
            )
            self.assertEqual(set(environments), {"first", "second"})
            tmpdirs = {Path(env["TMPDIR"]) for env in environments.values()}
            lane_roots = {
                Path(env["EQIORA_VERIFY_LANE_ROOT"]) for env in environments.values()
            }
            cargo_roots = {
                Path(env["CARGO_TARGET_DIR"]) for env in environments.values()
            }
            self.assertEqual(len(tmpdirs), 2)
            self.assertEqual(len(lane_roots), 2)
            self.assertEqual(len(cargo_roots), 2)
            self.assertTrue(all(not path.exists() for path in tmpdirs))
            self.assertTrue(all(path.exists() for path in cargo_roots))
            for environment in environments.values():
                lane_root = Path(environment["EQIORA_VERIFY_LANE_ROOT"])
                self.assertTrue(lane_root.is_relative_to(scratch_root))
                self.assertTrue(
                    Path(environment["TMPDIR"]).is_relative_to(scratch_root)
                )
                self.assertTrue(
                    Path(environment["CARGO_TARGET_DIR"]).is_relative_to(lane_root)
                )

    def test_03_overlapping_same_lane_invocations_keep_distinct_tmpdirs(self) -> None:
        observed_tmpdirs: list[Path] = []
        observed_lock = threading.Lock()

        def assert_both_allocations_exist() -> None:
            with observed_lock:
                self.assertEqual(len(observed_tmpdirs), 2)
                self.assertEqual(len(set(observed_tmpdirs)), 2)
                self.assertTrue(all(path.exists() for path in observed_tmpdirs))

        barrier = threading.Barrier(2, action=assert_both_allocations_exist)

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            tmpdir = Path(dict(kwargs["env"])["TMPDIR"])
            with observed_lock:
                observed_tmpdirs.append(tmpdir)
            self.assertTrue(tmpdir.exists())
            barrier.wait(timeout=1.0)
            return subprocess.CompletedProcess(argv, 0)

        lane = self.lane("shared")
        plan = self.plan(PlannedCommand("shared lane", ("shared",), lane=lane))
        with (
            tempfile.TemporaryDirectory(prefix="e496-", dir=Path.home()) as directory,
            mock.patch("verification_scheduler.subprocess.run", side_effect=execute),
        ):
            scratch_root = Path(directory)

            def invoke() -> None:
                run_plan(
                    plan,
                    scratch_root,
                    budget=ResourceBudget(1, 1),
                    scratch_root=scratch_root,
                )

            with ThreadPoolExecutor(max_workers=2) as pool:
                futures = [pool.submit(invoke) for _ in range(2)]
                for future in futures:
                    future.result(timeout=2.0)

            self.assertEqual(len(observed_tmpdirs), 2)
            self.assertEqual(len(set(observed_tmpdirs)), 2)
            self.assertTrue(all(not path.exists() for path in observed_tmpdirs))

    def test_04_old_default_shape_exceeds_the_frozen_socket_budget(self) -> None:
        home = self.SYNTHETIC_HOME
        base = home / ".cache" / "eqiora" / "local-verify" / ("0" * 16)
        old_tmpdir = base / "lanes" / f"root-cargo-{'0' * 8}" / "tmp" / f"run-{'0' * 8}"
        candidate = self.socket_candidate(old_tmpdir)
        self.assertEqual(len(os.fsencode(old_tmpdir)), 98)
        self.assertEqual(len(os.fsencode(candidate)), 140)
        self.assertGreater(len(os.fsencode(candidate)), self.UNIX_PATHNAME_MAX)
        self.assertEqual(len(os.fsencode("/" + self.SOCKET_RELATIVE.as_posix())), 42)

    def test_05_utf8_107_byte_candidate_is_admitted_then_cleaned(self) -> None:
        configured_root, tmpdir_107, _tmpdir_108 = self.utf8_boundary_paths()
        candidate = self.socket_candidate(tmpdir_107)
        self.assertEqual(len(str(candidate)), 106)
        self.assertEqual(len(os.fsencode(candidate)), 107)
        present: set[Path] = set()

        def allocate(*_args: object, **_kwargs: object) -> str:
            present.add(tmpdir_107)
            return str(tmpdir_107)

        def remove(path: str | Path, *_args: object, **_kwargs: object) -> None:
            present.discard(Path(path))

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            self.assertEqual(Path(dict(kwargs["env"])["TMPDIR"]), tmpdir_107)
            self.assertIn(tmpdir_107, present)
            return subprocess.CompletedProcess(argv, 0)

        plan = self.plan(
            PlannedCommand("107-byte witness", ("admitted",), lane=self.lane("utf8"))
        )

        def open_log(
            *_args: object, **_kwargs: object
        ) -> contextlib.AbstractContextManager[io.BytesIO]:
            return contextlib.nullcontext(io.BytesIO())

        with (
            mock.patch(
                "verification_scheduler.Path.home", return_value=self.SYNTHETIC_HOME
            ),
            mock.patch("verification_scheduler.Path.mkdir"),
            mock.patch("verification_scheduler.Path.open", side_effect=open_log),
            mock.patch(
                "verification_scheduler.tempfile.TemporaryDirectory",
                return_value=contextlib.nullcontext(str(configured_root / "logs")),
            ),
            mock.patch("verification_scheduler.tempfile.mkdtemp", side_effect=allocate),
            mock.patch("verification_scheduler.shutil.rmtree", side_effect=remove),
            mock.patch("verification_scheduler._emit_reports"),
            mock.patch(
                "verification_scheduler.subprocess.run", side_effect=execute
            ) as child,
        ):
            run_plan(
                plan,
                self.SYNTHETIC_HOME / "worktree",
                budget=ResourceBudget(1, 1),
                scratch_root=configured_root,
            )

        child.assert_called_once()
        self.assertNotIn(tmpdir_107, present)

    def test_06_utf8_108_byte_candidate_rejects_before_child_and_cleans(self) -> None:
        configured_root, _tmpdir_107, tmpdir_108 = self.utf8_boundary_paths()
        candidate = self.socket_candidate(tmpdir_108)
        self.assertEqual(len(str(candidate)), 107)
        self.assertEqual(len(os.fsencode(candidate)), 108)
        present: set[Path] = set()
        allocations: list[Path] = []

        def allocate(*_args: object, **_kwargs: object) -> str:
            allocations.append(tmpdir_108)
            present.add(tmpdir_108)
            return str(tmpdir_108)

        def remove(path: str | Path, *_args: object, **_kwargs: object) -> None:
            present.discard(Path(path))

        plan = self.plan(
            PlannedCommand("108-byte falsifier", ("forbidden",), lane=self.lane("utf8"))
        )

        def open_log(
            *_args: object, **_kwargs: object
        ) -> contextlib.AbstractContextManager[io.BytesIO]:
            return contextlib.nullcontext(io.BytesIO())

        error: ValueError | None = None
        with (
            mock.patch(
                "verification_scheduler.Path.home", return_value=self.SYNTHETIC_HOME
            ),
            mock.patch("verification_scheduler.Path.mkdir"),
            mock.patch("verification_scheduler.Path.open", side_effect=open_log),
            mock.patch(
                "verification_scheduler.tempfile.TemporaryDirectory",
                return_value=contextlib.nullcontext(str(configured_root / "logs")),
            ),
            mock.patch("verification_scheduler.tempfile.mkdtemp", side_effect=allocate),
            mock.patch("verification_scheduler.shutil.rmtree", side_effect=remove),
            mock.patch("verification_scheduler._emit_reports"),
            mock.patch("verification_scheduler.subprocess.run") as child,
        ):
            try:
                run_plan(
                    plan,
                    self.SYNTHETIC_HOME / "worktree",
                    budget=ResourceBudget(1, 1),
                    scratch_root=configured_root,
                )
            except ValueError as caught:
                error = caught

        self.assertFalse(present, "an allocated rejected TMPDIR must be removed")
        self.assertEqual(
            child.call_count,
            0,
            "the 108-byte Unix-socket candidate started a child",
        )
        self.assertIsNotNone(error)
        assert error is not None
        self.assertRegex(str(error), r"108")
        self.assertRegex(str(error), r"107")
        if allocations:
            self.assertEqual(allocations, [tmpdir_108])

    def test_07_child_failure_cleans_tmp_but_preserves_cargo_root(self) -> None:
        observed: dict[str, Path] = {}

        def fail(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            environment = dict(kwargs["env"])
            observed.update(
                tmpdir=Path(environment["TMPDIR"]),
                lane_root=Path(environment["EQIORA_VERIFY_LANE_ROOT"]),
                cargo_root=Path(environment["CARGO_TARGET_DIR"]),
            )
            self.assertTrue(observed["tmpdir"].exists())
            raise subprocess.CalledProcessError(23, argv)

        plan = self.plan(
            PlannedCommand(
                "reported child failure", ("fail",), lane=self.lane("failure")
            )
        )
        with tempfile.TemporaryDirectory(prefix="e496-", dir=Path.home()) as directory:
            scratch_root = Path(directory)
            with (
                mock.patch("verification_scheduler.subprocess.run", side_effect=fail),
                self.assertRaises(VerificationFailure) as raised,
            ):
                run_plan(
                    plan,
                    scratch_root,
                    budget=ResourceBudget(1, 1),
                    scratch_root=scratch_root,
                )

            self.assertEqual(
                [failure.returncode for failure in raised.exception.failures], [23]
            )
            self.assertFalse(observed["tmpdir"].exists())
            self.assertTrue(observed["lane_root"].exists())
            self.assertTrue(observed["cargo_root"].exists())

    def test_08_outside_home_configured_root_rejects_before_child(self) -> None:
        plan = self.plan(
            PlannedCommand("must not start", ("forbidden",), lane=self.lane("outside"))
        )
        with (
            mock.patch("verification_scheduler.subprocess.run") as child,
            self.assertRaisesRegex(ValueError, "below the home directory"),
        ):
            run_plan(
                plan,
                Path.home(),
                budget=ResourceBudget(1, 1),
                scratch_root=Path("/var/lib/eqiora-issue496-scratch"),
            )
        child.assert_not_called()


class SchedulerTests(unittest.TestCase):
    @staticmethod
    def plan(*commands: PlannedCommand) -> VerificationPlan:
        return VerificationPlan("affected", (), (), (), commands, ())

    @staticmethod
    def lane(
        name: str,
        *,
        cpu_slots: int = 1,
        memory_mib: int = 1,
        gpu_slots: int = 0,
        locks: tuple[str, ...] = (),
    ) -> VerificationLane:
        return VerificationLane(
            name,
            ResourceRequest(cpu_slots, memory_mib, gpu_slots, locks),
        )

    def test_v3_delegates_admission_to_the_callable_scheduler_core(self) -> None:
        core = importlib.import_module("resource_scheduler")
        scheduler = importlib.import_module("verification_scheduler")

        self.assertIs(scheduler.ResourceBudget, core.ResourceBudget)
        self.assertIs(scheduler.ResourceRequest, core.ResourceRequest)
        self.assertNotIn("_fits", vars(scheduler))
        self.assertIn("run_tasks(", inspect.getsource(scheduler.run_plan))

    def test_disjoint_lanes_overlap_and_logs_remain_plan_ordered(self) -> None:
        rendezvous = threading.Barrier(2)
        completion_order: list[str] = []
        completion_lock = threading.Lock()

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            rendezvous.wait(timeout=1.0)
            if argv[0] == "slow":
                time.sleep(0.05)
            output = kwargs["stdout"]
            output.write(f"LOG-{argv[0]}\n".encode())
            output.flush()
            with completion_lock:
                completion_order.append(argv[0])
            return subprocess.CompletedProcess(argv, 0)

        plan = self.plan(
            PlannedCommand("slow witness", ("slow",), lane=self.lane("rust")),
            PlannedCommand("fast witness", ("fast",), lane=self.lane("python")),
        )
        stream = io.StringIO()
        with (
            mock.patch("local_verify.subprocess.run", side_effect=execute),
            contextlib.redirect_stdout(stream),
            tempfile.TemporaryDirectory(dir=Path.home()) as directory,
        ):
            run_plan(
                plan,
                Path(directory),
                budget=ResourceBudget(2, 2),
                scratch_root=Path(directory) / "scratch",
            )

        self.assertEqual(completion_order, ["fast", "slow"])
        self.assertEqual(
            [
                line
                for line in stream.getvalue().splitlines()
                if line.startswith("LOG-")
            ],
            ["LOG-slow", "LOG-fast"],
        )

    def test_cpu_memory_gpu_and_named_locks_each_prevent_overlap(self) -> None:
        scenarios = {
            "cpu": (
                self.lane("first", cpu_slots=1),
                self.lane("second", cpu_slots=1),
                ResourceBudget(1, 2, 0),
            ),
            "memory": (
                self.lane("first", memory_mib=2),
                self.lane("second", memory_mib=2),
                ResourceBudget(2, 2, 0),
            ),
            "gpu": (
                self.lane("first", gpu_slots=1),
                self.lane("second", gpu_slots=1),
                ResourceBudget(2, 2, 1),
            ),
            "named lock": (
                self.lane("first", locks=("shared-tool",)),
                self.lane("second", locks=("shared-tool",)),
                ResourceBudget(2, 2, 0),
            ),
        }
        for name, (first, second, budget) in scenarios.items():
            with self.subTest(resource=name):
                second_started = threading.Event()
                overlap_observed = threading.Event()

                def execute(
                    argv: tuple[str, ...], **kwargs: object
                ) -> subprocess.CompletedProcess[bytes]:
                    if argv[0] == "first":
                        if second_started.wait(timeout=0.05):
                            overlap_observed.set()
                    else:
                        second_started.set()
                    return subprocess.CompletedProcess(argv, 0)

                plan = self.plan(
                    PlannedCommand("first", ("first",), lane=first),
                    PlannedCommand("second", ("second",), lane=second),
                )
                with (
                    mock.patch("local_verify.subprocess.run", side_effect=execute),
                    tempfile.TemporaryDirectory(dir=Path.home()) as directory,
                ):
                    run_plan(
                        plan,
                        Path(directory),
                        budget=budget,
                        scratch_root=Path(directory) / "scratch",
                    )
                self.assertFalse(overlap_observed.is_set())

    def test_insufficient_budget_rejects_before_starting_a_child(self) -> None:
        scenarios = {
            "cpu": (ResourceRequest(2, 1), ResourceBudget(1, 1)),
            "memory": (ResourceRequest(1, 2), ResourceBudget(1, 1)),
            "gpu": (ResourceRequest(1, 1, 1), ResourceBudget(1, 1, 0)),
        }
        for name, (request, budget) in scenarios.items():
            with self.subTest(resource=name):
                plan = self.plan(
                    PlannedCommand(
                        "must not start",
                        ("forbidden",),
                        lane=VerificationLane("oversized", request),
                    )
                )
                with (
                    mock.patch("local_verify.subprocess.run") as execute,
                    tempfile.TemporaryDirectory(dir=Path.home()) as directory,
                    self.assertRaisesRegex(ValueError, name),
                ):
                    run_plan(
                        plan,
                        Path(directory),
                        budget=budget,
                        scratch_root=Path(directory) / "scratch",
                    )
                execute.assert_not_called()

    def test_commands_remain_ordered_inside_one_lane(self) -> None:
        observed: list[str] = []

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            observed.append(argv[0])
            return subprocess.CompletedProcess(argv, 0)

        lane = self.lane("ordered")
        plan = self.plan(
            PlannedCommand("first", ("first",), lane=lane),
            PlannedCommand("second", ("second",), lane=lane),
        )
        with (
            mock.patch("local_verify.subprocess.run", side_effect=execute),
            tempfile.TemporaryDirectory(dir=Path.home()) as directory,
        ):
            run_plan(
                plan,
                Path(directory),
                budget=ResourceBudget(2, 2),
                scratch_root=Path(directory) / "scratch",
            )
        self.assertEqual(observed, ["first", "second"])

    def test_failure_skips_successor_and_collects_independent_failure(self) -> None:
        rendezvous = threading.Barrier(2)
        observed: list[str] = []
        observed_lock = threading.Lock()

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            with observed_lock:
                observed.append(argv[0])
            if argv[0] != "forbidden":
                rendezvous.wait(timeout=1.0)
            returncode = {"first-failure": 7, "independent-failure": 9}.get(argv[0], 0)
            if returncode:
                raise subprocess.CalledProcessError(returncode, argv)
            return subprocess.CompletedProcess(argv, returncode)

        failed_lane = self.lane("failed")
        plan = self.plan(
            PlannedCommand("first failure", ("first-failure",), lane=failed_lane),
            PlannedCommand("must be skipped", ("forbidden",), lane=failed_lane),
            PlannedCommand(
                "independent failure",
                ("independent-failure",),
                lane=self.lane("independent"),
            ),
        )
        with (
            mock.patch("local_verify.subprocess.run", side_effect=execute),
            tempfile.TemporaryDirectory(dir=Path.home()) as directory,
            self.assertRaises(VerificationFailure) as raised,
        ):
            run_plan(
                plan,
                Path(directory),
                budget=ResourceBudget(2, 2),
                scratch_root=Path(directory) / "scratch",
            )

        self.assertCountEqual(observed, ["first-failure", "independent-failure"])
        self.assertEqual(
            [failure.returncode for failure in raised.exception.failures], [7, 9]
        )

    def test_execution_error_preserves_detail_and_skips_lane_successor(self) -> None:
        rendezvous = threading.Barrier(2)
        observed: list[str] = []
        observed_lock = threading.Lock()

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            with observed_lock:
                observed.append(argv[0])
            if argv[0] != "forbidden":
                rendezvous.wait(timeout=1.0)
            if argv[0] == "broken":
                raise OSError("missing executable: broken")
            return subprocess.CompletedProcess(argv, 0)

        failed_lane = self.lane("failed")
        plan = self.plan(
            PlannedCommand("broken command", ("broken",), lane=failed_lane),
            PlannedCommand("must be skipped", ("forbidden",), lane=failed_lane),
            PlannedCommand(
                "independent success",
                ("independent",),
                lane=self.lane("independent"),
            ),
        )
        with (
            mock.patch("local_verify.subprocess.run", side_effect=execute),
            tempfile.TemporaryDirectory(dir=Path.home()) as directory,
            self.assertRaises(VerificationFailure) as raised,
        ):
            run_plan(
                plan,
                Path(directory),
                budget=ResourceBudget(2, 2),
                scratch_root=Path(directory) / "scratch",
            )

        self.assertCountEqual(observed, ["broken", "independent"])
        failures = raised.exception.failures
        self.assertEqual(len(failures), 1)
        self.assertIsNone(failures[0].returncode)
        self.assertEqual(failures[0].detail, "missing executable: broken")
        self.assertEqual(failures[0].command.label, "broken command")
        self.assertIn("missing executable: broken", str(raised.exception))

    def test_lane_environment_is_home_backed_contained_and_disjoint(self) -> None:
        environments: dict[str, dict[str, str]] = {}
        rendezvous = threading.Barrier(2)

        def execute(
            argv: tuple[str, ...], **kwargs: object
        ) -> subprocess.CompletedProcess[bytes]:
            environments[argv[0]] = dict(kwargs["env"])
            rendezvous.wait(timeout=1.0)
            return subprocess.CompletedProcess(argv, 0)

        plan = self.plan(
            PlannedCommand("first", ("first",), lane=self.lane("first")),
            PlannedCommand("second", ("second",), lane=self.lane("second")),
        )
        with (
            mock.patch("local_verify.subprocess.run", side_effect=execute),
            tempfile.TemporaryDirectory(dir=Path.home()) as directory,
        ):
            scratch_root = Path(directory) / "scratch"
            run_plan(
                plan,
                Path(directory),
                budget=ResourceBudget(2, 2),
                scratch_root=scratch_root,
            )
            roots = {
                name: Path(environment["EQIORA_VERIFY_LANE_ROOT"])
                for name, environment in environments.items()
            }
            self.assertEqual(len(set(roots.values())), 2)
            for name, root in roots.items():
                self.assertTrue(root.is_relative_to(scratch_root))
                self.assertTrue(Path(environments[name]["TMPDIR"]).is_relative_to(root))
                self.assertTrue(
                    Path(environments[name]["CARGO_TARGET_DIR"]).is_relative_to(root)
                )
                self.assertEqual(environments[name]["CARGO_BUILD_JOBS"], "1")


class ResourceDetectionTests(unittest.TestCase):
    SYSCONF_VALUES = {"SC_AVPHYS_PAGES": 131072, "SC_PAGE_SIZE": 4096}

    def test_memavailable_is_preferred_over_sysconf_fallback(self) -> None:
        meminfo = (
            "MemTotal:       32000000 kB\n"
            "MemFree:          512000 kB\n"
            "MemAvailable:    2048000 kB\n"
        )
        sysconf = mock.MagicMock(side_effect=self.SYSCONF_VALUES.__getitem__)
        with (
            mock.patch("verification_scheduler.Path.read_text", return_value=meminfo),
            mock.patch("verification_scheduler.os.sysconf", sysconf),
        ):
            self.assertEqual(_available_memory_mib(), 2000)
        sysconf.assert_not_called()

    def test_unreadable_meminfo_falls_back_to_sysconf(self) -> None:
        sysconf = mock.MagicMock(side_effect=self.SYSCONF_VALUES.__getitem__)
        with (
            mock.patch(
                "verification_scheduler.Path.read_text",
                side_effect=OSError("denied"),
            ),
            mock.patch("verification_scheduler.os.sysconf", sysconf),
        ):
            self.assertEqual(_available_memory_mib(), 512)


class CpuAllocationTests(unittest.TestCase):
    # The module lane constants clamp cpu requests to the importing host's
    # cpu_count, so the fixture pins the uncapped plan requests instead.
    PLAN_LANE_REQUESTS = (
        ("repository", 1),
        ("root-cargo", 4),
        ("dependency-policy", 1),
        ("python-candidate", 2),
        ("studio", 2),
        ("cubecl", 2),
    )

    def test_sixty_four_slots_divide_deterministically_across_plan_lanes(self) -> None:
        plan = build_plan("periodic", [], [], workspace())
        self.assertEqual(
            [lane.name for lane in dict.fromkeys(item.lane for item in plan.commands)],
            [name for name, _cpu in self.PLAN_LANE_REQUESTS],
        )
        lanes = tuple(
            VerificationLane(name, ResourceRequest(cpu, 1))
            for name, cpu in self.PLAN_LANE_REQUESTS
        )
        self.assertEqual(
            cpu_allocations(lanes, ResourceBudget(64, 1)),
            {
                "repository": 5,
                "root-cargo": 21,
                "dependency-policy": 5,
                "python-candidate": 11,
                "studio": 11,
                "cubecl": 11,
            },
        )

    def test_exact_budget_keeps_one_job_per_lane(self) -> None:
        lanes = (
            VerificationLane("first", ResourceRequest(1, 1)),
            VerificationLane("second", ResourceRequest(1, 1)),
        )
        self.assertEqual(
            cpu_allocations(lanes, ResourceBudget(2, 1)),
            {"first": 1, "second": 1},
        )


class LocalDocumentationTests(unittest.TestCase):
    def test_dependency_trees_are_outside_the_documentation_contract(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "docs").mkdir()
            (root / "docs/capability-matrix.md").write_text(
                "matrix\n", encoding="utf-8"
            )
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
