#!/usr/bin/env python3
"""Precommitted evidence for the exact-cylinder steady-Stokes Marimo app."""

from __future__ import annotations

import ast
import hashlib
import importlib
import re
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
CHECK = "cp313:marimo-0.23.16-exact-cylinder-stokes"
ORACLE_FLAG = "EQIORA_EXACT_CYLINDER_STOKES_MARIMO_ORACLE"
HEX_SHA256 = re.compile(r"[0-9a-f]{64}")


def _dotted_name(node: ast.expr) -> str | None:
    if isinstance(node, ast.Name):
        return node.id
    if isinstance(node, ast.Attribute):
        owner = _dotted_name(node.value)
        return None if owner is None else f"{owner}.{node.attr}"
    return None


def _admit_app_source(source: str) -> ast.Module:
    """Admit only the frozen installed-product composition source shape."""

    tree = ast.parse(source, filename=APP_PATH.as_posix())
    imports: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            imports.update(alias.name for alias in node.names)
        elif isinstance(node, ast.ImportFrom):
            if node.module is None or node.level != 0:
                raise AssertionError("the app must use only absolute imports")
            imports.add(node.module)

    allowed = {"eqiora", "eqiora.matplotlib", "importlib.resources", "marimo"}
    if imports != allowed:
        raise AssertionError(
            f"app imports differ from the frozen set: {sorted(imports)}"
        )

    call_counts: dict[str, int] = {}
    for node in ast.walk(tree):
        if isinstance(node, ast.Call):
            name = _dotted_name(node.func)
            if name is not None:
                call_counts[name] = call_counts.get(name, 0) + 1

    required = {
        "eqiora.submit": 1,
        "eqiora.fluid.steady_stokes_evidence": 1,
        "eqplot.plot_scalar_field": 1,
        "files": 1,
    }
    for name, count in required.items():
        if call_counts.get(name, 0) != count:
            raise AssertionError(f"app must call {name} exactly {count} time(s)")
    if call_counts.get("eqiora.run", 0) != 0:
        raise AssertionError(
            "app must retain the one inspectable Run returned by submit"
        )

    result_calls = sum(
        count for name, count in call_counts.items() if name.endswith(".result")
    )
    if result_calls != 1:
        raise AssertionError("app must obtain its Result from the one Run exactly once")

    constants = tuple(
        node.value
        for node in ast.walk(tree)
        if isinstance(node, ast.Constant) and isinstance(node.value, str)
    )
    if any(HEX_SHA256.fullmatch(value) for value in constants):
        raise AssertionError("the app must not freeze an identity digest")
    joined = "\n".join(constants)
    for forbidden in (
        "Cargo.toml",
        "PYTHONPATH",
        "bindings/python",
        "tools/ci",
        "verify/",
        "repository-only",
    ):
        if forbidden in joined:
            raise AssertionError(
                f"repository-only source marker is forbidden: {forbidden}"
            )
    return tree


