from __future__ import annotations

import contextlib
import io
import sys
import tempfile
import tomllib
import types
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
    REVISION = "1" * 40

    @staticmethod
    def option(argv: list[str], name: str) -> Path:
        return Path(argv[argv.index(name) + 1])

    @staticmethod
    def write_family(directory: Path) -> None:
        directory.mkdir()
        (directory / "eqiora-0.1.0a1.tar.gz").write_bytes(b"sdist")
        for python in ("311", "312", "313", "314"):
            (directory / f"eqiora-0.1.0a1-cp{python}-cp{python}-linux.whl").write_bytes(
                f"wheel-{python}".encode()
            )

    def test_required_profiles_include_one_exact_notebook_projection(self) -> None:
        self.assertEqual(
            REQUIRED_PROFILES,
            ("base", "jax", "matplotlib", "notebook", "torch", "typing"),
        )

    def test_aggregate_runs_exact_prepare_h2_finalize_argv_then_profiles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            calls: list[list[str]] = []

            def run(argv: list[str], **_kwargs: object) -> str:
                calls.append(argv)
                if argv[2] == "prepare":
                    self.write_family(self.option(argv, "--out"))
                elif argv[1].endswith("python_candidate_h2.py"):
                    output = self.option(argv, "--out")
                    output.mkdir()
                    (output / "eqiora-0.1.0a1-python-candidate-h2.json").write_bytes(
                        b"opaque schema-test receipt"
                    )
                elif argv[2] == "finalize":
                    output = self.option(argv, "--manifest-out")
                    output.mkdir()
                    (output / "eqiora-0.1.0a1-python-candidate.json").write_bytes(
                        b"opaque schema-test manifest"
                    )
                    receipt = self.option(argv, "--h2-receipt")
                    (output / receipt.name).write_bytes(receipt.read_bytes())
                else:  # pragma: no cover - assertion explains unexpected argv
                    self.fail(f"unexpected command: {argv}")
                return ""

            candidate = mock.sentinel.candidate
            with (
                mock.patch.object(
                    python_distribution_gate,
                    "checked_run",
                    side_effect=run,
                    create=True,
                ),
                mock.patch.object(
                    python_distribution_gate,
                    "source_identity",
                    return_value=types.SimpleNamespace(commit=self.REVISION),
                    create=True,
                ),
                mock.patch.object(
                    python_distribution_gate,
                    "load_candidate_family",
                    return_value=candidate,
                    create=True,
                ) as load_candidate_family,
                mock.patch.object(
                    python_distribution_gate,
                    "verify_artifacts",
                ) as verify_artifacts,
                mock.patch.object(
                    python_distribution_gate,
                    "require_candidate_profile",
                ) as require_profile,
                mock.patch.object(
                    python_distribution_gate,
                    "build_candidate",
                    side_effect=AssertionError(
                        "monolithic candidate path is forbidden"
                    ),
                    create=True,
                ) as build_candidate,
            ):
                observed = python_distribution_gate.build_and_verify_candidate(root)

            self.assertIs(observed, candidate)
            build_candidate.assert_not_called()
            self.assertEqual(len(calls), 3)
            family = self.option(calls[0], "--out")
            h2_output = self.option(calls[1], "--out")
            metadata = self.option(calls[2], "--manifest-out")
            self.assertEqual(len({family, h2_output, metadata}), 3)
            for left in (family, h2_output, metadata):
                self.assertTrue(left.is_relative_to(root))
                for right in (family, h2_output, metadata):
                    if left != right:
                        self.assertFalse(left.is_relative_to(right))

            receipt = h2_output / "eqiora-0.1.0a1-python-candidate-h2.json"
            manifest = metadata / "eqiora-0.1.0a1-python-candidate.json"
            retained_receipt = metadata / receipt.name
            self.assertEqual(
                calls,
                [
                    [
                        "python3",
                        "tools/release/python_candidate.py",
                        "prepare",
                        "--expected-commit",
                        self.REVISION,
                        "--out",
                        str(family),
                    ],
                    [
                        "python3",
                        "tools/release/python_candidate_h2.py",
                        "--expected-commit",
                        self.REVISION,
                        "--artifacts",
                        str(family),
                        "--out",
                        str(h2_output),
                    ],
                    [
                        "python3",
                        "tools/release/python_candidate.py",
                        "finalize",
                        "--expected-commit",
                        self.REVISION,
                        "--artifacts",
                        str(family),
                        "--h2-receipt",
                        str(receipt),
                        "--manifest-out",
                        str(metadata),
                    ],
                ],
            )
            load_candidate_family.assert_called_once_with(
                manifest,
                family,
                requested_profiles=REQUIRED_PROFILES,
                h2_receipt=retained_receipt,
            )
            verify_artifacts.assert_called_once_with(candidate, family)
            self.assertEqual(
                require_profile.call_args_list,
                [mock.call(candidate, profile) for profile in REQUIRED_PROFILES],
            )
            self.assertEqual(
                {path.name for path in metadata.iterdir()},
                {manifest.name, retained_receipt.name},
            )
            self.assertEqual(
                {path.suffix for path in family.iterdir()},
                {".gz", ".whl"},
            )

    def test_aggregate_rejects_partial_metadata_and_post_h2_family_mutation(
        self,
    ) -> None:
        for mutation, diagnostic in (
            ("missing-retained-receipt", "retained H2 receipt"),
            ("family-byte", "family inventory changed"),
        ):
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                receipt_bytes = b"opaque schema-test receipt"
                retained_receipt: Path | None = None

                def run(argv: list[str], **_kwargs: object) -> str:
                    nonlocal retained_receipt
                    if argv[2] == "prepare":
                        self.write_family(self.option(argv, "--out"))
                    elif argv[1].endswith("python_candidate_h2.py"):
                        output = self.option(argv, "--out")
                        output.mkdir()
                        (
                            output / "eqiora-0.1.0a1-python-candidate-h2.json"
                        ).write_bytes(receipt_bytes)
                    else:
                        family = self.option(argv, "--artifacts")
                        output = self.option(argv, "--manifest-out")
                        output.mkdir()
                        (output / "eqiora-0.1.0a1-python-candidate.json").write_bytes(
                            b"opaque schema-test manifest"
                        )
                        if mutation == "family-byte":
                            receipt = self.option(argv, "--h2-receipt")
                            retained_receipt = output / receipt.name
                            retained_receipt.write_bytes(receipt.read_bytes())
                            next(family.glob("*.whl")).write_bytes(b"mutated")
                        else:
                            self.assertEqual(mutation, "missing-retained-receipt")
                    return ""

                with (
                    mock.patch.object(
                        python_distribution_gate,
                        "checked_run",
                        side_effect=run,
                        create=True,
                    ),
                    mock.patch.object(
                        python_distribution_gate,
                        "source_identity",
                        return_value=types.SimpleNamespace(commit=self.REVISION),
                        create=True,
                    ),
                    mock.patch.object(
                        python_distribution_gate,
                        "load_candidate_family",
                        create=True,
                    ) as load_candidate_family,
                    mock.patch.object(
                        python_distribution_gate,
                        "verify_artifacts",
                    ) as verify_artifacts,
                    mock.patch.object(
                        python_distribution_gate,
                        "build_candidate",
                        side_effect=AssertionError(
                            "monolithic candidate path is forbidden"
                        ),
                        create=True,
                    ),
                ):
                    with self.assertRaisesRegex(RuntimeError, diagnostic):
                        python_distribution_gate.build_and_verify_candidate(root)

                load_candidate_family.assert_not_called()
                verify_artifacts.assert_not_called()
                if mutation == "family-byte":
                    assert retained_receipt is not None
                    self.assertEqual(retained_receipt.read_bytes(), receipt_bytes)

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
