#!/usr/bin/env python3
"""Precommitted evidence for the exact-cylinder steady-Stokes Marimo app."""

from __future__ import annotations

import ast
import hashlib
import importlib
import re
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
RELEASE = ROOT / "tools/release"
if str(RELEASE) not in sys.path:
    sys.path.insert(0, str(RELEASE))

APP_PATH = Path("examples/python/exact_cylinder_stokes_marimo.py")
APP = ROOT / APP_PATH
MUTANT_PATH = Path(
    "verify/interfaces/python-exact-cylinder-stokes-marimo/references/"
    "exact_cylinder_stokes_marimo_repository_helper_mutant.py"
)
MUTANT = ROOT / MUTANT_PATH
CHECK = "cp313:marimo-0.23.16-exact-cylinder-stokes"
ORACLE_FLAG = "EQIORA_EXACT_CYLINDER_STOKES_MARIMO_ORACLE"
EXPECTED_MUTANT_FAILURE = "ModuleNotFoundError: No module named 'examples'"
UNEXPECTED_HELPER_MARKER = "EQIORA_REPOSITORY_HELPER_UNEXPECTEDLY_RESOLVED_BEFORE_RUN"
HEX_SHA256 = re.compile(r"(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])")


def _recursive_regular_inventory(root: Path) -> tuple[str, ...]:
    members: list[str] = []
    for path in sorted(
        root.rglob("*"), key=lambda item: item.relative_to(root).as_posix()
    ):
        if path.is_symlink() or not path.is_file():
            raise AssertionError(f"consumer contains a non-regular member: {path}")
        members.append(path.relative_to(root).as_posix())
    return tuple(members)


def _require_one_direct_run(source: str) -> None:
    """Check only direct calls in the one canonical app source."""

    tree = ast.parse(source, filename=APP_PATH.as_posix())
    submit = 0
    result = 0
    run = 0
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Attribute):
            continue
        if isinstance(node.func.value, ast.Name) and node.func.value.id == "eqiora":
            if node.func.attr == "submit":
                submit += 1
            elif node.func.attr == "run":
                run += 1
        if node.func.attr == "result":
            result += 1
    observed = (submit, result, run)
    if observed != (1, 1, 0):
        raise AssertionError(
            "canonical app direct calls must be "
            f"eqiora.submit=1, .result=1, eqiora.run=0; observed {observed}"
        )