class ExactCylinderStokesMarimoEvidence(unittest.TestCase):
    def test_app_is_one_installed_product_run_before_repository_falsifiers(
        self,
    ) -> None:
        self.assertTrue(APP.is_file(), f"missing canonical app: {APP_PATH}")
        source = APP.read_text(encoding="utf-8")

        # Ordinary positive path first. These predicates freeze composition,
        # not any scientific value already owned by the five predecessor cases.
        _admit_app_source(source)
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

        # These mutants are checked only after the ordinary source is admitted.
        # They fail before a Marimo process or Eqiora Run can exist.
        repository_mutant = (
            "import pathlib\n"
            + source
            + ("\npathlib.Path('repository-only.sentinel').read_text()\n")
        )
        with self.assertRaisesRegex(AssertionError, "imports differ|repository-only"):
            _admit_app_source(repository_mutant)

        editable_mutant = (
            "import sys\n"
            + source
            + ("\nsys.path.insert(0, 'bindings/python/python')\n")
        )
        with self.assertRaisesRegex(AssertionError, "imports differ|repository-only"):
            _admit_app_source(editable_mutant)

        second_run_mutant = source + (
            "\ndef _forbidden_second_run(model, plan):\n"
            "    return eqiora.submit(model, plan=plan)\n"
        )
        with self.assertRaisesRegex(AssertionError, "eqiora.submit exactly 1"):
            _admit_app_source(second_run_mutant)

    def test_candidate_inventory_and_runner_freeze_the_exact_route(self) -> None:
        manifest = importlib.import_module("candidate_manifest")
        profiles = importlib.import_module("python_candidate_profiles")
        candidate = importlib.import_module("python_candidate")

        self.assertIn(CHECK, manifest.NOTEBOOK_CHECKS)
        self.assertIn(CHECK, profiles.NOTEBOOK_CHECK_NAMES)

        runner_source = Path(candidate.__file__).read_text(encoding="utf-8")
        self.assertIn(APP_PATH.as_posix(), runner_source)
        self.assertIn(CHECK, runner_source)
        self.assertIn(ORACLE_FLAG, runner_source)

    def test_candidate_stages_only_the_app_for_the_exact_isolated_host(self) -> None:
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
            (extracted / "repository-only.sentinel").write_text(
                "must not be staged", encoding="utf-8"
            )

            workspace_root = root / "notebook-profile"
            workspace = types.SimpleNamespace(
                root=workspace_root,
                environment=workspace_root / "environment",
                consumer=workspace_root / "consumer",
            )
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

            installs: list[tuple[str, ...]] = []
            commands: list[tuple[str, ...]] = []
            launches: list[tuple[tuple[str, ...], Path]] = []
            emitted: list[str] = []
            original_observer = profiles.run_notebook_profile

            def install_environment(**kwargs: object) -> Path:
                installs.append(tuple(str(value) for value in kwargs["requirements"]))
                return python

            def checked_run(argv: list[str], **_kwargs: object) -> str:
                commands.append(tuple(str(value) for value in argv))
                return ""

            def observe_checks(
                observations: tuple[tuple[str, object], ...], *, emit: object
            ) -> tuple[str, ...]:
                def record(name: str) -> None:
                    emitted.append(name)
                    emit(name)  # type: ignore[operator]

                return original_observer(observations, emit=record)  # type: ignore[arg-type]

            process = mock.Mock()
            process.poll.return_value = None
            process.wait.return_value = 0

            def popen(argv: list[str], **kwargs: object) -> mock.Mock:
                launches.append(
                    (
                        tuple(str(value) for value in argv),
                        Path(kwargs["cwd"]),
                    )
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
                mock.patch.object(candidate, "checked_run", side_effect=checked_run),
                mock.patch.object(candidate.subprocess, "Popen", side_effect=popen),
                mock.patch.object(
                    candidate.socket,
                    "create_connection",
                    return_value=mock.MagicMock(),
                ),
                mock.patch.object(
                    executor, "stage_frontend", side_effect=stage_frontend
                ),
                mock.patch.object(executor, "acquire_inputs", return_value=acquired),
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

            self.assertIn(CHECK, observed)
            self.assertIn(CHECK, emitted)
            candidate_requirements = tuple(
                requirement
                for requirement in installs[0]
                if str(root / "candidate.whl") in requirement
            )
            self.assertTrue(candidate_requirements)
            self.assertIn("matplotlib", "\n".join(candidate_requirements))
            self.assertIn("notebook", "\n".join(candidate_requirements))
            exact_launches = [
                (argv, cwd)
                for argv, cwd in launches
                if APP_PATH.name in "\n".join(argv)
            ]
            self.assertEqual(len(exact_launches), 1)
            argv, cwd = exact_launches[0]
            self.assertEqual(argv[:4], (str(python), "-I", "-m", "marimo"))
            self.assertEqual(cwd, workspace.consumer)
            launched_app = next(
                Path(value) for value in argv if value.endswith(APP_PATH.name)
            )
            self.assertEqual(launched_app.parent, workspace.consumer)
            self.assertEqual(launched_app.read_bytes(), fake_app.read_bytes())
            self.assertFalse((workspace.consumer / "repository-only.sentinel").exists())
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
                    for command in commands
                )
            )


if __name__ == "__main__":
    unittest.main()
