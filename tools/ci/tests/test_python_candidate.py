from __future__ import annotations

import contextlib
import importlib
import io
import sys
import tarfile
import tempfile
import threading
import time
import tomllib
import types
import unittest
import zipfile
from collections import Counter
from collections.abc import Callable, Iterator
from dataclasses import FrozenInstanceError
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

import python_candidate as python_candidate_module  # noqa: E402

from python_candidate import (  # noqa: E402
    PYTHON_TEST_FIXTURES,
    CandidateError,
    DistributionConfig,
    SourceIdentity,
    ensure_exact_uv,
    exact_uv_version,
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
            uv="uv==0.12.1",
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

    def test_standard_release_tools_group_is_the_only_uv_version_source(self) -> None:
        document = tomllib.loads(
            (REPOSITORY_ROOT / "pyproject.toml").read_text(encoding="utf-8")
        )
        config = python_candidate_module.load_config()

        self.assertNotIn("uv", document["tool"]["eqiora-distribution"])
        self.assertEqual(
            document["dependency-groups"]["release-tools"],
            [config.twine, config.uv],
        )

    @mock.patch("python_candidate.tool_version", return_value="uv 0.12.1")
    def test_release_tool_requires_the_exact_reviewed_uv(
        self,
        version: mock.Mock,
    ) -> None:
        require_exact_uv("/usr/bin/uv", "uv==0.12.1")
        version.assert_called_once_with(["/usr/bin/uv", "--version"])

        version.return_value = "uv 0.12.0"
        with self.assertRaisesRegex(CandidateError, "requires uv 0.12.1"):
            require_exact_uv("/usr/bin/uv", "uv==0.12.1")

        for malformed in ("uv>=0.12.1", "uv==0.12", "uv==../0.12.1"):
            with self.assertRaisesRegex(CandidateError, "requirement is malformed"):
                exact_uv_version(malformed)

    def test_exact_uv_is_installed_once_in_a_versioned_cache(self) -> None:
        calls: list[list[str]] = []

        def checked(argv: list[str], **_kwargs: object) -> str:
            calls.append(argv)
            if argv[1:3] == ["-m", "venv"]:
                (_virtual_environment := Path(argv[-1]) / "bin").mkdir(parents=True)
                (_virtual_environment / "python").touch()
                return ""
            if argv[1:4] == ["-m", "pip", "install"]:
                Path(argv[0]).with_name("uv").touch()
                return ""
            if argv[-1:] == ["--version"]:
                return "uv 0.12.1"
            self.fail(f"unexpected command: {argv}")

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            cache = Path(temporary) / "tools"
            with mock.patch.object(
                python_candidate_module,
                "checked_run",
                side_effect=checked,
            ):
                first = ensure_exact_uv("uv==0.12.1", cache_root=cache)
                install_call_count = len(calls)
                second = ensure_exact_uv("uv==0.12.1", cache_root=cache)

        self.assertEqual(first, second)
        self.assertEqual(
            Path(first),
            cache.resolve() / "uv" / "0.12.1" / "bin" / "uv",
        )
        self.assertEqual(install_call_count, 4)
        self.assertEqual(len(calls), 5)
        self.assertEqual(
            calls[1][-3:],
            [
                "--disable-pip-version-check",
                "--only-binary=:all:",
                "uv==0.12.1",
            ],
        )

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


class CandidateProfileFanoutContractTests(unittest.TestCase):
    COMPLETE_NAMES = (
        "base-3.11",
        "base-3.12",
        "base-3.13",
        "base-3.14",
        "numpy-floor-3.12",
        "generated-public-api",
        "torch-3.13",
        "jax-3.13",
        "matplotlib-3.13",
        "typing-3.13",
    )

    @staticmethod
    def profiles_module() -> object:
        # Keep the rest of this file collectable before the new private module
        # exists; the focused tests still fail at their exact missing seam.
        return importlib.import_module("python_candidate_profiles")

    @staticmethod
    def can_overlap(left: object, right: object) -> bool:
        return (
            left.cpu_slots + right.cpu_slots <= 2
            and left.memory_mib + right.memory_mib <= 4096
            and left.gpu_slots + right.gpu_slots <= 0
            and set(left.locks).isdisjoint(right.locks)
        )

    def test_complete_and_development_plans_are_exact_and_resource_admitted(
        self,
    ) -> None:
        profiles = self.profiles_module()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            complete = profiles.build_profile_plan(
                Path(temporary), self.config(), skip_extras=False
            )
            development = profiles.build_profile_plan(
                Path(temporary) / "development",
                self.config(),
                skip_extras=True,
            )

        self.assertEqual(profiles.COMPLETE_PROFILE_NAMES, self.COMPLETE_NAMES)
        self.assertEqual(
            profiles.DEVELOPMENT_PROFILE_NAMES,
            self.COMPLETE_NAMES[:6],
        )
        self.assertEqual(tuple(item.name for item in complete), self.COMPLETE_NAMES)
        self.assertEqual(
            tuple(item.name for item in development), self.COMPLETE_NAMES[:6]
        )

        by_name = {item.name: item.resources for item in complete}
        for request in by_name.values():
            self.assertLessEqual(request.cpu_slots, 2)
            self.assertLessEqual(request.memory_mib, 4096)
            self.assertEqual(request.gpu_slots, 0)

        heavy = ("torch-3.13", "jax-3.13", "typing-3.13")
        for index, name in enumerate(heavy):
            for other in heavy[index + 1 :]:
                self.assertFalse(
                    self.can_overlap(by_name[name], by_name[other]),
                    f"heavy profiles {name} and {other} were jointly admitted",
                )
            self.assertTrue(
                self.can_overlap(by_name[name], by_name["matplotlib-3.13"]),
                f"heavy profile {name} cannot overlap a fitting light profile",
            )
        self.assertTrue(self.can_overlap(by_name["base-3.11"], by_name["base-3.12"]))

    def config(self) -> DistributionConfig:
        return PythonCandidateTests().config()

    @contextlib.contextmanager
    def mocked_candidate_build(
        self,
        root: Path,
        profile_callback: Callable[[str, types.SimpleNamespace], None],
    ) -> Iterator[types.SimpleNamespace]:
        config = self.config()
        output = root / "artifacts"
        scratch = root / "candidate-scratch"
        extracted = scratch / "source"
        extracted.mkdir(parents=True)
        (extracted / "LICENSE").write_text("license\n", encoding="utf-8")
        (extracted / "NOTICE").write_text("notice\n", encoding="utf-8")
        sdist = scratch / "eqiora-0.1.0a1.tar.gz"
        sdist.write_bytes(b"sdist")
        wheels = {
            version: scratch / f"eqiora-0.1.0a1-cp{version.replace('.', '')}.whl"
            for version in config.interpreters
        }
        for version, wheel in wheels.items():
            wheel.write_bytes(f"wheel-{version}".encode())

        observations = types.SimpleNamespace(
            output=output,
            scratch=scratch,
            extracted=extracted,
            sdist=sdist,
            wheels=wheels,
            inspections=0,
            interpreter_resolutions=0,
            active_interpreter_resolutions=0,
            maximum_interpreter_resolutions=0,
            temporary_calls=[],
            scratch_exited=False,
        )
        interpreter_lock = threading.Lock()

        def temporary_directory(*args: object, **kwargs: object) -> object:
            observations.temporary_calls.append((args, kwargs))
            parent = kwargs.get("dir")
            if parent is not None and not Path(parent).resolve().is_relative_to(
                Path.home().resolve()
            ):
                raise AssertionError("candidate scratch escaped home")

            @contextlib.contextmanager
            def owned_scratch() -> object:
                try:
                    yield str(scratch)
                finally:
                    observations.scratch_exited = True

            return owned_scratch()

        def inspect(*args: object, **kwargs: object) -> tuple[str, dict[str, object]]:
            observations.inspections += 1
            python_version = kwargs["python_version"]
            wheel = args[0] if args else kwargs["wheel"]
            return config.python_version, {
                "filename": wheel.name,
                "kind": "wheel",
                "python": python_version,
                "sha256": python_candidate_module.sha256(wheel),
            }

        def interpreter(_uv: str, version: str) -> str:
            with interpreter_lock:
                observations.active_interpreter_resolutions += 1
                observations.maximum_interpreter_resolutions = max(
                    observations.maximum_interpreter_resolutions,
                    observations.active_interpreter_resolutions,
                )
            time.sleep(0.005)
            with interpreter_lock:
                observations.active_interpreter_resolutions -= 1
                observations.interpreter_resolutions += 1
            return f"/managed/python-{version}"

        def base(**kwargs: object) -> list[str]:
            name = f"base-{kwargs['python_version']}"
            profile_callback(name, observations)
            return [f"check:{name}"]

        def numpy_floor(**kwargs: object) -> tuple[list[str], dict[str, str]]:
            name = "numpy-floor-3.12"
            profile_callback(name, observations)
            return [f"check:{name}"], {
                "python": "3.12",
                "observed": "2.1.0",
            }

        def optional(**kwargs: object) -> list[str]:
            name = f"{kwargs['name']}-3.13"
            profile_callback(name, observations)
            return [f"check:{name}"]

        def typing(**_kwargs: object) -> str:
            name = "typing-3.13"
            profile_callback(name, observations)
            return f"check:{name}"

        def checked(argv: list[str], **_kwargs: object) -> str:
            if any(part.endswith("generate_python_api.py") for part in argv):
                profile_callback("generated-public-api", observations)
            return ""

        build_artifacts = mock.Mock(return_value=(sdist, wheels, extracted))
        manifest_writer = mock.Mock(return_value=output / "candidate.json")
        patches = (
            mock.patch.object(
                python_candidate_module.platform, "system", return_value="Linux"
            ),
            mock.patch.object(
                python_candidate_module.platform, "machine", return_value="x86_64"
            ),
            mock.patch.object(
                python_candidate_module, "load_config", return_value=config
            ),
            mock.patch.object(
                python_candidate_module,
                "source_identity",
                return_value=SourceIdentity("0" * 40, ()),
            ),
            mock.patch.object(
                python_candidate_module,
                "require_executable",
                side_effect=lambda name: f"/tool/{name}",
            ),
            mock.patch.object(
                python_candidate_module,
                "ensure_exact_uv",
                return_value="/tool/uv",
            ),
            mock.patch.object(
                python_candidate_module, "checked_run", side_effect=checked
            ),
            mock.patch.object(
                python_candidate_module, "build_artifacts", build_artifacts
            ),
            mock.patch.object(
                python_candidate_module, "inspect_wheel", side_effect=inspect
            ),
            mock.patch.object(
                python_candidate_module, "uv_interpreter", side_effect=interpreter
            ),
            mock.patch.object(
                python_candidate_module, "run_base_profile", side_effect=base
            ),
            mock.patch.object(
                python_candidate_module,
                "run_numpy_floor_profile",
                side_effect=numpy_floor,
            ),
            mock.patch.object(
                python_candidate_module, "run_optional_profile", side_effect=optional
            ),
            mock.patch.object(
                python_candidate_module, "run_full_typing_profile", side_effect=typing
            ),
            mock.patch.object(
                python_candidate_module, "write_manifest", manifest_writer
            ),
            mock.patch.object(
                python_candidate_module.tempfile,
                "TemporaryDirectory",
                side_effect=temporary_directory,
            ),
        )
        with contextlib.ExitStack() as stack:
            for patch in patches:
                stack.enter_context(patch)
            observations.build_artifacts = build_artifacts
            observations.manifest_writer = manifest_writer
            yield observations

    def test_direct_candidate_scratch_is_resolved_below_home(self) -> None:
        calls: Counter[str] = Counter()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            with self.mocked_candidate_build(
                Path(temporary), lambda name, _observed: calls.update((name,))
            ) as observed:
                python_candidate_module.build_candidate(
                    observed.output,
                    require_tag=False,
                    skip_extras=True,
                )

        self.assertEqual(len(observed.temporary_calls), 1)
        self.assertEqual(calls, Counter(self.COMPLETE_NAMES[:6]))
        _args, keyword_arguments = observed.temporary_calls[0]
        parent = keyword_arguments.get("dir")
        self.assertIsNotNone(parent)
        self.assertTrue(Path(parent).resolve().is_relative_to(Path.home().resolve()))

    def test_build_has_one_barrier_fanout_and_frozen_manifest_merge(self) -> None:
        calls: Counter[str] = Counter()
        active: set[str] = set()
        overlap: set[frozenset[str]] = set()
        lock = threading.Lock()

        def profile(name: str, observations: types.SimpleNamespace) -> None:
            self.assertEqual(observations.inspections, 4)
            self.assertEqual(observations.interpreter_resolutions, 4)
            with lock:
                calls[name] += 1
                overlap.update(frozenset((name, other)) for other in active)
                active.add(name)
            time.sleep(0.04 if name == "base-3.11" else 0.02)
            with lock:
                active.remove(name)

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            with self.mocked_candidate_build(root, profile) as observed:
                manifest = python_candidate_module.build_candidate(
                    observed.output,
                    require_tag=False,
                    skip_extras=False,
                )

        self.assertEqual(manifest, observed.output / "candidate.json")
        self.assertEqual(calls, Counter(self.COMPLETE_NAMES))
        observed.build_artifacts.assert_called_once()
        observed.manifest_writer.assert_called_once()
        self.assertEqual(observed.maximum_interpreter_resolutions, 1)
        self.assertEqual(len(observed.temporary_calls), 1)

        heavy = {"torch-3.13", "jax-3.13", "typing-3.13"}
        self.assertFalse(any(pair <= heavy for pair in overlap))
        self.assertTrue(
            any(len(pair & heavy) == 1 and len(pair - heavy) == 1 for pair in overlap)
        )
        self.assertIn(frozenset(("base-3.11", "base-3.12")), overlap)

        manifest_arguments = observed.manifest_writer.call_args.kwargs
        self.assertEqual(
            manifest_arguments["checks"],
            [
                "twine-strict",
                "sdist-to-wheel-rebuild",
                *(f"check:{name}" for name in self.COMPLETE_NAMES),
            ],
        )
        self.assertEqual(
            manifest_arguments["dependency_profiles"],
            {"numpy_floor": {"python": "3.12", "observed": "2.1.0"}},
        )

    def test_final_identity_rejects_profile_mutation_before_manifest(self) -> None:
        mutations = {
            "sdist": lambda observed: observed.sdist.write_bytes(b"mutated sdist"),
            "wheel": lambda observed: observed.wheels["3.11"].write_bytes(
                b"mutated wheel"
            ),
            "extracted source": lambda observed: (
                observed.extracted / "unexpected"
            ).write_text("mutation\n", encoding="utf-8"),
        }
        for target, mutate in mutations.items():
            with self.subTest(target=target):
                mutated = False

                def profile(_name: str, observed: types.SimpleNamespace) -> None:
                    nonlocal mutated
                    if not mutated:
                        mutate(observed)
                        mutated = True

                with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                    with self.mocked_candidate_build(
                        Path(temporary), profile
                    ) as observed:
                        with self.assertRaises(CandidateError):
                            python_candidate_module.build_candidate(
                                observed.output,
                                require_tag=False,
                                skip_extras=True,
                            )
                        observed.manifest_writer.assert_not_called()

    def test_profile_failures_join_before_cleanup_and_block_manifest(self) -> None:
        rendezvous = threading.Barrier(2)
        started: list[str] = []
        lock = threading.Lock()

        def profile(name: str, observed: types.SimpleNamespace) -> None:
            if name not in {"base-3.11", "base-3.12"}:
                return
            with lock:
                started.append(name)
            rendezvous.wait(timeout=1.0)
            if name == "base-3.11":
                time.sleep(0.03)
            self.assertFalse(observed.scratch_exited)
            raise RuntimeError(f"diagnostic from {name}")

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            with self.mocked_candidate_build(Path(temporary), profile) as observed:
                with self.assertRaises(CandidateError) as raised:
                    python_candidate_module.build_candidate(
                        observed.output,
                        require_tag=False,
                        skip_extras=True,
                    )
                observed.manifest_writer.assert_not_called()
            self.assertTrue(observed.scratch_exited)

        self.assertCountEqual(started, ["base-3.11", "base-3.12"])
        diagnostic = str(raised.exception)
        first = diagnostic.index("base-3.11")
        second = diagnostic.index("base-3.12")
        self.assertLess(first, second)

    def test_profile_writable_roots_and_environment_are_disjoint(self) -> None:
        profiles = self.profiles_module()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            scratch = Path(temporary)
            plan = profiles.build_profile_plan(
                scratch, self.config(), skip_extras=False
            )

            writable: list[Path] = []
            for item in plan:
                self.assertTrue(item.root.is_relative_to(scratch))
                paths = [
                    item.environment,
                    item.consumer,
                    item.temporary,
                    item.log,
                ]
                if item.matplotlib_config is not None:
                    paths.append(item.matplotlib_config)
                for path in paths:
                    self.assertTrue(path.is_relative_to(item.root))
                writable.extend(paths)

                environment = dict(item.environment_variables)
                self.assertEqual(Path(environment["TMPDIR"]), item.temporary)
                if item.name == "matplotlib-3.13":
                    self.assertEqual(
                        Path(environment["MPLCONFIGDIR"]), item.matplotlib_config
                    )
                else:
                    self.assertIsNone(item.matplotlib_config)
                    self.assertNotIn("MPLCONFIGDIR", environment)

            self.assertEqual(len(set(writable)), len(writable))

            owners = {
                "EQIORA_TEST_TORCH_VERSION": "torch-3.13",
                "EQIORA_REQUIRE_JAX_ABI_PROBE": "jax-3.13",
                "EQIORA_TEST_JAX_VERSION": "jax-3.13",
                "EQIORA_TEST_PYTHON_VERSION": "jax-3.13",
                "JAX_ENABLE_X64": "jax-3.13",
                "XLA_FLAGS": "jax-3.13",
                "EQIORA_TEST_MATPLOTLIB_VERSION": "matplotlib-3.13",
                "MPLBACKEND": "matplotlib-3.13",
                "MPLCONFIGDIR": "matplotlib-3.13",
            }
            for variable, owner in owners.items():
                observed = [
                    item.name
                    for item in plan
                    if variable in dict(item.environment_variables)
                ]
                self.assertEqual(observed, [owner], variable)

    def test_reverse_completion_merges_immutable_receipts_in_frozen_order(
        self,
    ) -> None:
        profiles = self.profiles_module()
        first = profiles.ProfileReceipt(
            name="base-3.11",
            checks=("base-z", "base-a"),
            dependency_profiles=(),
            diagnostics=("base diagnostic",),
            log="BASE LOG\n",
        )
        second = profiles.ProfileReceipt(
            name="numpy-floor-3.12",
            checks=("numpy-floor",),
            dependency_profiles=(
                (
                    "numpy_floor",
                    (
                        ("observed", "2.1.0"),
                        ("python", "3.12"),
                    ),
                ),
            ),
            diagnostics=("numpy diagnostic",),
            log="NUMPY LOG\n",
        )
        with self.assertRaises(FrozenInstanceError):
            first.name = "mutated"

        forward = profiles.merge_profile_receipts(
            ("base-3.11", "numpy-floor-3.12"), (first, second)
        )
        reversed_completion = profiles.merge_profile_receipts(
            ("base-3.11", "numpy-floor-3.12"), (second, first)
        )

        self.assertEqual(forward, reversed_completion)
        self.assertEqual(forward.receipts, (first, second))
        self.assertEqual(forward.checks, ("base-z", "base-a", "numpy-floor"))
        self.assertEqual(
            forward.dependency_profiles,
            second.dependency_profiles,
        )
        self.assertEqual(
            forward.diagnostics,
            (
                ("base-3.11", "base diagnostic"),
                ("numpy-floor-3.12", "numpy diagnostic"),
            ),
        )
        self.assertEqual(
            forward.logs,
            (
                ("base-3.11", "BASE LOG\n"),
                ("numpy-floor-3.12", "NUMPY LOG\n"),
            ),
        )

        manifests: list[bytes] = []
        with (
            tempfile.TemporaryDirectory(dir=Path.home()) as temporary,
            mock.patch.object(
                python_candidate_module,
                "tool_version",
                return_value="reviewed tool",
            ),
        ):
            root = Path(temporary)
            for index, report in enumerate((forward, reversed_completion)):
                output = root / str(index)
                output.mkdir()
                sdist = output / "eqiora-0.1.0a1.tar.gz"
                sdist.write_bytes(b"one immutable source distribution")
                manifest = python_candidate_module.write_manifest(
                    output=output,
                    source=SourceIdentity("0" * 40, ()),
                    sdist=sdist,
                    version="0.1.0a1",
                    wheel_records=[],
                    checks=list(report.checks),
                    config=self.config(),
                    uv="/reviewed/uv",
                    complete_profiles=True,
                    dependency_profiles={
                        name: dict(values)
                        for name, values in report.dependency_profiles
                    },
                )
                manifests.append(manifest.read_bytes())
        self.assertEqual(manifests[0], manifests[1])

        for invalid in ((first,), (first, first), (first, second, second)):
            with self.assertRaisesRegex(ValueError, "receipt"):
                profiles.merge_profile_receipts(
                    ("base-3.11", "numpy-floor-3.12"), invalid
                )


if __name__ == "__main__":
    unittest.main()
