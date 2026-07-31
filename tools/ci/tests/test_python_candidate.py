from __future__ import annotations

import io
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

from python_candidate import (  # noqa: E402
    PYTHON_TEST_FIXTURES,
    CandidateError,
    DistributionConfig,
    SourceIdentity,
    inspect_wheel,
    prepare_base_consumer_tree,
    python_distribution_version,
    require_exact_uv,
    require_expected_tag,
    run_public_smoke,
    safe_extract_sdist,
)


class PythonCandidateTests(unittest.TestCase):
    def config(self) -> DistributionConfig:
        return DistributionConfig(
            cargo_version="0.1.0-alpha.1",
            interpreters=("3.11", "3.12", "3.13", "3.14"),
            wheel_platform="manylinux_2_17_x86_64",
            extras_interpreter="3.13",
            numpy_floor_interpreter="3.12",
            numpy_floor="numpy==2.1.0",
            uv="uv==0.11.31",
            maturin="maturin==1.14.1",
            pytest="pytest==9.1.1",
            mypy="mypy==2.3.0",
            twine="twine==6.2.0",
            torch="torch==2.13.0",
            jax=("jax==0.11.0", "jaxlib==0.11.0"),
            matplotlib="matplotlib==3.11.1",
            rust="1.89",
        )

    def test_release_identity_has_one_python_version_and_exact_tag(self) -> None:
        self.assertEqual(
            python_distribution_version("0.1.0-alpha.1"),
            "0.1.0a1",
        )
        self.assertEqual(self.config().expected_tag, "v0.1.0a1")
        require_expected_tag(
            SourceIdentity(commit="0" * 40, tags=("v0.1.0a1",)),
            self.config().expected_tag,
        )
        with self.assertRaisesRegex(
            CandidateError,
            "requires exact tag v0.1.0a1",
        ):
            require_expected_tag(
                SourceIdentity(commit="0" * 40, tags=("v0.1.0",)),
                self.config().expected_tag,
            )
        for rejected in (
            "0.1.0-dev.1",
            "0.1.0-alpha",
            "0.1.0-alpha.01",
            "0.1.0-alpha.1.extra",
            "0.1.0+local",
        ):
            with self.assertRaises(CandidateError, msg=rejected):
                python_distribution_version(rejected)

    @mock.patch("python_candidate.tool_version", return_value="uv 0.11.31")
    def test_release_tool_requires_the_exact_reviewed_uv(
        self,
        version: mock.Mock,
    ) -> None:
        require_exact_uv("/usr/bin/uv", "uv==0.11.31")
        version.assert_called_once_with(["/usr/bin/uv", "--version"])

        version.return_value = "uv 0.11.32"
        with self.assertRaisesRegex(CandidateError, "requires uv 0.11.31"):
            require_exact_uv("/usr/bin/uv", "uv==0.11.31")

        with self.assertRaisesRegex(CandidateError, "requirement is malformed"):
            require_exact_uv("/usr/bin/uv", "uv>=0.11.31")

    @mock.patch("python_candidate.checked_run")
    def test_public_smoke_uses_the_installed_interpreter_in_isolated_mode(
        self,
        checked: mock.Mock,
    ) -> None:
        run_public_smoke(
            python=Path("/candidate/bin/python"),
            extracted=Path("/sdist"),
            run_root=Path("/consumer"),
            expected_version="0.1.0a1",
            profile="base",
        )

        checked.assert_called_once_with(
            [
                "/candidate/bin/python",
                "-I",
                "/sdist/tools/release/python_public_smoke.py",
                "--expected-version",
                "0.1.0a1",
                "--profile",
                "base",
            ],
            cwd=Path("/consumer"),
        )

    def test_wheel_contract_accepts_complete_typed_optional_metadata(self) -> None:
        license_bytes = b"license\n"
        notice_bytes = b"notice\n"
        metadata = b"""\
Metadata-Version: 2.4
Name: eqiora
Version: 0.1.0a1
Requires-Python: <3.15,>=3.11
License-Expression: Apache-2.0
License-File: LICENSE
License-File: NOTICE
Provides-Extra: jax
Provides-Extra: matplotlib
Provides-Extra: torch
Requires-Dist: numpy<3,>=2.1
Requires-Dist: torch>=2.13,<2.14; extra == "torch"
Requires-Dist: jax==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: jaxlib==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: matplotlib==3.11.1; extra == "matplotlib"

typed candidate
"""
        with tempfile.TemporaryDirectory() as temporary:
            wheel = (
                Path(temporary) / "eqiora-0.1.0a1-cp313-cp313-manylinux_2_17_x86_64.whl"
            )
            dist_info = "eqiora-0.1.0a1.dist-info/"
            with zipfile.ZipFile(wheel, mode="w") as archive:
                for name in (
                    "eqiora/__init__.py",
                    "eqiora/__init__.pyi",
                    "eqiora/diff.pyi",
                    "eqiora/fsi.pyi",
                    "eqiora/jax.pyi",
                    "eqiora/matplotlib.pyi",
                    "eqiora/solid.pyi",
                    "eqiora/torch.pyi",
                    "eqiora/py.typed",
                    "eqiora/examples/steady-flow-past-cylinder.model.json",
                    "eqiora/examples/mixed-boundary-elasticity.eqi",
                    "eqiora/examples/fixed-reference-fsi.eqi",
                    "eqiora/_eqiora.cpython-313-x86_64-linux-gnu.so",
                    f"{dist_info}sboms/eqiora-python.cyclonedx.json",
                ):
                    archive.writestr(name, b"")
                archive.writestr(f"{dist_info}METADATA", metadata)
                archive.writestr(f"{dist_info}licenses/LICENSE", license_bytes)
                archive.writestr(f"{dist_info}licenses/NOTICE", notice_bytes)

            version, record = inspect_wheel(
                wheel,
                python_version="3.13",
                config=self.config(),
                license_bytes=license_bytes,
                notice_bytes=notice_bytes,
            )

        self.assertEqual(version, "0.1.0a1")
        self.assertEqual(record["python"], "3.13")
        self.assertEqual(record["platform"], "manylinux_2_17_x86_64")
        self.assertRegex(record["sha256"], r"^[0-9a-f]{64}$")

    def test_wheel_contract_rejects_framework_as_a_base_dependency(self) -> None:
        license_bytes = b"license\n"
        notice_bytes = b"notice\n"
        metadata = b"""\
Metadata-Version: 2.4
Name: eqiora
Version: 0.1.0a1
Requires-Python: <3.15,>=3.11
License-Expression: Apache-2.0
License-File: LICENSE
License-File: NOTICE
Provides-Extra: jax
Provides-Extra: matplotlib
Provides-Extra: torch
Requires-Dist: numpy<3,>=2.1
Requires-Dist: torch>=2.13,<2.14
Requires-Dist: jax==0.11.0; extra == "jax"
Requires-Dist: jaxlib==0.11.0; extra == "jax"
Requires-Dist: matplotlib==3.11.1; extra == "matplotlib"

invalid candidate
"""
        with tempfile.TemporaryDirectory() as temporary:
            wheel = (
                Path(temporary) / "eqiora-0.1.0a1-cp313-cp313-manylinux_2_17_x86_64.whl"
            )
            dist_info = "eqiora-0.1.0a1.dist-info/"
            with zipfile.ZipFile(wheel, mode="w") as archive:
                for name in (
                    "eqiora/__init__.py",
                    "eqiora/__init__.pyi",
                    "eqiora/diff.pyi",
                    "eqiora/fsi.pyi",
                    "eqiora/jax.pyi",
                    "eqiora/matplotlib.pyi",
                    "eqiora/solid.pyi",
                    "eqiora/torch.pyi",
                    "eqiora/py.typed",
                    "eqiora/examples/steady-flow-past-cylinder.model.json",
                    "eqiora/examples/mixed-boundary-elasticity.eqi",
                    "eqiora/examples/fixed-reference-fsi.eqi",
                    "eqiora/_eqiora.cpython-313-x86_64-linux-gnu.so",
                    f"{dist_info}sboms/eqiora-python.cyclonedx.json",
                ):
                    archive.writestr(name, b"")
                archive.writestr(f"{dist_info}METADATA", metadata)
                archive.writestr(f"{dist_info}licenses/LICENSE", license_bytes)
                archive.writestr(f"{dist_info}licenses/NOTICE", notice_bytes)

            with self.assertRaisesRegex(
                CandidateError,
                "torch must remain an optional-extra dependency",
            ):
                inspect_wheel(
                    wheel,
                    python_version="3.13",
                    config=self.config(),
                    license_bytes=license_bytes,
                    notice_bytes=notice_bytes,
                )

    def test_sdist_extraction_rejects_parent_traversal(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / "invalid.tar.gz"
            with tarfile.open(archive, mode="w:gz") as destination:
                member = tarfile.TarInfo("../escape")
                payload = b"not allowed"
                member.size = len(payload)
                destination.addfile(member, io.BytesIO(payload))

            with self.assertRaisesRegex(CandidateError, "escapes its root"):
                safe_extract_sdist(archive, root / "extract")

    def test_consumer_tree_preserves_repository_relative_fixture_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            extracted = root / "source"
            run_root = root / "consumer"
            run_root.mkdir()
            # Derived from the constant rather than restated: a copy of the
            # fixture list here would silently stop covering a fixture added
            # to `PYTHON_TEST_FIXTURES`, which is the drift this test exists
            # to catch.
            files = (
                "bindings/python/tests/test_vertical_slice.py",
                "bindings/python/typecheck/base.py",
                *(str(fixture / "payload.json") for fixture in PYTHON_TEST_FIXTURES),
            )
            self.assertGreaterEqual(len(files), 4)
            for relative in files:
                path = extracted / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(relative, encoding="utf-8")

            tests, typecheck = prepare_base_consumer_tree(extracted, run_root)

            self.assertEqual(tests, run_root / "bindings/python/tests")
            self.assertEqual(typecheck, run_root / "bindings/python/typecheck")
            test_path = tests / "test_vertical_slice.py"
            self.assertEqual(test_path.parents[3], run_root)
            for relative in files:
                self.assertTrue((run_root / relative).is_file())


if __name__ == "__main__":
    unittest.main()