class ExactCylinderStokesMarimoEvidence(unittest.TestCase):
    def test_canonical_app_is_one_run_without_the_exact_repository_helper(self) -> None:
        self.assertTrue(APP.is_file(), f"missing canonical app: {APP_PATH}")
        source = APP.read_text(encoding="utf-8")

        # This is an exact-source check for the one canonical app, not a generic
        # Python import, file-access, or isolation policy.
        self.assertEqual(source.count("steady_stokes_evidence("), 1)
        self.assertEqual(source.count("plot_scalar_field("), 1)
        self.assertEqual(source.count("files(eqiora)"), 1)
        self.assertNotIn(
            "from examples.python.exact_cylinder_stokes import solve",
            source,
        )
        self.assertIsNone(HEX_SHA256.search(source))
        self.assertIn('__generated_with__ = "0.23.16"', source)
        self.assertIn("steady-flow-past-cylinder.model.json", source)
        for marker in (
            "eqiora-stokes-geometry",
            "eqiora-stokes-mesh-plan",
            "eqiora-stokes-mesh",
            "eqiora-stokes-model",
            "eqiora-stokes-plan",
            "eqiora-stokes-run",
            "eqiora-stokes-result",
            "eqiora-stokes-evidence",
            "EQIORA_EXACT_CYLINDER_STOKES_READY",
        ):
            self.assertIn(marker, source)

        _require_one_direct_run(source)
        second_run_mutant = source + "\neqiora.run(model, plan=plan)\n"
        with self.assertRaisesRegex(
            AssertionError,
            r"eqiora\.submit=1, \.result=1, eqiora\.run=0; observed \(1, 1, 1\)",
        ):
            _require_one_direct_run(second_run_mutant)

        self.assertTrue(MUTANT.is_file(), f"missing exact mutant: {MUTANT_PATH}")
        mutant = MUTANT.read_text(encoding="utf-8")
        self.assertEqual(
            mutant.count(
                "from examples.python.exact_cylinder_stokes import solve as repository_solve"
            ),
            1,
        )
        self.assertIn(UNEXPECTED_HELPER_MARKER, mutant)
        self.assertNotIn("eqiora.submit", mutant)
        self.assertNotIn("eqiora.run", mutant)
        self.assertEqual(mutant.count("repository_solve()"), 1)
        self.assertNotIn("app.run()", mutant)

    def test_candidate_inventory_and_runner_freeze_the_exact_route(self) -> None:
        manifest = importlib.import_module("candidate_manifest")
        profiles = importlib.import_module("python_candidate_profiles")
        candidate = importlib.import_module("python_candidate")

        self.assertIn(CHECK, manifest.NOTEBOOK_CHECKS)
        self.assertIn(CHECK, profiles.NOTEBOOK_CHECK_NAMES)

        runner_source = Path(candidate.__file__).read_text(encoding="utf-8")
        self.assertIn(APP_PATH.as_posix(), runner_source)
        self.assertIn(MUTANT_PATH.as_posix(), runner_source)
        self.assertIn(CHECK, runner_source)
        self.assertIn(ORACLE_FLAG, runner_source)
        self.assertIn(EXPECTED_MUTANT_FAILURE, runner_source)

    def test_positive_then_exact_missing_helper_mutant_use_closed_consumers(
        self,
    ) -> None:
        candidate = importlib.import_module("python_candidate")
        profiles = importlib.import_module("python_candidate_profiles")
        executor = importlib.import_module("python_candidate_h2")

        receipt: dict[str, object] = {
            "browser": {
                "downloaded_archive_sha256": "a" * 64,
                "executable_sha256": "b" * 64,
                "platform": "linux-x64",
            },
            "python_host": {"resolved_environment_sha256": "c" * 64},
        }
        frontend = {
            "h2_receipt_sha256": hashlib.sha256(
                executor.canonical_json_bytes(receipt)
            ).hexdigest()
        }

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            extracted = root / "extracted"
            rich_test = extracted / "bindings/python/tests/test_rich_mesh_display.py"
            rich_test.parent.mkdir(parents=True)
            rich_test.write_text("def test_positive():\n    pass\n", encoding="utf-8")
            fake_app = extracted / APP_PATH
            fake_app.parent.mkdir(parents=True)
            fake_app.write_text("import marimo\n", encoding="utf-8")
            fake_mutant = extracted / MUTANT_PATH
            fake_mutant.parent.mkdir(parents=True)
            fake_mutant.write_bytes(MUTANT.read_bytes())

            python = root / "environment/bin/python"
            acquired = types.SimpleNamespace(
                npm=root / "node/bin/npm",
                node=root / "node/bin/node",
                browser_executable=root / "browser/chrome",
                browser_archive_sha256="a" * 64,
                browser_executable_sha256="b" * 64,
                browser_platform="linux-x64",
                python_wheels=(dict(name="host"),),
            )
            acquired.browser_executable.parent.mkdir(parents=True)
            acquired.browser_executable.write_bytes(b"reviewed browser")
            receipt["python_host"] = {
                "resolved_environment_sha256": executor.structured_sha256(
                    acquired.python_wheels
                )
            }
            frontend["h2_receipt_sha256"] = hashlib.sha256(
                executor.canonical_json_bytes(receipt)
            ).hexdigest()

            original_observer = profiles.run_notebook_profile

            def execute_profile(
                workspace_name: str,
                mutant_output: str | None,
            ) -> tuple[
                list[str],
                list[tuple[tuple[str, ...], Path]],
                list[tuple[tuple[str, ...], Path]],
                list[tuple[str, ...]],
            ]:
                workspace_root = root / workspace_name
                workspace = types.SimpleNamespace(
                    root=workspace_root,
                    environment=workspace_root / "environment",
                    consumer=workspace_root / "consumer",
                )
                checked_commands: list[tuple[tuple[str, ...], Path]] = []
                launches: list[tuple[tuple[str, ...], Path]] = []
                installs: list[tuple[str, ...]] = []
                emitted: list[str] = []
                causal_events: list[str] = []
                launch_inventories: dict[str, tuple[str, ...]] = {}

                def install_environment(**kwargs: object) -> Path:
                    installs.append(
                        tuple(str(value) for value in kwargs["requirements"])
                    )
                    return python

                def checked_run(argv: list[str], **kwargs: object) -> str:
                    command = tuple(str(value) for value in argv)
                    cwd = Path(kwargs.get("cwd", ROOT))
                    checked_commands.append((command, cwd))
                    if command[:5] == (
                        "npm",
                        "run",
                        "test:hosts",
                        "--",
                        "--project=marimo-0.23.16",
                    ) and "exact-cylinder-stokes-marimo.spec.ts" in "\n".join(command):
                        causal_events.append("positive-browser")
                    if any(Path(value).name == MUTANT_PATH.name for value in command):
                        causal_events.append("negative-missing-helper")
                        launch_inventories["negative"] = _recursive_regular_inventory(
                            cwd
                        )
                        if mutant_output is None:
                            return UNEXPECTED_HELPER_MARKER
                        raise subprocess.CalledProcessError(
                            1,
                            command,
                            output=mutant_output,
                        )
                    return ""

                def observe_checks(
                    observations: tuple[tuple[str, object], ...], *, emit: object
                ) -> tuple[str, ...]:
                    def record(name: str) -> None:
                        emitted.append(name)
                        emit(name)  # type: ignore[operator]

                    return original_observer(  # type: ignore[arg-type]
                        observations,
                        emit=record,
                    )

                process = mock.Mock()
                process.poll.return_value = None
                process.wait.return_value = 0

                def popen(argv: list[str], **kwargs: object) -> mock.Mock:
                    command = tuple(str(value) for value in argv)
                    cwd = Path(kwargs["cwd"])
                    launches.append(
                        (
                            command,
                            cwd,
                        )
                    )
                    if any(Path(value).name == APP_PATH.name for value in command):
                        launch_inventories["positive"] = _recursive_regular_inventory(
                            cwd
                        )
                    return process

                def stage_frontend(_source: Path, build: object) -> None:
                    Path(build.frontend).mkdir(parents=True, exist_ok=True)

                with (
                    mock.patch.object(
                        profiles,
                        "run_notebook_profile",
                        side_effect=observe_checks,
                    ),
                    mock.patch.object(
                        profiles,
                        "install_environment",
                        side_effect=install_environment,
                    ),
                    mock.patch.object(
                        candidate, "checked_run", side_effect=checked_run
                    ),
                    mock.patch.object(candidate.subprocess, "Popen", side_effect=popen),
                    mock.patch.object(
                        candidate.socket,
                        "create_connection",
                        return_value=mock.MagicMock(),
                    ),
                    mock.patch.object(
                        executor,
                        "stage_frontend",
                        side_effect=stage_frontend,
                    ),
                    mock.patch.object(
                        executor, "acquire_inputs", return_value=acquired
                    ),
                ):
                    observed = candidate.run_notebook_profile(
                        uv="/reviewed/uv",
                        interpreter="/reviewed/python3.13",
                        wheel=root / "candidate.whl",
                        extracted=extracted,
                        workspace=workspace,
                        config=candidate.load_config(),
                        receipt=receipt,
                        frontend=frontend,
                    )
                self.assertEqual(observed, emitted)
                self.assertEqual(
                    causal_events,
                    ["positive-browser", "negative-missing-helper"],
                )
                self.assertEqual(launch_inventories["positive"], (APP_PATH.name,))
                self.assertEqual(
                    launch_inventories["negative"],
                    (MUTANT_PATH.name,),
                )
                return observed, launches, checked_commands, installs

            observed, launches, checked_commands, installs = execute_profile(
                "accepted-missing-helper",
                EXPECTED_MUTANT_FAILURE,
            )
            self.assertIn(CHECK, observed)

            candidate_requirements = tuple(
                requirement
                for requirement in installs[0]
                if str(root / "candidate.whl") in requirement
            )
            self.assertTrue(candidate_requirements)
            self.assertIn("matplotlib", "\n".join(candidate_requirements))
            self.assertIn("notebook", "\n".join(candidate_requirements))

            positive_launches = [
                (argv, cwd)
                for argv, cwd in launches
                if any(Path(value).name == APP_PATH.name for value in argv)
            ]
            self.assertEqual(len(positive_launches), 1)
            positive_argv, positive_cwd = positive_launches[0]
            self.assertEqual(
                positive_argv[:5],
                (str(python), "-I", "-m", "marimo", "run"),
            )
            self.assertGreaterEqual(len(positive_argv), 6)
            self.assertEqual(
                Path(positive_argv[5]).resolve(strict=True),
                (positive_cwd / APP_PATH.name).resolve(strict=True),
            )
            self.assertEqual(
                _recursive_regular_inventory(positive_cwd),
                (APP_PATH.name,),
            )
            self.assertEqual(
                (positive_cwd / APP_PATH.name).read_bytes(),
                fake_app.read_bytes(),
            )

            negative_commands = [
                (argv, cwd)
                for argv, cwd in checked_commands
                if any(Path(value).name == MUTANT_PATH.name for value in argv)
            ]
            self.assertEqual(len(negative_commands), 1)
            negative_argv, negative_cwd = negative_commands[0]
            self.assertEqual(
                negative_argv,
                (str(python), "-I", str(negative_cwd / MUTANT_PATH.name)),
            )
            self.assertEqual(
                Path(negative_argv[2]).resolve(strict=True),
                (negative_cwd / MUTANT_PATH.name).resolve(strict=True),
            )
            self.assertNotEqual(positive_cwd, negative_cwd)
            self.assertEqual(
                _recursive_regular_inventory(negative_cwd),
                (MUTANT_PATH.name,),
            )
            self.assertEqual(
                (negative_cwd / MUTANT_PATH.name).read_bytes(),
                fake_mutant.read_bytes(),
            )
            self.assertTrue(
                any(
                    command[:5]
                    == (
                        "npm",
                        "run",
                        "test:hosts",
                        "--",
                        "--project=marimo-0.23.16",
                    )
                    and "exact-cylinder-stokes-marimo.spec.ts" in "\n".join(command)
                    for command, _cwd in checked_commands
                )
            )

            with self.assertRaises(
                (candidate.CandidateError, subprocess.CalledProcessError)
            ):
                execute_profile(
                    "unrelated-negative-failure",
                    "RuntimeError: unrelated denial",
                )
            with self.assertRaises(
                (candidate.CandidateError, subprocess.CalledProcessError)
            ):
                execute_profile(
                    "unexpectedly-resolved-helper",
                    None,
                )


if __name__ == "__main__":
    unittest.main()
