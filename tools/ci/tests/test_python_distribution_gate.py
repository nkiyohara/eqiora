from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import tomllib
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/ci"))
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

import python_distribution_gate  # noqa: E402
from candidate_manifest import ManifestError, REQUIRED_PROFILES  # noqa: E402


class PythonDistributionGateTests(unittest.TestCase):
    def test_aggregate_builds_once_and_projects_every_required_profile(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "artifacts" / "eqiora-python-candidate.json"

            def build(output: Path, *, require_tag: bool, skip_extras: bool) -> Path:
                self.assertEqual(output, root / "artifacts")
                self.assertFalse(require_tag)
                self.assertFalse(skip_extras)
                output.mkdir()
                manifest.write_text("candidate", encoding="utf-8")
                return manifest

            candidate = mock.sentinel.candidate
            with (
                mock.patch.object(
                    python_distribution_gate,
                    "build_candidate",
                    side_effect=build,
                ) as build_candidate,
                mock.patch.object(
                    python_distribution_gate,
                    "load_candidate",
                    return_value=candidate,
                ) as load_candidate,
                mock.patch.object(
                    python_distribution_gate,
                    "verify_artifacts",
                ) as verify_artifacts,
                mock.patch.object(
                    python_distribution_gate,
                    "require_candidate_profile",
                ) as require_profile,
            ):
                observed = python_distribution_gate.build_and_verify_candidate(root)

            self.assertIs(observed, candidate)
            build_candidate.assert_called_once()
            load_candidate.assert_called_once_with(manifest)
            verify_artifacts.assert_called_once_with(candidate, root / "artifacts")
            self.assertEqual(
                require_profile.call_args_list,
                [mock.call(candidate, profile) for profile in REQUIRED_PROFILES],
            )
            self.assertFalse(manifest.exists())
            self.assertTrue((root / "manifest" / manifest.name).is_file())

    def test_profile_failure_is_a_fail_closed_gate_diagnostic(self) -> None:
        with (
            mock.patch.object(
                python_distribution_gate,
                "build_and_verify_candidate",
                side_effect=ManifestError(
                    "candidate jax profile omits required check 'cp313:jax'"
                ),
            ),
            redirect_stderr(io.StringIO()) as stderr,
        ):
            self.assertEqual(python_distribution_gate.main(), 2)

        self.assertIn("Python distribution gate failed", stderr.getvalue())
        self.assertIn("candidate jax profile", stderr.getvalue())

    def test_aggregate_scratch_is_resolved_below_home(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            scratch = Path(temporary) / "gate"
            scratch.mkdir()

            def temporary_directory(*args: object, **kwargs: object) -> object:
                parent = kwargs.get("dir")
                self.assertIsNotNone(parent)
                self.assertTrue(
                    Path(parent).resolve().is_relative_to(Path.home().resolve())
                )
                return contextlib.nullcontext(str(scratch))

            with (
                mock.patch.object(
                    python_distribution_gate,
                    "build_and_verify_candidate",
                ) as build,
                mock.patch.object(
                    python_distribution_gate.tempfile,
                    "TemporaryDirectory",
                    side_effect=temporary_directory,
                ) as temporary_factory,
            ):
                self.assertEqual(python_distribution_gate.main(), 0)

        temporary_factory.assert_called_once()
        build.assert_called_once_with(scratch)

    def test_registered_profiles_share_one_exact_target(self) -> None:
        manifests = (
            "verify/interfaces/python-distribution-candidate/case.toml",
            "verify/interfaces/python-jax-differentiation/case.toml",
            "verify/interfaces/python-pytorch-differentiation/case.toml",
        )
        targets = []
        for relative in manifests:
            document = tomllib.loads(
                (REPOSITORY_ROOT / relative).read_text(encoding="utf-8")
            )
            targets.append(document["evidence"])

        self.assertEqual(
            targets,
            [
                {
                    "runner": "python-installed-wheel",
                    "script": "tools/ci/python_distribution_gate.py",
                }
            ]
            * len(manifests),
        )
        for focused in ("tools/ci/python_jax_gate.py", "tools/ci/python_torch_gate.py"):
            self.assertTrue((REPOSITORY_ROOT / focused).is_file())


if __name__ == "__main__":
    unittest.main()
