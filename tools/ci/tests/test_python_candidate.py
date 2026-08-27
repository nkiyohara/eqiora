from __future__ import annotations

import ast
import base64
import builtins
import contextlib
import hashlib
import importlib
import inspect
import io
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
import tomllib
import types
import unittest
import urllib.request
import warnings
import zipfile
from collections import Counter
from collections.abc import Callable, Iterator
from dataclasses import FrozenInstanceError, replace
from pathlib import Path
from unittest import mock

from packaging.utils import parse_wheel_filename


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

import candidate_manifest as candidate_manifest_module  # noqa: E402
import python_candidate as python_candidate_module  # noqa: E402

from python_candidate import (  # noqa: E402
    PYTHON_TEST_FIXTURES,
    PYTHON_TEST_RESOURCES,
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


EXACT_WHEEL_INTERPRETERS = ("311", "312", "313", "314")
EXACT_PHYSICAL_PLATFORM = "manylinux_2_17_x86_64.manylinux2014_x86_64"
EXACT_WHEEL_PAYLOAD_SHA256 = {
    "311": "ed43ac65d3c530f6bcbeaefeecb1ffb2c71ea095526acac884cae5aa95ede8b0",
    "312": "c5e3da21766d0e72af45ff0d32b3b67a77ca97b6891d3f9fe3274b05a38b67c1",
    "313": "ec165fa06dd8f506937c232afdfb13d3be2bdaab9cee2698ac4526fefb922094",
    "314": "1b85bc509658486a6c068cc33ad9306c802d469a25096a557faeb0a55595fc6b",
}
EXACT_WHEEL_MEMBER = "eqiora-0.1.0a1.dist-info/WHEEL"
EXACT_RECORD_MEMBER = "eqiora-0.1.0a1.dist-info/RECORD"
PLAYWRIGHT_CORE_LOCK_SHA256 = (
    "d739363f768ff874f025ae0e4e2e90f327454981bc8870c34739dea5178ef35e"
)
PLAYWRIGHT_CORE_PACKAGE_SHA256 = (
    "07c47543631fef9508760365dee9fbe958c562093ec8d122543949ed231f233f"
)
PLAYWRIGHT_CORE_BROWSERS_SHA256 = (
    "f306eed529599b1eaf2f8a85db9de2b23e1a3fe36c2b66434b7c9434fb627a99"
)
PLAYWRIGHT_CORE_BUNDLE_SHA256 = (
    "9393fa79e1c67c74edc26b610d65a4f7ed73d345a762465cc88340a33a2454ac"
)
PLAYWRIGHT_CORE_INTEGRITY = (
    "sha512-wPYSwEBJY9GHraISXqyqtx0na0LpO3XEX7jNDhntbex7tzUS7kLnZsOlFruFJB4Hi/"
    "rhDMjXGqHewDZ68nYZVw=="
)
PLAYWRIGHT_CORE_URL = (
    "https://registry.npmjs.org/playwright-core/-/playwright-core-1.62.1.tgz"
)
INSTALL_SCRIPT_INVENTORY_SHA256 = (
    "c706e144c3250d27383c3e6799cdcc8ac0220c7dd1c7cc4a89e14953a0204503"
)
LIFECYCLE_SCRIPT_SOURCE_UNION = (
    (
        "node_modules/fsevents",
        "fsevents",
        "2.3.2",
        "install",
        "node-gyp rebuild",
        ("lockfile", "packument"),
    ),
    (
        "node_modules/lightningcss",
        "lightningcss",
        "1.33.0",
        "prepare",
        "patch-package",
        ("packument", "tarball"),
    ),
    (
        "node_modules/tinyexec",
        "tinyexec",
        "1.3.0",
        "prepare",
        "npm run build",
        ("packument", "tarball"),
    ),
    (
        "node_modules/vite/node_modules/fsevents",
        "fsevents",
        "2.3.3",
        "install",
        "node-gyp rebuild",
        ("lockfile", "packument"),
    ),
)
PACKUMENT_INSTALL_SCRIPT_ADMISSIONS = (
    (
        "node_modules/fsevents",
        "fsevents",
        "2.3.2",
        "install",
        "node-gyp rebuild",
        "packument",
    ),
    (
        "node_modules/vite/node_modules/fsevents",
        "fsevents",
        "2.3.3",
        "install",
        "node-gyp rebuild",
        "packument",
    ),
)
PLAYWRIGHT_BROWSER_URL = (
    "https://cdn.playwright.dev/builds/cft/151.0.7922.34/linux64/"
    "chrome-headless-shell-linux64.zip"
)
PLAYWRIGHT_BROWSER_MEMBER = "chrome-headless-shell-linux64/chrome-headless-shell"
CONTENT_BOUND_BROWSER_PROFILE = {
    "platform": "linux-x86_64",
    "browser": "Chromium Headless Shell 151.0.7922.34",
    "playwright_revision": "1234",
    "url": PLAYWRIGHT_BROWSER_URL,
    "raw_archive_bytes": 120_231_126,
    "raw_archive_sha256": (
        "3cfc2bd00d1bafcf8a68dc74c9c92bb7150ddc8d26ade948a776316e1cec4f14"
    ),
    "zip_member_count": 287,
    "total_expanded_bytes": 273_378_828,
    "largest_expanded_member_bytes": 196_975_952,
    "largest_member": PLAYWRIGHT_BROWSER_MEMBER,
    "executable_sha256": (
        "e11fc9ce65c96313476f7ee9844b6fb6a9220fb048693cfe9eee00acf4170a9f"
    ),
    "closed_member_inventory_sha256": (
        "960a12d7e14cd59583eb0dd74065ed111d48167f7867ace8ff4ce578f2b64f3a"
    ),
}
CONTENT_BOUND_RESOURCE_LIMITS = {
    "family_member_count": 5,
    "family_member_bytes": 16_777_216,
    "family_total_bytes": 67_108_864,
    "source_member_count": 50_000,
    "source_member_bytes": 67_108_864,
    "source_total_bytes": 536_870_912,
    "locked_package_count": 2_048,
    "locked_package_bytes": 1_073_741_824,
    "resolved_python_wheel_count": 256,
    "resolved_python_wheel_bytes": 1_073_741_824,
    "build_output_count": 3,
    "build_output_bytes": 16_777_216,
    "host_scenarios": 2,
    "member_steps": 104_652,
    "byte_steps": 4_789_240_546,
}
NOTEBOOK_PROFILE_CHECKS = (
    "frontend:lock-integrity",
    "frontend:license-inventory",
    "frontend:bundle-byte-rebuild",
    "wheel-family:notebook-metadata",
    "cp313:notebook-anywidget-0.11.0",
    "cp313:marimo-0.23.16-exact-cylinder-stokes",
    "cp313:notebook-managed-chromium-r1234",
    "cp313:notebook-no-external-network",
    "cp313:notebook-cleanup-and-mutation",
)


def exact_wheel_name(compact_python: str, *, version: str = "0.1.0a1") -> str:
    return (
        f"eqiora-{version}-cp{compact_python}-cp{compact_python}-"
        f"{EXACT_PHYSICAL_PLATFORM}.whl"
    )


def exact_wheel_tags(compact_python: str) -> tuple[str, str]:
    prefix = f"cp{compact_python}-cp{compact_python}-"
    return (
        f"{prefix}manylinux_2_17_x86_64",
        f"{prefix}manylinux2014_x86_64",
    )


def maturin_wheel_payload(
    compact_python: str,
    *,
    tags: tuple[str, ...] | None = None,
) -> bytes:
    observed_tags = exact_wheel_tags(compact_python) if tags is None else tags
    tag_lines = "".join(f"Tag: {tag}\n" for tag in observed_tags)
    return (
        "Wheel-Version: 1.0\n"
        "Generator: maturin (1.14.1)\n"
        "Root-Is-Purelib: false\n"
        f"{tag_lines}"
    ).encode("utf-8")


def record_payload_for_wheel(
    wheel_member: str,
    wheel: bytes,
    record_member: str,
) -> bytes:
    digest = base64.urlsafe_b64encode(hashlib.sha256(wheel).digest()).rstrip(b"=")
    return (
        wheel_member.encode("utf-8")
        + b",sha256="
        + digest
        + f",{len(wheel)}\n".encode("ascii")
        + record_member.encode("utf-8")
        + b",,\n"
    )


def maturin_record_payload(compact_python: str) -> bytes:
    return record_payload_for_wheel(
        EXACT_WHEEL_MEMBER,
        maturin_wheel_payload(compact_python),
        EXACT_RECORD_MEMBER,
    )


def write_maturin_wheel(
    path: Path,
    compact_python: str,
    *,
    tags: tuple[str, ...] | None = None,
    members: tuple[tuple[str, bytes], ...] | None = None,
) -> None:
    match = re.fullmatch(r"eqiora-(.+)-cp[0-9]+-cp[0-9]+-.+\.whl", path.name)
    if match is None:
        raise AssertionError(f"synthetic wheel has an invalid filename: {path.name}")
    version = match.group(1)
    entries = members or (
        (
            f"eqiora-{version}.dist-info/WHEEL",
            maturin_wheel_payload(compact_python, tags=tags),
        ),
    )
    if not any(name.endswith(".dist-info/METADATA") for name, _payload in entries):
        entries = (
            *entries,
            (
                f"eqiora-{version}.dist-info/METADATA",
                (
                    "Metadata-Version: 2.4\n"
                    "Name: eqiora\n"
                    f"Version: {version}\n\n"
                ).encode("utf-8"),
            ),
        )
    if not any(name.endswith(".dist-info/RECORD") for name, _payload in entries):
        wheels = tuple(
            (name, payload)
            for name, payload in entries
            if name.endswith(".dist-info/WHEEL")
        )
        if wheels:
            wheel_member, wheel_payload = wheels[0]
            record_member = f"{wheel_member.removesuffix('/WHEEL')}/RECORD"
            record_payload = record_payload_for_wheel(
                wheel_member,
                wheel_payload,
                record_member,
            )
        else:
            record_member = EXACT_RECORD_MEMBER
            record_payload = maturin_record_payload(compact_python)
        entries = (*entries, (record_member, record_payload))
    with warnings.catch_warnings():
        warnings.filterwarnings(
            "ignore", message="Duplicate name:", category=UserWarning
        )
        with zipfile.ZipFile(path, mode="w") as archive:
            for name, payload in entries:
                member = zipfile.ZipInfo(name)
                member.create_system = 3
                member.external_attr = 0o100644 << 16
                member.compress_type = zipfile.ZIP_STORED
                archive.writestr(member, payload)


def wheel_byte_identity(
    path: Path,
    *,
    version: str = "0.1.0a1",
) -> tuple[bytes, bytes, bytes]:
    archive_bytes = path.read_bytes()
    with zipfile.ZipFile(path, mode="r") as archive:
        wheel_bytes = archive.read(f"eqiora-{version}.dist-info/WHEEL")
        record_bytes = archive.read(f"eqiora-{version}.dist-info/RECORD")
    return archive_bytes, wheel_bytes, record_bytes


@contextlib.contextmanager
def reject_post_producer_wheel_writes(
    sealed_paths: set[Path],
) -> Iterator[None]:
    real_builtin_open = builtins.open
    real_io_open = io.open
    real_os_open = os.open
    real_os_rename = os.rename
    real_os_replace = os.replace
    real_path_write_bytes = Path.write_bytes
    real_zipfile = zipfile.ZipFile

    def key(value: object) -> Path | None:
        candidate = value
        if not isinstance(candidate, (str, bytes, os.PathLike)):
            candidate = getattr(candidate, "name", None)
        if not isinstance(candidate, (str, bytes, os.PathLike)):
            return None
        try:
            return Path(candidate).resolve()
        except (OSError, TypeError, ValueError):
            return None

    def reject(value: object, operation: str) -> None:
        observed = key(value)
        if observed is not None and observed in sealed_paths:
            raise AssertionError(
                f"post-producer wheel rewrite is forbidden: {operation}: {observed}"
            )

    def opens_for_write(mode: object) -> bool:
        return any(token in str(mode) for token in ("w", "a", "x", "+"))

    def guarded_builtin_open(
        file: object, mode: str = "r", *args: object, **kwargs: object
    ) -> object:
        if opens_for_write(mode):
            reject(file, f"builtins.open({mode})")
        return real_builtin_open(file, mode, *args, **kwargs)

    def guarded_io_open(
        file: object, mode: str = "r", *args: object, **kwargs: object
    ) -> object:
        if opens_for_write(mode):
            reject(file, f"io.open({mode})")
        return real_io_open(file, mode, *args, **kwargs)

    def guarded_os_open(
        file: object, flags: int, *args: object, **kwargs: object
    ) -> int:
        write_flags = os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND
        if flags & write_flags:
            reject(file, f"os.open({flags})")
        return real_os_open(file, flags, *args, **kwargs)

    def guarded_write_bytes(path: Path, data: bytes) -> int:
        reject(path, "Path.write_bytes")
        return real_path_write_bytes(path, data)

    def guarded_rename(
        source: object,
        destination: object,
        *args: object,
        **kwargs: object,
    ) -> None:
        reject(source, "os.rename(source)")
        reject(destination, "os.rename(destination)")
        real_os_rename(source, destination, *args, **kwargs)

    def guarded_replace(
        source: object,
        destination: object,
        *args: object,
        **kwargs: object,
    ) -> None:
        reject(source, "os.replace(source)")
        reject(destination, "os.replace(destination)")
        real_os_replace(source, destination, *args, **kwargs)

    def guarded_zipfile(
        file: object,
        mode: str = "r",
        *args: object,
        **kwargs: object,
    ) -> zipfile.ZipFile:
        if opens_for_write(mode):
            reject(file, f"zipfile.ZipFile({mode})")
        return real_zipfile(file, mode, *args, **kwargs)

    with (
        mock.patch.object(builtins, "open", side_effect=guarded_builtin_open),
        mock.patch.object(io, "open", side_effect=guarded_io_open),
        mock.patch.object(os, "open", side_effect=guarded_os_open),
        mock.patch.object(os, "rename", side_effect=guarded_rename),
        mock.patch.object(os, "replace", side_effect=guarded_replace),
        mock.patch.object(Path, "write_bytes", new=guarded_write_bytes),
        mock.patch.object(zipfile, "ZipFile", side_effect=guarded_zipfile),
    ):
        yield


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

    def test_role_d_producer_removes_the_operative_alpha1_singleton(self) -> None:
        self.assertEqual(
            python_distribution_version("0.1.0-alpha.2"),
            "0.1.0a2",
        )
        source = (
            REPOSITORY_ROOT / "tools/release/python_candidate.py"
        ).read_text(encoding="utf-8")
        if 'config.python_version != "0.1.0a1"' in source:
            self.fail("Role D producer still pins the operative alpha.1 singleton")
        if '"0.1.0a2"' in source or '"v0.1.0a2"' in source:
            self.fail("Role D producer added a handwritten alpha.2 singleton")

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

    def test_notebook_is_one_exact_optional_dependency_and_never_mandatory(
        self,
    ) -> None:
        document = tomllib.loads(
            (REPOSITORY_ROOT / "pyproject.toml").read_text(encoding="utf-8")
        )
        project = document["project"]

        self.assertEqual(
            project["optional-dependencies"]["notebook"],
            ["anywidget==0.11.0"],
        )
        self.assertFalse(
            any(
                requirement.partition(";")[0]
                .partition("[")[0]
                .split("=", maxsplit=1)[0]
                .strip()
                .lower()
                == "anywidget"
                for requirement in project["dependencies"]
            )
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
Provides-Extra: gmsh
Provides-Extra: matplotlib
Provides-Extra: torch
Requires-Dist: numpy<3,>=2.1
Requires-Dist: gmsh==4.15.2 ; extra == 'gmsh'
Requires-Dist: torch>=2.13,<2.14; extra == "torch"
Requires-Dist: jax==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: jaxlib==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: matplotlib==3.11.1; extra == "matplotlib"

typed candidate
"""
        with tempfile.TemporaryDirectory() as temporary:
            wheel = Path(temporary) / exact_wheel_name("313")
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
                    "eqiora/examples/steady-flow-past-cylinder.eqi",
                    "eqiora/examples/mixed-boundary-elasticity.eqi",
                    "eqiora/examples/fixed-reference-fsi.eqi",
                    "eqiora/_eqiora.cpython-313-x86_64-linux-gnu.so",
                    f"{dist_info}sboms/eqiora-python.cyclonedx.json",
                ):
                    archive.writestr(name, b"")
                archive.writestr(f"{dist_info}METADATA", metadata)
                archive.writestr(
                    f"{dist_info}WHEEL",
                    maturin_wheel_payload("313"),
                )
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
        self.assertEqual(record["filename"], exact_wheel_name("313"))
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
Provides-Extra: gmsh
Provides-Extra: matplotlib
Provides-Extra: torch
Requires-Dist: numpy<3,>=2.1
Requires-Dist: gmsh==4.15.2; extra == "gmsh"
Requires-Dist: torch>=2.13,<2.14
Requires-Dist: jax==0.11.0; extra == "jax"
Requires-Dist: jaxlib==0.11.0; extra == "jax"
Requires-Dist: matplotlib==3.11.1; extra == "matplotlib"

invalid candidate
"""
        with tempfile.TemporaryDirectory() as temporary:
            wheel = Path(temporary) / exact_wheel_name("313")
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
                    "eqiora/examples/steady-flow-past-cylinder.eqi",
                    "eqiora/examples/mixed-boundary-elasticity.eqi",
                    "eqiora/examples/fixed-reference-fsi.eqi",
                    "eqiora/_eqiora.cpython-313-x86_64-linux-gnu.so",
                    f"{dist_info}sboms/eqiora-python.cyclonedx.json",
                ):
                    archive.writestr(name, b"")
                archive.writestr(f"{dist_info}METADATA", metadata)
                archive.writestr(
                    f"{dist_info}WHEEL",
                    maturin_wheel_payload("313"),
                )
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

    def test_notebook_wheel_contract_requires_exact_metadata_and_assets(self) -> None:
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
Provides-Extra: gmsh
Provides-Extra: matplotlib
Provides-Extra: notebook
Provides-Extra: torch
Requires-Dist: numpy<3,>=2.1
Requires-Dist: gmsh==4.15.2; extra == "gmsh"
Requires-Dist: torch>=2.13,<2.14; extra == "torch"
Requires-Dist: jax==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: jaxlib==0.11.0; python_version >= "3.12" and extra == "jax"
Requires-Dist: matplotlib==3.11.1; extra == "matplotlib"
Requires-Dist: anywidget == 0.11.0 ; extra == "notebook"

N1 candidate
"""
        notebook_assets = {
            "eqiora/_presentation/static/mesh-view.mjs": b"module\n",
            "eqiora/_presentation/static/mesh-view.css": b"style\n",
        }
        with tempfile.TemporaryDirectory() as temporary:
            wheel = Path(temporary) / exact_wheel_name("313")
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
                    "eqiora/examples/steady-flow-past-cylinder.eqi",
                    "eqiora/examples/mixed-boundary-elasticity.eqi",
                    "eqiora/examples/fixed-reference-fsi.eqi",
                    "eqiora/_eqiora.cpython-313-x86_64-linux-gnu.so",
                    f"{dist_info}sboms/eqiora-python.cyclonedx.json",
                ):
                    archive.writestr(name, b"")
                for name, payload in notebook_assets.items():
                    archive.writestr(name, payload)
                archive.writestr(f"{dist_info}METADATA", metadata)
                archive.writestr(
                    f"{dist_info}WHEEL",
                    maturin_wheel_payload("313"),
                )
                archive.writestr(f"{dist_info}licenses/LICENSE", license_bytes)
                archive.writestr(f"{dist_info}licenses/NOTICE", notice_bytes)

            version, _ = inspect_wheel(
                wheel,
                python_version="3.13",
                config=self.config(),
                license_bytes=license_bytes,
                notice_bytes=notice_bytes,
                notebook_assets=notebook_assets,
            )

        self.assertEqual(version, "0.1.0a1")

    def test_notebook_requirement_parses_equivalent_quotes_and_rejects_drift(
        self,
    ) -> None:
        for declaration in (
            "anywidget==0.11.0; extra == 'notebook'",
            'anywidget == 0.11.0 ; extra == "notebook"',
        ):
            with self.subTest(accepted=declaration):
                self.assertTrue(
                    python_candidate_module._has_exact_notebook_anywidget_requirement(
                        [declaration]
                    )
                )

        rejected = {
            "wrong-name": "another-widget==0.11.0; extra == 'notebook'",
            "wrong-specifier": "anywidget==0.11.1; extra == 'notebook'",
            "url": "anywidget @ https://example.invalid/anywidget.whl; extra == 'notebook'",
            "extras": "anywidget[testing]==0.11.0; extra == 'notebook'",
            "missing-marker": "anywidget==0.11.0",
            "wrong-marker": "anywidget==0.11.0; extra == 'studio'",
            "duplicate": "anywidget==0.11.0; extra == 'notebook'",
        }
        for name, declaration in rejected.items():
            dependencies = [declaration]
            if name == "duplicate":
                dependencies.append(declaration)
            with self.subTest(rejected=name):
                self.assertFalse(
                    python_candidate_module._has_exact_notebook_anywidget_requirement(
                        dependencies
                    )
                )

    def test_wheel_build_retains_exact_maturin_compressed_names_and_bytes(
        self,
    ) -> None:
        config = self.config()
        interpreters = {
            version: f"/managed/python{version}" for version in config.interpreters
        }
        commands: list[list[str]] = []
        producer_hashes: dict[str, str] = {}
        build_return_hashes: dict[str, str] = {}
        admission_hashes: dict[str, str] = {}
        producer_identities: dict[str, tuple[bytes, bytes, bytes]] = {}
        build_return_identities: dict[str, tuple[bytes, bytes, bytes]] = {}
        admission_identities: dict[str, tuple[bytes, bytes, bytes]] = {}
        sealed_wheel_paths: set[Path] = set()
        returned_wheel_names: tuple[str, ...] = ()
        expected_inventory: tuple[dict[str, object], ...] = ()
        wheel_contract_observations: dict[
            str, tuple[int, tuple[str, ...], set[str], str]
        ] = {}
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            output = root / "artifacts"
            output.mkdir()
            scratch = root / "scratch"
            extracted = root / "source" / f"eqiora-{config.python_version}"
            extracted.mkdir(parents=True)

            def checked_run(arguments: list[str], **_kwargs: object) -> str:
                commands.append(arguments)
                if "sdist" in arguments:
                    (output / f"eqiora-{config.python_version}.tar.gz").write_bytes(
                        b"sdist"
                    )
                    return ""
                compatibility = arguments[arguments.index("--compatibility") + 1]
                interpreter = arguments[arguments.index("--interpreter") + 1]
                version = next(
                    name for name, path in interpreters.items() if path == interpreter
                )
                compact = version.replace(".", "")
                self.assertEqual(compatibility, "manylinux_2_17")
                wheel = output / exact_wheel_name(
                    compact,
                    version=config.python_version,
                )
                write_maturin_wheel(wheel, compact)
                producer_identities[version] = wheel_byte_identity(wheel)
                producer_hashes[version] = hashlib.sha256(
                    producer_identities[version][0]
                ).hexdigest()
                sealed_wheel_paths.add(wheel.resolve())
                return ""

            with (
                reject_post_producer_wheel_writes(sealed_wheel_paths),
                mock.patch.object(
                    python_candidate_module, "checked_run", side_effect=checked_run
                ),
                mock.patch.object(
                    python_candidate_module,
                    "safe_extract_sdist",
                    return_value=extracted,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "cargo_workspace_version",
                    return_value=config.cargo_version,
                ),
                mock.patch.object(
                    Path,
                    "rename",
                    side_effect=AssertionError("wheel rename is forbidden"),
                ),
                mock.patch.object(
                    Path,
                    "replace",
                    side_effect=AssertionError("wheel replace is forbidden"),
                ),
                mock.patch.object(
                    Path,
                    "hardlink_to",
                    side_effect=AssertionError("wheel link is forbidden"),
                ),
                mock.patch.object(
                    Path,
                    "symlink_to",
                    side_effect=AssertionError("wheel link is forbidden"),
                ),
                mock.patch.object(
                    os,
                    "rename",
                    side_effect=AssertionError("wheel rename is forbidden"),
                ),
                mock.patch.object(
                    os,
                    "replace",
                    side_effect=AssertionError("wheel replace is forbidden"),
                ),
                mock.patch.object(
                    os,
                    "link",
                    side_effect=AssertionError("wheel link is forbidden"),
                ),
                mock.patch.object(
                    os,
                    "symlink",
                    side_effect=AssertionError("wheel link is forbidden"),
                ),
                mock.patch(
                    "shutil.copy",
                    side_effect=AssertionError("wheel copy is forbidden"),
                ),
                mock.patch(
                    "shutil.copy2",
                    side_effect=AssertionError("wheel copy is forbidden"),
                ),
                mock.patch(
                    "shutil.copyfile",
                    side_effect=AssertionError("wheel copy is forbidden"),
                ),
                mock.patch(
                    "shutil.copytree",
                    side_effect=AssertionError("wheel copy is forbidden"),
                ),
            ):
                _sdist, wheels, _extracted = python_candidate_module.build_artifacts(
                    output=output,
                    scratch=scratch,
                    config=config,
                    uv="/managed/uv",
                    interpreters=interpreters,
                )
                build_return_identities = {
                    version: wheel_byte_identity(wheel)
                    for version, wheel in wheels.items()
                }
                build_return_hashes = {
                    version: hashlib.sha256(identity[0]).hexdigest()
                    for version, identity in build_return_identities.items()
                }
                executor = importlib.import_module("python_candidate_h2")
                admitted = executor.admit_candidate_family(output)
                admitted_inventory = admitted.inventory
                expected_inventory = H2ExecutionBoundaryTests.expected_family_inventory(
                    output
                )
                returned_wheel_names = tuple(path.name for path in wheels.values())
                admission_identities = {
                    version: wheel_byte_identity(wheel)
                    for version, wheel in wheels.items()
                }
                admission_hashes = {
                    version: hashlib.sha256(identity[0]).hexdigest()
                    for version, identity in admission_identities.items()
                }
                for version, wheel in wheels.items():
                    compact = version.replace(".", "")
                    _name, _parsed_version, _build, filename_tags = (
                        parse_wheel_filename(wheel.name)
                    )
                    payload = admission_identities[version][1]
                    internal_tags = tuple(
                        line.removeprefix("Tag: ")
                        for line in payload.decode("utf-8").splitlines()
                        if line.startswith("Tag: ")
                    )
                    wheel_contract_observations[version] = (
                        len(payload),
                        internal_tags,
                        {str(tag) for tag in filename_tags},
                        hashlib.sha256(payload).hexdigest(),
                    )

        wheel_commands = [arguments for arguments in commands if "build" in arguments]
        self.assertEqual(len(wheel_commands), 4)
        for version, arguments in zip(config.interpreters, wheel_commands, strict=True):
            with self.subTest(interpreter=version):
                build_index = arguments.index("build")
                target_dir = str(scratch / "cargo-target")
                self.assertEqual(
                    arguments[:build_index],
                    [
                        "/managed/uv",
                        "tool",
                        "run",
                        "--from",
                        "maturin[zig]==1.14.1",
                        "maturin",
                    ],
                )
                self.assertEqual(
                    arguments[build_index:],
                    [
                        "build",
                        "--release",
                        "--zig",
                        "--compatibility",
                        "manylinux_2_17",
                        "--auditwheel",
                        "check",
                        "--interpreter",
                        interpreters[version],
                        "--target-dir",
                        target_dir,
                        "--out",
                        str(output),
                    ],
                )
                self.assertNotIn("manylinux2014", arguments)
                self.assertEqual(Path(target_dir), scratch / "cargo-target")
        self.assertEqual(
            returned_wheel_names,
            tuple(
                exact_wheel_name(
                    version.replace(".", ""),
                    version=config.python_version,
                )
                for version in config.interpreters
            ),
        )
        self.assertEqual(producer_hashes, build_return_hashes)
        self.assertEqual(build_return_hashes, admission_hashes)
        self.assertEqual(producer_identities, build_return_identities)
        self.assertEqual(build_return_identities, admission_identities)
        self.assertEqual(
            admitted_inventory,
            expected_inventory,
        )
        for version in config.interpreters:
            compact = version.replace(".", "")
            (
                payload_size,
                internal_tags,
                filename_tags,
                payload_sha256,
            ) = wheel_contract_observations[version]
            self.assertEqual(payload_size, 147)
            self.assertEqual(internal_tags, exact_wheel_tags(compact))
            self.assertEqual(
                filename_tags,
                set(internal_tags),
            )
            self.assertEqual(
                payload_sha256,
                EXACT_WHEEL_PAYLOAD_SHA256[compact],
            )
            self.assertEqual(
                admission_identities[version][2],
                maturin_record_payload(compact),
            )

    def test_post_producer_observer_rejects_byte_identical_wheel_rewrites(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            wheel = Path(temporary) / exact_wheel_name("311")
            write_maturin_wheel(wheel, "311")
            expected = wheel_byte_identity(wheel)
            sealed = {wheel.resolve()}
            rename_source = Path(temporary) / "byte-identical-rename-source.whl"
            replace_source = Path(temporary) / "byte-identical-replace-source.whl"
            rename_source.write_bytes(expected[0])
            replace_source.write_bytes(expected[0])

            def write_with_builtin_open() -> None:
                with builtins.open(wheel, mode="r+b") as destination:
                    destination.write(expected[0])

            def write_with_io_open() -> None:
                with io.open(wheel, mode="r+b") as destination:
                    destination.write(expected[0])

            def write_with_os_open() -> None:
                descriptor = os.open(wheel, os.O_WRONLY)
                os.close(descriptor)

            def write_with_zipfile() -> None:
                with zipfile.ZipFile(wheel, mode="a"):
                    pass

            rewrites: dict[str, Callable[[], object]] = {
                "Path.write_bytes": lambda: wheel.write_bytes(expected[0]),
                "builtins.open": write_with_builtin_open,
                "io.open": write_with_io_open,
                "os.open": write_with_os_open,
                "os.rename": lambda: os.rename(rename_source, wheel),
                "os.replace": lambda: os.replace(replace_source, wheel),
                "zipfile.ZipFile": write_with_zipfile,
            }
            for operation, rewrite in rewrites.items():
                with (
                    self.subTest(operation=operation),
                    reject_post_producer_wheel_writes(sealed),
                ):
                    with self.assertRaisesRegex(
                        AssertionError,
                        "post-producer wheel rewrite is forbidden",
                    ):
                        rewrite()
                self.assertEqual(wheel_byte_identity(wheel), expected)

    def test_producer_rejects_nonexact_physical_name_before_twine_or_rewrite(
        self,
    ) -> None:
        config = self.config()
        interpreters = {
            version: f"/managed/python{version}" for version in config.interpreters
        }
        mutant_names = {
            "old-canonical-only-optional-alias": (
                "eqiora-0.1.0a1-cp311-cp311-manylinux_2_17_x86_64.whl"
            ),
            "alias-first": (
                "eqiora-0.1.0a1-cp311-cp311-"
                "manylinux2014_x86_64.manylinux_2_17_x86_64.whl"
            ),
            "broadened-dotted-suffix": (
                "eqiora-0.1.0a1-cp311-cp311-"
                f"{EXACT_PHYSICAL_PLATFORM}.manylinux_2_28_x86_64.whl"
            ),
        }
        for mutant, filename in mutant_names.items():
            with (
                self.subTest(mutant=mutant),
                tempfile.TemporaryDirectory(dir=Path.home()) as temporary,
            ):
                root = Path(temporary)
                output = root / "artifacts"
                output.mkdir()
                scratch = root / "scratch"
                extracted = root / "source" / f"eqiora-{config.python_version}"
                extracted.mkdir(parents=True)
                commands: list[list[str]] = []

                def checked_run(arguments: list[str], **_kwargs: object) -> str:
                    commands.append(arguments)
                    if "sdist" in arguments:
                        (output / f"eqiora-{config.python_version}.tar.gz").write_bytes(
                            b"sdist"
                        )
                    elif "build" in arguments:
                        write_maturin_wheel(output / filename, "311")
                    return ""

                with (
                    mock.patch.object(
                        python_candidate_module,
                        "checked_run",
                        side_effect=checked_run,
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "safe_extract_sdist",
                        return_value=extracted,
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "cargo_workspace_version",
                        return_value=config.cargo_version,
                    ),
                    mock.patch.object(
                        Path,
                        "rename",
                        side_effect=AssertionError("rename cannot repair the mutant"),
                    ),
                    mock.patch.object(
                        Path,
                        "replace",
                        side_effect=AssertionError("replace cannot repair the mutant"),
                    ),
                    mock.patch.object(
                        os,
                        "rename",
                        side_effect=AssertionError("rename cannot repair the mutant"),
                    ),
                    mock.patch.object(
                        os,
                        "replace",
                        side_effect=AssertionError("replace cannot repair the mutant"),
                    ),
                    mock.patch.object(
                        os,
                        "link",
                        side_effect=AssertionError("link cannot repair the mutant"),
                    ),
                    mock.patch(
                        "shutil.copy",
                        side_effect=AssertionError("copy cannot repair the mutant"),
                    ),
                    mock.patch(
                        "shutil.copy2",
                        side_effect=AssertionError("copy cannot repair the mutant"),
                    ),
                    mock.patch(
                        "shutil.copyfile",
                        side_effect=AssertionError("copy cannot repair the mutant"),
                    ),
                ):
                    with self.assertRaises(CandidateError):
                        python_candidate_module.build_artifacts(
                            output=output,
                            scratch=scratch,
                            config=config,
                            uv="/managed/uv",
                            interpreters=interpreters,
                        )

                wheel_builds = [call for call in commands if "build" in call]
                twine_calls = [call for call in commands if "twine" in call]
                self.assertEqual(len(wheel_builds), 1)
                self.assertEqual(twine_calls, [])
                self.assertEqual(
                    {path.name for path in output.iterdir()},
                    {f"eqiora-{config.python_version}.tar.gz", filename},
                )

    def test_producer_output_collision_fails_without_overwrite_or_cleanup(self) -> None:
        config = self.config()
        collisions = (
            "eqiora-0.1.0a1-cp311-cp311-manylinux_2_17_x86_64.whl",
            exact_wheel_name("311"),
            ".eqiora-0.1.0a1-cp311.partial.whl",
        )
        for filename in collisions:
            with (
                self.subTest(filename=filename),
                tempfile.TemporaryDirectory(dir=Path.home()) as temporary,
            ):
                root = Path(temporary)
                output = root / "artifacts"
                output.mkdir()
                collision = output / filename
                collision.write_bytes(b"pre-existing bytes")
                checked = mock.Mock()
                with mock.patch.object(
                    python_candidate_module,
                    "checked_run",
                    checked,
                ):
                    with self.assertRaises(CandidateError):
                        python_candidate_module.build_artifacts(
                            output=output,
                            scratch=root / "scratch",
                            config=config,
                            uv="/managed/uv",
                            interpreters={
                                version: f"/managed/python{version}"
                                for version in config.interpreters
                            },
                        )

                checked.assert_not_called()
                self.assertEqual(collision.read_bytes(), b"pre-existing bytes")
                self.assertEqual(tuple(output.iterdir()), (collision,))

    def test_notebook_wheel_asset_inventory_is_closed(self) -> None:
        expected = {
            "eqiora/_presentation/static/mesh-view.mjs": b"module\n",
            "eqiora/_presentation/static/mesh-view.css": b"style\n",
        }
        for mutation in ("missing", "empty", "extra", "modified"):
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory() as temporary,
            ):
                wheel = Path(temporary) / exact_wheel_name("313")
                members = dict(expected)
                if mutation == "missing":
                    members.pop("eqiora/_presentation/static/mesh-view.css")
                elif mutation == "empty":
                    members["eqiora/_presentation/static/mesh-view.css"] = b""
                elif mutation == "extra":
                    members["eqiora/_presentation/static/unreviewed.js"] = b"x"
                else:
                    members["eqiora/_presentation/static/mesh-view.mjs"] = b"changed"
                with zipfile.ZipFile(wheel, mode="w") as archive:
                    for name, payload in members.items():
                        archive.writestr(name, payload)

                validator = getattr(
                    python_candidate_module,
                    "verify_notebook_asset_inventory",
                )
                with self.assertRaises(CandidateError):
                    validator(wheel, expected)

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
                *(str(resource) for resource in PYTHON_TEST_RESOURCES),
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
        "notebook-3.13",
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

    def test_base_profile_dispatch_excludes_notebook_authority_inputs(self) -> None:
        profiles = self.profiles_module()
        config = self.config()
        base = mock.create_autospec(
            python_candidate_module.run_base_profile,
            return_value=["check:base-3.11"],
        )

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = profiles.build_profile_plan(
                root, config, skip_extras=False
            )[0]
            extracted = root / "source"
            wheel = root / "candidate-cp311.whl"
            with mock.patch.object(
                python_candidate_module,
                "run_base_profile",
                new=base,
            ):
                profile_receipt = python_candidate_module.execute_profile(
                    workspace,
                    uv="/reviewed/uv",
                    config=config,
                    wheels={"3.11": wheel},
                    extracted=extracted,
                    interpreters={"3.11": "/reviewed/python3.11"},
                    receipt=mock.sentinel.validated_receipt,
                    frontend=mock.sentinel.derived_frontend,
                )

        self.assertEqual(profile_receipt.checks, ("check:base-3.11",))
        base.assert_called_once_with(
            uv="/reviewed/uv",
            interpreter="/reviewed/python3.11",
            python_version="3.11",
            wheel=wheel,
            extracted=extracted,
            workspace=workspace,
            config=config,
        )

    def test_base_profile_receipts_gmsh_only_after_base_checks_and_mesh_tests(
        self,
    ) -> None:
        profiles = self.profiles_module()
        config = self.config()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = profiles.build_profile_plan(
                root, config, skip_extras=False
            )[0]
            wheel = root / "eqiora-cp311.whl"
            extracted = root / "source"
            python = workspace.environment / "bin/python"
            events = mock.Mock()
            install = events.install_environment
            install.return_value = python
            run = events.run
            run.return_value = ""
            public_smoke = events.run_public_smoke

            with (
                mock.patch.object(profiles, "install_environment", new=install),
                mock.patch.object(
                    profiles,
                    "prepare_base_consumer_tree",
                    return_value=(
                        workspace.consumer / "tests",
                        workspace.consumer / "typecheck",
                    ),
                ),
                mock.patch.object(
                    profiles, "prepare_exact_cylinder_demo_consumer"
                ),
                mock.patch.object(
                    profiles, "prepare_mixed_boundary_elasticity_demo_consumer"
                ),
                mock.patch.object(
                    profiles, "prepare_fixed_reference_fsi_demo_consumer"
                ),
                mock.patch.object(profiles, "assert_installed_origin"),
                mock.patch.object(profiles, "assert_matplotlib_is_optional"),
                mock.patch.object(profiles, "run_public_smoke", new=public_smoke),
            ):
                checks = profiles.run_base_profile(
                    uv="/reviewed/uv",
                    interpreter="/reviewed/python3.11",
                    python_version="3.11",
                    wheel=wheel,
                    extracted=extracted,
                    workspace=workspace,
                    config=config,
                    run=run,
                )

        tests = workspace.consumer / "tests"
        typecheck = workspace.consumer / "typecheck"
        gmsh_tests = tuple(
            tests / name
            for name in (
                "test_gmsh_meshing.py",
                "test_exact_cylinder_stokes_result.py",
            )
        )
        gmsh_path = str(python.parent)
        if inherited_path := os.environ.get("PATH"):
            gmsh_path = os.pathsep.join((gmsh_path, inherited_path))
        self.assertEqual(
            checks,
            [
                "cp311:installed-wheel",
                "cp311:base-and-numpy",
                "cp311:packaged-exact-cylinder-model-demo",
                "cp311:packaged-mixed-boundary-elasticity-demo",
                "cp311:packaged-fixed-reference-fsi-demo",
                "cp311:async-and-cancellation",
                "cp311:strict-base-typing",
                "cp311:public-smoke-base",
                "cp311:matplotlib-free-base",
            ],
        )
        self.assertEqual(
            events.mock_calls,
            [
                mock.call.install_environment(
                    uv="/reviewed/uv",
                    interpreter="/reviewed/python3.11",
                    environment=workspace.environment,
                    requirements=[str(wheel), config.pytest, config.mypy],
                    run=run,
                ),
                mock.call.run(
                    [
                        str(python),
                        "-I",
                        "-m",
                        "pytest",
                        "-q",
                        str(tests),
                        "--ignore",
                        str(gmsh_tests[0]),
                        "--ignore",
                        str(gmsh_tests[1]),
                    ],
                    cwd=workspace.consumer,
                ),
                mock.call.run(
                    [
                        str(python),
                        "-I",
                        "-m",
                        "mypy",
                        "--strict",
                        str(typecheck / "base.py"),
                    ],
                    cwd=workspace.consumer,
                ),
                mock.call.run_public_smoke(
                    python=python,
                    extracted=extracted,
                    run_root=workspace.consumer,
                    expected_version=config.python_version,
                    profile="base",
                    run=run,
                ),
                mock.call.run(
                    [
                        "/reviewed/uv",
                        "pip",
                        "install",
                        "--python",
                        str(python),
                        f"{wheel}[gmsh]",
                    ],
                    cwd=workspace.environment.parent,
                ),
                mock.call.run(
                    [
                        str(python),
                        "-I",
                        "-m",
                        "pytest",
                        "-q",
                        *(str(test) for test in gmsh_tests),
                    ],
                    cwd=workspace.consumer,
                    extra_environment={
                        "EQIORA_GMSH": str(
                            python.parent / ("gmsh.exe" if os.name == "nt" else "gmsh")
                        ),
                        "PATH": gmsh_path,
                    },
                ),
            ],
        )

    def test_base_public_smoke_rejects_gmsh_imported_with_eqiora(self) -> None:
        smoke = importlib.import_module("python_public_smoke")
        expected_version = "0.1.0a1"

        def reached_base_execution(*_args: object, **_kwargs: object) -> object:
            raise RuntimeError("base smoke continued after importing Gmsh")

        eqiora = types.SimpleNamespace(
            __version__=expected_version,
            Field=reached_base_execution,
        )
        real_import = builtins.__import__

        def import_with_gmsh(
            name: str,
            globals: object = None,
            locals: object = None,
            fromlist: object = (),
            level: int = 0,
        ) -> object:
            if name == "eqiora":
                sys.modules["gmsh"] = types.ModuleType("gmsh")
                return eqiora
            return real_import(name, globals, locals, fromlist, level)

        with (
            mock.patch.dict(sys.modules, {"eqiora": eqiora}),
            mock.patch.object(builtins, "__import__", new=import_with_gmsh),
            mock.patch.object(
                smoke.importlib.metadata,
                "version",
                return_value=expected_version,
            ),
        ):
            sys.modules.pop("gmsh", None)
            try:
                smoke.base_smoke(expected_version)
            except AssertionError:
                pass
            except RuntimeError as error:
                self.fail(str(error))
            else:  # pragma: no cover - the fake base execution always stops
                self.fail("base smoke accepted an Eqiora import that loaded Gmsh")

    def test_notebook_profile_dispatch_requires_validated_authority_pair(self) -> None:
        profiles = self.profiles_module()
        config = self.config()
        validated_receipt = mock.sentinel.validated_receipt
        derived_frontend = mock.sentinel.derived_frontend
        notebook = mock.create_autospec(
            python_candidate_module.run_notebook_profile,
            return_value=["check:notebook-3.13"],
        )

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            notebook_workspace = profiles.build_profile_plan(
                root, config, skip_extras=False
            )[6]
            source = root / "source"
            candidate_wheel = root / "candidate-cp313.whl"
            with mock.patch.object(
                python_candidate_module,
                "run_notebook_profile",
                new=notebook,
            ):
                profile_receipt = python_candidate_module.execute_profile(
                    notebook_workspace,
                    uv="/reviewed/uv",
                    config=config,
                    wheels={"3.13": candidate_wheel},
                    extracted=source,
                    interpreters={"3.13": "/reviewed/python3.13"},
                    receipt=validated_receipt,
                    frontend=derived_frontend,
                )

        self.assertEqual(profile_receipt.checks, ("check:notebook-3.13",))
        notebook.assert_called_once_with(
            uv="/reviewed/uv",
            interpreter="/reviewed/python3.13",
            wheel=candidate_wheel,
            extracted=source,
            workspace=notebook_workspace,
            config=config,
            receipt=validated_receipt,
            frontend=derived_frontend,
        )

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

        def notebook(**_kwargs: object) -> list[str]:
            name = "notebook-3.13"
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
                python_candidate_module,
                "run_notebook_profile",
                side_effect=notebook,
                create=True,
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

            config = self.config()
            workspace = next(item for item in plan if item.name == "matplotlib-3.13")
            extracted = scratch / "source"
            source_test = extracted / "bindings/python/tests/test_matplotlib.py"
            source_test.parent.mkdir(parents=True)
            source_test.write_text(
                "def test_placeholder():\n    pass\n", encoding="utf-8"
            )
            python = workspace.environment / (
                "Scripts/python.exe" if os.name == "nt" else "bin/python"
            )
            demos = tuple(
                workspace.consumer / name for name in ("exact.py", "mixed.py", "fsi.py")
            )
            install = mock.Mock(return_value=python)

            def run(argv: list[str], **_kwargs: object) -> str:
                for value in argv:
                    output = Path(value)
                    if output.is_absolute() and output.suffix == ".png":
                        output.write_bytes(b"\x89PNG\r\n\x1a\n")
                return ""

            checked = mock.Mock(side_effect=run)
            with (
                mock.patch.object(profiles, "install_environment", new=install),
                mock.patch.object(
                    profiles,
                    "prepare_exact_cylinder_demo_consumer",
                    return_value=demos[0],
                ),
                mock.patch.object(
                    profiles,
                    "prepare_mixed_boundary_elasticity_demo_consumer",
                    return_value=demos[1],
                ),
                mock.patch.object(
                    profiles,
                    "prepare_fixed_reference_fsi_demo_consumer",
                    return_value=demos[2],
                ),
                mock.patch.dict(
                    os.environ,
                    {"EQIORA_GMSH": "/ambient/gmsh", "PATH": "/ambient/bin"},
                ),
            ):
                checks = profiles.run_optional_profile(
                    name="matplotlib",
                    uv="/reviewed/uv",
                    interpreter="/reviewed/python3.13",
                    wheel=scratch / "candidate.whl",
                    extracted=extracted,
                    workspace=workspace,
                    config=config,
                    run=checked,
                )

            self.assertEqual(
                checks,
                [
                    "cp313:matplotlib",
                    "cp313:packaged-exact-cylinder-pressure-demo",
                    "cp313:packaged-mixed-boundary-displacement-demo",
                    "cp313:packaged-fixed-reference-fsi-still",
                ],
            )
            install.assert_called_once_with(
                uv="/reviewed/uv",
                interpreter="/reviewed/python3.13",
                environment=workspace.environment,
                requirements=[
                    f"{scratch / 'candidate.whl'}[gmsh,matplotlib]",
                    config.pytest,
                    config.matplotlib,
                ],
                run=checked,
            )
            self.assertEqual(
                [tuple(call.args[0][2:4]) for call in checked.call_args_list],
                [
                    ("-m", "pytest"),
                    (str(demos[0]), "--pressure-png"),
                    (str(demos[1]), "--displacement-png"),
                    (str(demos[2]), "--fsi-png"),
                ],
            )
            expected_environment = {
                "EQIORA_GMSH": str(
                    python.parent / ("gmsh.exe" if os.name == "nt" else "gmsh")
                ),
                "PATH": os.pathsep.join((str(python.parent), "/ambient/bin")),
            }
            for index, call in enumerate(checked.call_args_list):
                with self.subTest(matplotlib_run=index):
                    self.assertEqual(
                        call.kwargs.get("extra_environment"), expected_environment
                    )

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


class H2ExecutionBoundaryTests(unittest.TestCase):
    REVISION = "1" * 40
    H2_EXECUTOR = REPOSITORY_ROOT / "tools/release/python_candidate_h2.py"
    playwright_core_archive_bytes: bytes | None = None

    def playwright_workspace(self, root: Path) -> object:
        executor = importlib.import_module("python_candidate_h2")
        workspace = executor.create_isolated_build_workspaces(root / "builds")[0]
        workspace.frontend.mkdir()
        return workspace

    def install_locked_playwright_core(
        self, workspace: object
    ) -> tuple[dict[str, object], Path, Path]:
        lock_path = REPOSITORY_ROOT / "bindings/python/frontend/package-lock.json"
        self.assertEqual(
            hashlib.sha256(lock_path.read_bytes()).hexdigest(),
            PLAYWRIGHT_CORE_LOCK_SHA256,
        )
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
        entry = lock["packages"]["node_modules/playwright-core"]
        self.assertEqual(
            entry,
            {
                "version": "1.62.1",
                "resolved": PLAYWRIGHT_CORE_URL,
                "integrity": PLAYWRIGHT_CORE_INTEGRITY,
                "dev": True,
                "license": "Apache-2.0",
                "bin": {"playwright-core": "cli.js"},
                "engines": {"node": ">=20"},
            },
        )
        package_archive = type(self).playwright_core_archive_bytes
        if package_archive is None:
            with urllib.request.urlopen(PLAYWRIGHT_CORE_URL, timeout=60) as response:
                package_archive = response.read()
            type(self).playwright_core_archive_bytes = package_archive
        observed_integrity = "sha512-" + base64.b64encode(
            hashlib.sha512(package_archive).digest()
        ).decode("ascii")
        self.assertEqual(observed_integrity, PLAYWRIGHT_CORE_INTEGRITY)

        archive_path = Path(workspace.root) / "playwright-core-1.62.1.tgz"
        archive_path.write_bytes(package_archive)
        extraction_root = Path(workspace.root) / "playwright-core-package"
        extraction_root.mkdir()
        with tarfile.open(
            fileobj=io.BytesIO(package_archive),
            mode="r:gz",
        ) as archive:
            members = archive.getmembers()
            for member in members:
                relative = Path(member.name)
                self.assertFalse(relative.is_absolute())
                self.assertNotIn("..", relative.parts)
                self.assertTrue(member.isdir() or member.isfile())
                target = extraction_root / relative
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                    continue
                source = archive.extractfile(member)
                self.assertIsNotNone(source)
                target.parent.mkdir(parents=True, exist_ok=True)
                with target.open("wb") as output:
                    shutil.copyfileobj(source, output)  # type: ignore[arg-type]

        package = extraction_root / "package"
        self.assertTrue(package.is_dir())
        self.assertFalse(package.is_symlink())
        self.assertEqual(
            hashlib.sha256((package / "package.json").read_bytes()).hexdigest(),
            PLAYWRIGHT_CORE_PACKAGE_SHA256,
        )
        self.assertEqual(
            hashlib.sha256((package / "browsers.json").read_bytes()).hexdigest(),
            PLAYWRIGHT_CORE_BROWSERS_SHA256,
        )
        self.assertEqual(
            hashlib.sha256((package / "lib/coreBundle.js").read_bytes()).hexdigest(),
            PLAYWRIGHT_CORE_BUNDLE_SHA256,
        )
        self.assertFalse((package / "lib/server/registry/index.js").exists())
        return lock, package, archive_path

    @staticmethod
    def expected_playwright_observation(browser_cache: Path) -> dict[str, object]:
        directory = browser_cache / "chromium_headless_shell-1234"
        return {
            "name": "chromium-headless-shell",
            "browserName": "chromium",
            "revision": "1234",
            "browserVersion": "151.0.7922.34",
            "installType": "download-by-default",
            "directory": str(directory),
            "executablePath": str(directory / PLAYWRIGHT_BROWSER_MEMBER),
            "downloadURLs": [PLAYWRIGHT_BROWSER_URL],
        }

    @staticmethod
    def playwright_probe(
        node: Path,
        package: Path,
        browser_cache: Path,
        program: str,
    ) -> subprocess.CompletedProcess[str]:
        environment = {
            "HOME": str(browser_cache.parent / "home"),
            "LANG": "C.UTF-8",
            "LC_ALL": "C.UTF-8",
            "PATH": os.environ.get("PATH", ""),
            "PLAYWRIGHT_BROWSERS_PATH": str(browser_cache),
            "TZ": "UTC",
        }
        return subprocess.run(
            [str(node), "-e", program, str(package)],
            check=False,
            text=True,
            capture_output=True,
            env=environment,
        )

    @staticmethod
    def instrumented_playwright_process(
        real_run: Callable[..., subprocess.CompletedProcess[str]],
        argv: list[str],
        kwargs: dict[str, object],
        mutation: tuple[str, object] | None = None,
    ) -> tuple[subprocess.CompletedProcess[str], tuple[str, ...]]:
        harness = r"""
const p=require('path'),program=process.argv[1],root=process.argv[2];
const mutation=JSON.parse(process.argv[3]);
const registry=require(p.join(root,'lib/coreBundle.js')).registry.registry;
const find=registry.findExecutable,calls=[];
registry.findExecutable=function(name){
  calls.push(name); const executable=find.call(this,name);
  if(mutation===null)return executable;
  return new Proxy(executable,{get(target,key,receiver){
    if(key===mutation.key)return key==='executablePath'?()=>mutation.value:mutation.value;
    return Reflect.get(target,key,receiver);
  }});
};
const argv=process.argv,write=process.stdout.write.bind(process.stdout); let output='',failure=null;
process.argv=[argv[0],root]; process.stdout.write=chunk=>{output+=String(chunk);return true;};
try{eval(program);}catch(error){failure=error&&error.stack?error.stack:String(error);}
finally{process.argv=argv;process.stdout.write=write;}
write(JSON.stringify({calls,output,failure}));
"""
        completed = real_run(
            [
                argv[0],
                "-e",
                harness,
                argv[2],
                argv[3],
                json.dumps(
                    None
                    if mutation is None
                    else {"key": mutation[0], "value": mutation[1]}
                ),
            ],
            **{**kwargs, "check": False},
        )
        if completed.returncode != 0:
            raise subprocess.CalledProcessError(
                completed.returncode,
                completed.args,
                output=completed.stdout,
                stderr=completed.stderr,
            )
        trace = json.loads(completed.stdout)
        if trace["failure"] is not None:
            raise subprocess.CalledProcessError(
                1, argv, output=trace["output"], stderr=trace["failure"]
            )
        return (
            subprocess.CompletedProcess(
                argv, 0, stdout=trace["output"], stderr=completed.stderr
            ),
            tuple(trace["calls"]),
        )

    @staticmethod
    def write_browser_archive(
        destination: Path,
        members: tuple[tuple[str, bytes, int], ...] | None = None,
    ) -> None:
        executable = b"#!/bin/sh\nprintf '%s\\n' 'HeadlessChrome 151.0.7922.34'\n"
        entries = members or ((PLAYWRIGHT_BROWSER_MEMBER, executable, 0o100755),)
        with warnings.catch_warnings():
            warnings.filterwarnings(
                "ignore", message="Duplicate name:", category=UserWarning
            )
            with zipfile.ZipFile(destination, mode="w") as archive:
                for name, payload, mode in entries:
                    member = zipfile.ZipInfo(name)
                    member.create_system = 3
                    member.external_attr = mode << 16
                    archive.writestr(member, payload)

    @staticmethod
    def structural_browser_inventory(
        archive_path: Path,
    ) -> tuple[dict[str, object], ...]:
        with zipfile.ZipFile(archive_path) as archive:
            records = (
                {
                    "path": member.filename,
                    "kind": "directory" if member.is_dir() else "file",
                    "external_attr": member.external_attr,
                    "compression": member.compress_type,
                    "compressed_size": member.compress_size,
                    "expanded_size": member.file_size,
                    "crc32": f"{member.CRC:08x}",
                }
                for member in archive.infolist()
            )
            return tuple(
                sorted(records, key=lambda record: str(record["path"]).encode())
            )

    @staticmethod
    def acquired_inputs(
        executor: object,
        root: Path,
        *,
        archive_sha256: str = "a" * 64,
        executable_sha256: str = "b" * 64,
    ) -> object:
        return executor.AcquiredInputs(
            node=root / "node",
            npm=root / "npm",
            npm_tarball=root / "npm.tgz",
            browser_archive=root / "browser.zip",
            browser_executable=root
            / "chromium_headless_shell-1234"
            / PLAYWRIGHT_BROWSER_MEMBER,
            browser_archive_sha256=archive_sha256,
            browser_executable_sha256=executable_sha256,
            browser_platform="linux-x86_64",
            playwright_test_integrity="sha512-test",
            playwright_core_integrity=PLAYWRIGHT_CORE_INTEGRITY,
            python_wheels=({"name": "anywidget", "sha256": "c" * 64},),
            anywidget_license_sha256="d" * 64,
            package_manifests=(
                ("node_modules/playwright-core", {"version": "1.62.1"}),
            ),
            package_packuments=(),
            locked_package_bytes=1,
            python_wheel_bytes=1,
            browser_member_count=287,
            browser_expanded_regular_bytes=273_378_828,
        )

    @staticmethod
    def diagnostic_acquired_inputs(executor: object, root: Path) -> object:
        return executor.AcquiredInputs(
            node=root / "private-host-canary" / "node",
            npm=root / "private-host-canary" / "npm",
            npm_tarball=root / "private-host-canary" / "npm.tgz",
            browser_archive=root / "private-host-canary" / "browser.zip",
            browser_executable=root
            / "private-host-canary"
            / "chromium_headless_shell-1234"
            / PLAYWRIGHT_BROWSER_MEMBER,
            browser_archive_sha256="a" * 64,
            browser_executable_sha256="b" * 64,
            browser_platform="linux-x86_64",
            playwright_test_integrity="sha512-playwright-test-canary",
            playwright_core_integrity="sha512-playwright-core-canary",
            python_wheels=(
                {
                    "name": "anywidget",
                    "version": "0.11.0",
                    "filename": "anywidget-0.11.0-py3-none-any.whl",
                    "sha256": "c" * 64,
                },
                {
                    "name": "marimo",
                    "version": "0.23.16",
                    "filename": "marimo-0.23.16-py3-none-any.whl",
                    "sha256": "d" * 64,
                },
            ),
            anywidget_license_sha256="e" * 64,
            package_manifests=(
                (
                    "node_modules/playwright-core",
                    {
                        "name": "playwright-core",
                        "version": "1.62.1",
                        "scripts": {},
                        "private": "manifest-secret-canary",
                    },
                ),
                (
                    "node_modules/unlisted-renderer",
                    {
                        "name": "unlisted-renderer",
                        "version": "9.9.9",
                        "description": "unchanged-manifest-peer-雪",
                    },
                ),
            ),
            package_packuments=(
                (
                    "playwright-core",
                    {
                        "name": "playwright-core",
                        "dist-tags": {"latest": "1.62.1"},
                        "versions": {
                            "1.62.1": {
                                "name": "playwright-core",
                                "version": "1.62.1",
                                "dist": {
                                    "tarball": (
                                        "https://registry.invalid/"
                                        "registry-token-canary/package.tgz"
                                    )
                                },
                                "authorization": "header-secret-canary",
                            }
                        },
                    },
                ),
                (
                    "unlisted-renderer",
                    {
                        "name": "unlisted-renderer",
                        "description": "unchanged-packument-peer-雪",
                        "versions": {
                            "9.9.9": {
                                "name": "unlisted-renderer",
                                "version": "9.9.9",
                            }
                        },
                    },
                ),
            ),
            locked_package_bytes=10,
            python_wheel_bytes=20,
            browser_member_count=287,
            browser_expanded_regular_bytes=273_378_828,
        )

    @staticmethod
    def independent_diagnostic_sha256(value: object) -> str:
        payload = json.dumps(
            value,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        return hashlib.sha256(payload).hexdigest()

    @classmethod
    def expected_acquisition_drift_message(
        cls,
        field: str,
        first: object,
        second: object,
        *,
        member_identity: str | None = None,
        first_member: object | None = None,
        second_member: object | None = None,
    ) -> str:
        members = ()
        if member_identity is not None:
            members = ((member_identity, first_member, second_member),)
        difference = cls.independent_acquisition_difference(
            field, first, second, members=members
        )
        document = json.dumps(
            {"differences": [difference]},
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        return f"H2 isolated runs acquired different external inputs: {document}"

    @classmethod
    def independent_acquisition_difference(
        cls,
        field: str,
        first: object,
        second: object,
        *,
        members: tuple[tuple[str, object | None, object | None], ...] = (),
    ) -> dict[str, object]:
        difference: dict[str, object] = {
            "field": field,
            "run_1_sha256": cls.independent_diagnostic_sha256(first),
            "run_2_sha256": cls.independent_diagnostic_sha256(second),
        }
        if members:
            difference["members"] = [
                {
                    "identity": identity,
                    "run_1_sha256": (
                        "absent"
                        if first_member is None
                        else cls.independent_diagnostic_sha256(first_member)
                    ),
                    "run_2_sha256": (
                        "absent"
                        if second_member is None
                        else cls.independent_diagnostic_sha256(second_member)
                    ),
                }
                for identity, first_member, second_member in members
            ]
        return difference

    @staticmethod
    def lifecycle_authority_inputs(
        lock: dict[str, object],
    ) -> tuple[
        tuple[tuple[str, dict[str, object]], ...],
        tuple[tuple[str, dict[str, object]], ...],
    ]:
        packages = lock["packages"]
        assert isinstance(packages, dict)
        scripts_by_source: dict[tuple[str, str], dict[str, str]] = {}
        for (
            lock_path,
            _name,
            _version,
            hook,
            command,
            sources,
        ) in LIFECYCLE_SCRIPT_SOURCE_UNION:
            for source in sources:
                if source in {"tarball", "packument"}:
                    scripts_by_source.setdefault((lock_path, source), {})[hook] = (
                        command
                    )

        tarball_manifests: list[tuple[str, dict[str, object]]] = []
        packument_versions: dict[str, dict[str, dict[str, object]]] = {}
        for lock_path, raw_entry in sorted(
            packages.items(), key=lambda item: item[0].encode("utf-8")
        ):
            if lock_path == "":
                continue
            assert isinstance(raw_entry, dict)
            suffix = lock_path.removeprefix("node_modules/")
            parts = suffix.split("node_modules/")[-1].split("/")
            fallback_name = (
                "/".join(parts[:2]) if parts[0].startswith("@") else parts[0]
            )
            name = raw_entry.get("name", fallback_name)
            version = raw_entry["version"]
            assert isinstance(name, str)
            assert isinstance(version, str)
            tarball_manifests.append(
                (
                    lock_path,
                    {
                        "name": name,
                        "version": version,
                        "scripts": scripts_by_source.get((lock_path, "tarball"), {}),
                    },
                )
            )
            packument_versions.setdefault(name, {})[version] = {
                "name": name,
                "version": version,
                "scripts": scripts_by_source.get((lock_path, "packument"), {}),
            }
        packuments = tuple(
            (name, {"name": name, "versions": versions})
            for name, versions in sorted(
                packument_versions.items(), key=lambda item: item[0].encode("utf-8")
            )
        )
        return tuple(tarball_manifests), packuments

    @staticmethod
    def lifecycle_receipt_record(
        identity: tuple[str, str, str, str, str, tuple[str, ...]],
    ) -> dict[str, object]:
        lock_path, name, version, hook, command, sources = identity
        return {
            "lock_path": lock_path,
            "name": name,
            "version": version,
            "resolved": (f"https://registry.npmjs.org/{name}/-/package-{version}.tgz"),
            "integrity": "sha512-" + base64.b64encode(bytes(64)).decode("ascii"),
            "selected_optional": False,
            "lifecycle_scripts": [
                {
                    "name": hook,
                    "command": command,
                    "sources": list(sources),
                }
            ],
        }

    def exact_lifecycle_inventory(
        self,
    ) -> tuple[
        object,
        tuple[dict[str, object], ...],
        tuple[tuple[str, dict[str, object]], ...],
    ]:
        executor = importlib.import_module("python_candidate_h2")
        lock_path = REPOSITORY_ROOT / "bindings/python/frontend/package-lock.json"
        self.assertEqual(
            hashlib.sha256(lock_path.read_bytes()).hexdigest(),
            PLAYWRIGHT_CORE_LOCK_SHA256,
        )
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
        tarball_manifests, packuments = self.lifecycle_authority_inputs(lock)
        workspace = types.SimpleNamespace(
            frontend=REPOSITORY_ROOT / ".absent-lifecycle-oracle-node-modules"
        )
        _source, _configs, _pins, locked = executor._frontend_inputs(
            REPOSITORY_ROOT,
            workspace,
            tarball_manifests,
            packuments,
        )
        return executor, locked, packuments

    def test_lifecycle_inventory_is_the_full_lock_identity_and_source_union(
        self,
    ) -> None:
        executor, locked, packuments = self.exact_lifecycle_inventory()
        lock = json.loads(
            (REPOSITORY_ROOT / "bindings/python/frontend/package-lock.json").read_text(
                encoding="utf-8"
            )
        )
        expected_scripts: dict[str, list[dict[str, object]]] = {}
        for (
            lock_path,
            _name,
            _version,
            hook,
            command,
            sources,
        ) in LIFECYCLE_SCRIPT_SOURCE_UNION:
            expected_scripts.setdefault(lock_path, []).append(
                {"name": hook, "command": command, "sources": list(sources)}
            )
        expected_inventory = []
        for lock_path, raw_entry in sorted(
            lock["packages"].items(), key=lambda item: item[0].encode("utf-8")
        ):
            if lock_path == "":
                continue
            suffix = lock_path.removeprefix("node_modules/")
            parts = suffix.split("node_modules/")[-1].split("/")
            fallback_name = (
                "/".join(parts[:2]) if parts[0].startswith("@") else parts[0]
            )
            expected_inventory.append(
                {
                    "lock_path": lock_path,
                    "name": raw_entry.get("name", fallback_name),
                    "version": raw_entry["version"],
                    "lifecycle_scripts": expected_scripts.get(lock_path, []),
                }
            )
        observed_inventory = [
            {
                "lock_path": item["lock_path"],
                "name": item["name"],
                "version": item["version"],
                "lifecycle_scripts": item["lifecycle_scripts"],
            }
            for item in locked
        ]
        self.assertEqual(len(locked), 103)
        self.assertEqual(len(locked), len(lock["packages"]) - 1)
        self.assertEqual(observed_inventory, expected_inventory)
        self.assertEqual(
            tuple(
                (
                    item["lock_path"],
                    item["name"],
                    item["version"],
                    script["name"],
                    script["command"],
                    tuple(script["sources"]),
                )
                for item in observed_inventory
                for script in item["lifecycle_scripts"]
            ),
            LIFECYCLE_SCRIPT_SOURCE_UNION,
        )
        self.assertEqual(
            executor.structured_sha256(observed_inventory),
            INSTALL_SCRIPT_INVENTORY_SHA256,
        )
        self.assertEqual(
            candidate_manifest_module.INSTALL_SCRIPT_INVENTORY_SHA256,
            INSTALL_SCRIPT_INVENTORY_SHA256,
        )

        partial_packuments = tuple(item for item in packuments if item[0] != "fsevents")
        tarball_manifests, _packuments = self.lifecycle_authority_inputs(lock)
        with self.assertRaisesRegex(
            CandidateError,
            "fsevents|packument|registry metadata|source",
        ):
            executor._frontend_inputs(
                REPOSITORY_ROOT,
                types.SimpleNamespace(
                    frontend=(REPOSITORY_ROOT / ".absent-lifecycle-oracle-node-modules")
                ),
                tarball_manifests,
                partial_packuments,
            )

    def test_lifecycle_gate_admits_only_noninstall_and_two_exact_packument_hits(
        self,
    ) -> None:
        self.assertEqual(
            tuple(candidate_manifest_module.PACKUMENT_INSTALL_SCRIPT_ADMISSIONS),
            PACKUMENT_INSTALL_SCRIPT_ADMISSIONS,
        )
        ordinary = self.lifecycle_receipt_record(
            (
                "node_modules/ordinary",
                "ordinary",
                "1.0.0",
                "prepare",
                "npm run build",
                ("packument", "tarball"),
            )
        )
        self.assertEqual(
            candidate_manifest_module._locked_record(
                ordinary, "receipt.inputs.locked_packages"
            ),
            ordinary,
        )
        for identity in PACKUMENT_INSTALL_SCRIPT_ADMISSIONS:
            record_identity = (*identity[:5], ("lockfile", identity[5]))
            record = self.lifecycle_receipt_record(record_identity)
            self.assertEqual(
                candidate_manifest_module._locked_record(
                    record, "receipt.inputs.locked_packages"
                ),
                record,
            )

        identity = PACKUMENT_INSTALL_SCRIPT_ADMISSIONS[0]
        fields = ("lock_path", "name", "version", "hook", "command", "source")
        replacements: tuple[object, ...] = (
            "node_modules/other/fsevents",
            "other",
            "2.3.3",
            "postinstall",
            "node-gyp rebuild --changed",
            "tarball",
        )
        for field, replacement in zip(fields, replacements, strict=True):
            with self.subTest(drift=field):
                mutant = list(identity)
                mutant[fields.index(field)] = replacement
                sources = (
                    ("lockfile", str(mutant[5]))
                    if field != "source"
                    else ("lockfile", str(replacement))
                )
                record = self.lifecycle_receipt_record((*mutant[:5], sources))
                with self.assertRaises(candidate_manifest_module.ManifestError):
                    candidate_manifest_module._locked_record(
                        record, "receipt.inputs.locked_packages"
                    )

        for missing_sources in (("packument",), ("lockfile",)):
            with self.subTest(partial_sources=missing_sources):
                record = self.lifecycle_receipt_record((*identity[:5], missing_sources))
                with self.assertRaises(candidate_manifest_module.ManifestError):
                    candidate_manifest_module._locked_record(
                        record, "receipt.inputs.locked_packages"
                    )

    def test_tarball_install_hooks_are_absolute_rejections_with_identity(
        self,
    ) -> None:
        accepted = PACKUMENT_INSTALL_SCRIPT_ADMISSIONS[0]
        for hook in ("preinstall", "install", "postinstall"):
            with self.subTest(hook=hook):
                record = self.lifecycle_receipt_record(
                    (*accepted[:3], hook, accepted[4], ("tarball",))
                )
                with self.assertRaises(
                    candidate_manifest_module.ManifestError
                ) as raised:
                    candidate_manifest_module._locked_record(
                        record, "receipt.inputs.locked_packages"
                    )
                message = str(raised.exception)
                self.assertIn(str(record["lock_path"]), message)
                self.assertIn(hook, message)
                self.assertIn("tarball", message)

    def test_lifecycle_inventory_pin_rejects_drift_and_partial_receipts(
        self,
    ) -> None:
        _executor, locked, _packuments = self.exact_lifecycle_inventory()
        candidate_manifest_module._validate_install_script_inventory(locked)

        drifted = json.loads(json.dumps(locked))
        lightningcss = next(
            item for item in drifted if item["lock_path"] == "node_modules/lightningcss"
        )
        lightningcss["lifecycle_scripts"][0]["command"] = "patch-package --changed"
        with self.assertRaisesRegex(
            candidate_manifest_module.ManifestError, "inventory|identity|drift"
        ):
            candidate_manifest_module._validate_install_script_inventory(drifted)

        partial = json.loads(json.dumps(locked))
        partial.pop()
        with self.assertRaisesRegex(
            candidate_manifest_module.ManifestError, "inventory|identity|drift"
        ):
            candidate_manifest_module._validate_install_script_inventory(partial)

    def test_exact_content_bound_browser_and_abstract_resource_profile(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        self.assertEqual(
            dict(executor.CONTENT_BOUND_BROWSER_PROFILE),
            CONTENT_BOUND_BROWSER_PROFILE,
        )
        self.assertEqual(
            dict(executor.CONTENT_BOUND_RESOURCE_LIMITS),
            CONTENT_BOUND_RESOURCE_LIMITS,
        )

        observed = {
            "family_member_count": 5,
            "family_largest_member_bytes": 16_777_216,
            "family_bytes": 67_108_864,
            "source_member_count": 50_000,
            "source_largest_member_bytes": 67_108_864,
            "source_bytes": 536_870_912,
            "locked_package_count": 2_047,
            "locked_package_bytes": 1_073_741_824,
            "build_output_count": 3,
            "build_output_bytes": 16_777_216,
            "resolved_python_wheel_count": 256,
            "resolved_python_wheel_bytes": 1_073_741_824,
            "browser_archive_bytes": CONTENT_BOUND_BROWSER_PROFILE[
                "raw_archive_bytes"
            ],
            "browser_archive_sha256": CONTENT_BOUND_BROWSER_PROFILE[
                "raw_archive_sha256"
            ],
            "browser_archive_member_count": CONTENT_BOUND_BROWSER_PROFILE[
                "zip_member_count"
            ],
            "browser_extracted_regular_bytes": CONTENT_BOUND_BROWSER_PROFILE[
                "total_expanded_bytes"
            ],
            "browser_largest_expanded_member_bytes": (
                CONTENT_BOUND_BROWSER_PROFILE["largest_expanded_member_bytes"]
            ),
            "browser_largest_member": CONTENT_BOUND_BROWSER_PROFILE[
                "largest_member"
            ],
            "browser_member_inventory_sha256": CONTENT_BOUND_BROWSER_PROFILE[
                "closed_member_inventory_sha256"
            ],
            "browser_executable_sha256": CONTENT_BOUND_BROWSER_PROFILE[
                "executable_sha256"
            ],
            "host_scenarios": 2,
        }
        equality = executor.require_content_bound_resources(dict(observed))
        self.assertEqual(
            equality,
            {"member_steps": 104_650, "byte_steps": 4_789_240_546},
        )
        alternate_equality = dict(observed)
        alternate_equality["source_member_count"] = 49_999
        alternate_equality["locked_package_count"] = 2_048
        self.assertEqual(
            executor.require_content_bound_resources(alternate_equality),
            {"member_steps": 104_650, "byte_steps": 4_789_240_546},
        )
        aggregate_equality = dict(observed)
        aggregate_equality["locked_package_count"] = 2_048
        self.assertEqual(
            executor.require_content_bound_resources(aggregate_equality),
            {"member_steps": 104_652, "byte_steps": 4_789_240_546},
        )

        component_maxima = {
            "family_member_count": 5,
            "family_largest_member_bytes": 16_777_216,
            "family_bytes": 67_108_864,
            "source_member_count": 50_000,
            "source_largest_member_bytes": 67_108_864,
            "source_bytes": 536_870_912,
            "locked_package_count": 2_048,
            "locked_package_bytes": 1_073_741_824,
            "build_output_count": 3,
            "build_output_bytes": 16_777_216,
            "resolved_python_wheel_count": 256,
            "resolved_python_wheel_bytes": 1_073_741_824,
            "browser_archive_bytes": 120_231_126,
            "browser_archive_member_count": 287,
            "browser_extracted_regular_bytes": 273_378_828,
            "browser_largest_expanded_member_bytes": 196_975_952,
            "host_scenarios": 2,
        }
        for name, maximum in component_maxima.items():
            with self.subTest(first_excess=name):
                mutant = dict(observed)
                mutant[name] = maximum + 1
                with self.assertRaises((CandidateError, RuntimeError)):
                    executor.require_content_bound_resources(mutant)

        for name in (
            "browser_archive_sha256",
            "browser_member_inventory_sha256",
            "browser_executable_sha256",
        ):
            with self.subTest(identity=name):
                mutant = dict(observed)
                mutant[name] = "0" * 64
                with self.assertRaises((CandidateError, RuntimeError)):
                    executor.require_content_bound_resources(mutant)

        for name in (
            "family_member_count",
            "build_output_count",
            "browser_archive_bytes",
            "browser_archive_member_count",
            "browser_extracted_regular_bytes",
            "browser_largest_expanded_member_bytes",
            "host_scenarios",
        ):
            with self.subTest(exact_identity=name):
                mutant = dict(observed)
                mutant[name] = int(mutant[name]) - 1
                with self.assertRaises((CandidateError, RuntimeError)):
                    executor.require_content_bound_resources(mutant)

    @unittest.skipIf(
        os.environ.get("EQIORA_CI_CONTRACT_ONLY") == "1",
        "the change-ownership lane runs contract tests, not the registered real H2 aggregate",
    )
    def test_ordinary_default_candidate_then_live_bound_falsifiers(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        transport = importlib.import_module(
            "tools.ci.tests.test_release_transport"
        )
        source_status = subprocess.check_output(
            ["git", "status", "--porcelain=v1", "--untracked-files=all"],
            cwd=REPOSITORY_ROOT,
            text=True,
        )
        self.assertEqual(
            source_status,
            "",
            f"real H2 aggregate requires a clean source tree:\n{source_status}",
        )
        expected_commit = subprocess.check_output(
            ["git", "rev-parse", "HEAD"],
            cwd=REPOSITORY_ROOT,
            text=True,
        ).strip()
        self.assertRegex(expected_commit, r"\A[0-9a-f]{40}\Z")
        launched: list[tuple[str, ...]] = []
        real_popen = subprocess.Popen

        def observe_launch(
            argv: object,
            *args: object,
            **kwargs: object,
        ) -> subprocess.Popen[str]:
            if isinstance(argv, (str, bytes)):
                vector = (os.fsdecode(argv),)
            else:
                vector = tuple(  # type: ignore[union-attr]
                    os.fsdecode(os.fspath(value)) for value in argv
                )
            launched.append(vector)
            return real_popen(argv, *args, **kwargs)  # type: ignore[arg-type]

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family_path = root / "family"
            h2_output = root / "h2"
            metadata = root / "metadata"
            with mock.patch.object(
                subprocess,
                "Popen",
                side_effect=observe_launch,
            ):
                prepared = python_candidate_module.prepare_candidate(
                    expected_commit=expected_commit,
                    out=family_path,
                    require_tag=False,
                )
                receipt_path = executor.execute_h2(
                    expected_commit=expected_commit,
                    artifacts=prepared,
                    out=h2_output,
                )
                manifest = python_candidate_module.finalize_candidate(
                    expected_commit=expected_commit,
                    artifacts=prepared,
                    h2_receipt=receipt_path,
                    manifest_out=metadata,
                )

            family = executor.admit_candidate_family(prepared)
            candidate_version = python_candidate_module.load_config().python_version
            expected_names = (
                f"eqiora-{candidate_version}.tar.gz",
                *(
                    exact_wheel_name(compact, version=candidate_version)
                    for compact in EXACT_WHEEL_INTERPRETERS
                ),
            )
            self.assertEqual(len(family.inventory), 5)
            self.assertEqual(
                tuple(record["filename"] for record in family.inventory),
                tuple(sorted(expected_names, key=lambda name: name.encode("utf-8"))),
            )
            self.assertTrue(all(int(record["size"]) > 0 for record in family.inventory))
            family_after = executor.family_inventory(prepared)
            self.assertEqual(family_after, family.inventory)

            receipt_bytes = receipt_path.read_bytes()
            receipt = json.loads(receipt_bytes)
            self.assertEqual(receipt_bytes, executor.canonical_json_bytes(receipt))
            executor.validate_h2_receipt(receipt)
            self.assertEqual(tuple(h2_output.iterdir()), (receipt_path,))
            retained_receipt = metadata / receipt_path.name
            self.assertEqual(retained_receipt.read_bytes(), receipt_bytes)
            self.assertEqual(
                {path.name for path in metadata.iterdir()},
                {manifest.name, retained_receipt.name},
            )
            document = json.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(
                set(document["checks"]).intersection(NOTEBOOK_PROFILE_CHECKS),
                set(NOTEBOOK_PROFILE_CHECKS),
            )
            accepted = transport.load_candidate_family(
                manifest,
                prepared,
                requested_profiles=("notebook",),
                h2_receipt=retained_receipt,
            )
            transport.verify_artifacts(accepted, prepared)
            self.assertEqual(executor.family_inventory(prepared), family.inventory)

            exact_commands = (
                ("npm", "ci", "--ignore-scripts"),
                ("npm", "run", "typecheck"),
                ("npm", "run", "lint"),
                ("npm", "run", "test"),
                ("npm", "run", "build"),
            )
            for exact_command in exact_commands:
                self.assertGreaterEqual(
                    sum(
                        any(
                            vector[index : index + len(exact_command)]
                            == exact_command
                            for index in range(
                                len(vector) - len(exact_command) + 1
                            )
                        )
                        for vector in launched
                    ),
                    2,
                )
            for exact_host in (
                ("-I", "-m", "marimo", "run"),
                ("npm", "run", "test:hosts", "--", "--project=marimo-0.23.16"),
            ):
                self.assertTrue(
                    any(
                        any(
                            vector[index : index + len(exact_host)] == exact_host
                            for index in range(len(vector) - len(exact_host) + 1)
                        )
                        for vector in launched
                    ),
                    exact_host,
                )

            receipt_mutations = {
                "visible-nonloopback-request": lambda value: value[
                    "clean_run_1"
                ].__setitem__("external_request_count_after_npm_ci", 1),
                "cdn-import": lambda value: value["clean_run_1"].__setitem__(
                    "emitted_imports", ["https://cdn.invalid/eqiora-h2.js"]
                ),
            }
            for name, mutate in receipt_mutations.items():
                with self.subTest(live_bound_falsifier=name):
                    mutant_root = root / name
                    mutant_root.mkdir()
                    mutant_receipt = json.loads(receipt_bytes)
                    mutate(mutant_receipt)
                    mutant_receipt_path = mutant_root / receipt_path.name
                    mutant_receipt_bytes = executor.canonical_json_bytes(
                        mutant_receipt
                    )
                    mutant_receipt_path.write_bytes(mutant_receipt_bytes)
                    mutant_document = json.loads(
                        manifest.read_text(encoding="utf-8")
                    )
                    mutant_document["build"]["frontend"][
                        "h2_receipt_sha256"
                    ] = hashlib.sha256(mutant_receipt_bytes).hexdigest()
                    mutant_manifest = mutant_root / manifest.name
                    mutant_manifest.write_text(
                        json.dumps(mutant_document), encoding="utf-8"
                    )
                    with self.assertRaises(transport.ManifestError):
                        transport.load_candidate_family(
                            mutant_manifest,
                            prepared,
                            requested_profiles=("notebook",),
                            h2_receipt=mutant_receipt_path,
                        )

            no_op_output = root / "no-op-h2"
            with mock.patch.object(
                executor,
                "_run_process",
                return_value=("", 0),
            ) as no_op_launch:
                with self.assertRaises((CandidateError, RuntimeError)):
                    executor.execute_h2(
                        expected_commit=expected_commit,
                        artifacts=prepared,
                        out=no_op_output,
                    )
            self.assertGreater(no_op_launch.call_count, 0)
            if no_op_output.exists():
                self.assertEqual(tuple(no_op_output.iterdir()), ())

            for omitted in NOTEBOOK_PROFILE_CHECKS[5:7]:
                with self.subTest(omitted_host=omitted):
                    omitted_output = root / f"omitted-{omitted.split(':')[1]}"
                    forged = python_candidate_module.CandidateProfileSummary(
                        config=python_candidate_module.load_config(),
                        uv="/reviewed/uv",
                        wheel_records=(),
                        checks=(
                            "twine-strict",
                            "sdist-to-wheel-rebuild",
                            *(
                                name
                                for name in NOTEBOOK_PROFILE_CHECKS
                                if name != omitted
                            ),
                        ),
                        dependency_profiles={},
                    )
                    with (
                        mock.patch.object(
                            python_candidate_module,
                            "run_candidate_profiles",
                            return_value=forged,
                        ) as profiles,
                        mock.patch.object(
                            python_candidate_module,
                            "write_manifest",
                            wraps=python_candidate_module.write_manifest,
                        ) as write_manifest,
                    ):
                        with self.assertRaises(CandidateError):
                            python_candidate_module.finalize_candidate(
                                expected_commit=expected_commit,
                                artifacts=prepared,
                                h2_receipt=receipt_path,
                                manifest_out=omitted_output,
                            )
                    profiles.assert_called_once()
                    write_manifest.assert_not_called()
                    if omitted_output.exists():
                        self.assertEqual(tuple(omitted_output.iterdir()), ())

            bypass_output = root / "finalizer-bypass"
            with (
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    return_value=mock.sentinel.false_success,
                ) as profiles,
                mock.patch.object(
                    python_candidate_module,
                    "write_manifest",
                    wraps=python_candidate_module.write_manifest,
                ) as write_manifest,
            ):
                with self.assertRaises(CandidateError):
                    python_candidate_module.finalize_candidate(
                        expected_commit=expected_commit,
                        artifacts=prepared,
                        h2_receipt=receipt_path,
                        manifest_out=bypass_output,
                    )
            profiles.assert_called_once()
            write_manifest.assert_not_called()
            if bypass_output.exists():
                self.assertEqual(tuple(bypass_output.iterdir()), ())

    def test_cross_family_receipt_substitution_rejects_two_valid_families(
        self,
    ) -> None:
        transport = importlib.import_module(
            "tools.ci.tests.test_release_transport"
        )
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            (root / "candidate-a").mkdir()
            manifest_a, artifacts_a, _, receipt_a, _ = (
                transport.complete_v3_candidate_document(root / "candidate-a")
            )
            accepted_a = transport.load_candidate_family(
                manifest_a,
                artifacts_a,
                requested_profiles=("notebook",),
                h2_receipt=receipt_a,
            )
            transport.verify_artifacts(accepted_a, artifacts_a)

            (root / "candidate-b").mkdir()
            manifest_b, artifacts_b, document_b, receipt_b, receipt_document_b = (
                transport.complete_v3_candidate_document(root / "candidate-b")
            )
            wheel_b = sorted(artifacts_b.glob("*.whl"))[0]
            with zipfile.ZipFile(wheel_b, mode="a") as archive:
                archive.comment = b"independently valid candidate B"
            transport._bind_receipt(
                manifest_b,
                artifacts_b,
                document_b,
                receipt_b,
                receipt_document_b,
            )
            accepted_b = transport.load_candidate_family(
                manifest_b,
                artifacts_b,
                requested_profiles=("notebook",),
                h2_receipt=receipt_b,
            )
            transport.verify_artifacts(accepted_b, artifacts_b)

            receipt_b.write_bytes(receipt_a.read_bytes())
            document_b["build"]["frontend"]["h2_receipt_sha256"] = hashlib.sha256(
                receipt_b.read_bytes()
            ).hexdigest()
            manifest_b.write_text(json.dumps(document_b), encoding="utf-8")
            with self.assertRaises(transport.ManifestError):
                transport.load_candidate_family(
                    manifest_b,
                    artifacts_b,
                    requested_profiles=("notebook",),
                    h2_receipt=receipt_b,
                )

    def test_locked_playwright_core_compatibility_and_connected_acquisition(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        node_location = shutil.which("node")
        if node_location is None:
            self.fail("the locked Playwright compatibility oracle requires Node")
        node = Path(node_location).resolve()

        class ExactBrowserAcquisitionReached(Exception):
            pass

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = self.playwright_workspace(root)
            lock, package, package_archive = self.install_locked_playwright_core(
                workspace
            )
            browser_cache = Path(workspace.browser_cache).resolve()
            expected = self.expected_playwright_observation(browser_cache)
            observed = self.playwright_probe(
                node,
                package,
                browser_cache,
                "const p=require('path'),r=process.argv[1];"
                "const e=require(p.join(r,'lib/coreBundle.js')).registry.registry"
                ".findExecutable('chromium-headless-shell');"
                "process.stdout.write(JSON.stringify({name:e.name,"
                "browserName:e.browserName,revision:e.revision,"
                "browserVersion:e.browserVersion,installType:e.installType,"
                "directory:e.directory,executablePath:e.executablePath(),"
                "downloadURLs:e.downloadURLs}));",
            )
            self.assertEqual(observed.returncode, 0, observed.stderr)
            self.assertEqual(json.loads(observed.stdout), expected)
            self.assertEqual(tuple(browser_cache.iterdir()), ())

            expected_archive = (
                Path(workspace.root) / "chromium-headless-shell-1234.zip"
            )
            exact_downloads: list[Path] = []

            def stop_at_exact_browser(destination: Path) -> None:
                exact_downloads.append(destination)
                raise ExactBrowserAcquisitionReached

            real_run = subprocess.run
            registry_invocations: list[tuple[str, ...]] = []

            def run(argv: list[str], **kwargs: object) -> object:
                if tuple(str(value) for value in argv[:2]) != (str(node), "-e"):
                    return real_run(argv, **kwargs)
                completed, calls = self.instrumented_playwright_process(
                    real_run, argv, dict(kwargs)
                )
                registry_invocations.append(calls)
                return completed

            with (
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=stop_at_exact_browser,
                    create=True,
                ) as exact_download,
                mock.patch.object(executor, "_download") as generic_download,
                mock.patch.object(executor.subprocess, "run", side_effect=run),
                mock.patch.object(executor, "_safe_extract_zip") as extract,
            ):
                with self.assertRaises(ExactBrowserAcquisitionReached):
                    executor._acquire_browser(workspace, node, lock, package)

            self.assertTrue(package_archive.is_file())
            self.assertEqual(
                registry_invocations,
                [("chromium-headless-shell",)],
            )
            self.assertEqual(exact_downloads, [expected_archive])
            exact_download.assert_called_once_with(expected_archive)
            generic_download.assert_not_called()
            extract.assert_not_called()
            self.assertFalse(expected_archive.exists())
            self.assertEqual(tuple(browser_cache.iterdir()), ())

    def _obsolete_one_member_playwright_acquisition_unit(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        node_location = shutil.which("node")
        if node_location is None:
            self.fail("the locked Playwright compatibility oracle requires Node")
        node = Path(node_location).resolve()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = self.playwright_workspace(root)
            lock, package, package_archive = self.install_locked_playwright_core(
                workspace
            )
            browser_cache = Path(workspace.browser_cache).resolve()
            expected = self.expected_playwright_observation(browser_cache)

            reference_program = (
                "const p=require('path'),r=process.argv[1];"
                "const e=require(p.join(r,'lib/coreBundle.js')).registry.registry"
                ".findExecutable('chromium-headless-shell');"
                "process.stdout.write(JSON.stringify({name:e.name,"
                "browserName:e.browserName,revision:e.revision,"
                "browserVersion:e.browserVersion,installType:e.installType,"
                "directory:e.directory,executablePath:e.executablePath(),"
                "downloadURLs:e.downloadURLs}));"
            )
            observed = self.playwright_probe(
                node, package, browser_cache, reference_program
            )
            self.assertEqual(observed.returncode, 0, observed.stderr)
            self.assertEqual(json.loads(observed.stdout), expected)
            self.assertEqual(tuple(browser_cache.iterdir()), ())

            old_module = self.playwright_probe(
                node,
                package,
                browser_cache,
                "const p=require('path'),r=process.argv[1];"
                "require(p.join(r,'lib/server/registry/index.js'));",
            )
            self.assertNotEqual(old_module.returncode, 0)
            self.assertIn("lib/server/registry/index.js", old_module.stderr)

            missing_nested_registry = self.playwright_probe(
                node,
                package,
                browser_cache,
                "const p=require('path'),r=process.argv[1];"
                "require(p.join(r,'lib/coreBundle.js')).registry"
                ".findExecutable('chromium-headless-shell');",
            )
            self.assertNotEqual(missing_nested_registry.returncode, 0)
            self.assertRegex(missing_nested_registry.stderr, "findExecutable")

            old_property = self.playwright_probe(
                node,
                package,
                browser_cache,
                "const p=require('path'),r=process.argv[1];"
                "const e=require(p.join(r,'lib/coreBundle.js')).registry.registry"
                ".findExecutable('chromium-headless-shell');"
                "process.stdout.write(JSON.stringify({"
                "own:Object.prototype.hasOwnProperty.call(e,'_downloadURLs'),"
                "value:e._downloadURLs??null}));",
            )
            self.assertEqual(old_property.returncode, 0, old_property.stderr)
            self.assertEqual(
                json.loads(old_property.stdout), {"own": False, "value": None}
            )

            expected_directory = Path(str(expected["directory"]))
            expected_executable = Path(str(expected["executablePath"]))
            downloads: list[tuple[str, Path]] = []

            def download(url: str, destination: Path) -> None:
                downloads.append((url, destination))
                self.assertEqual(url, PLAYWRIGHT_BROWSER_URL)
                self.write_browser_archive(destination)

            real_run = subprocess.run
            processes: list[tuple[tuple[str, ...], bool, dict[str, object], str]] = []
            registry_invocations: list[tuple[str, ...]] = []

            def run(argv: list[str], **kwargs: object) -> object:
                command = tuple(str(value) for value in argv)
                present = expected_executable.exists()
                if command[:2] == (str(node), "-e"):
                    completed, calls = self.instrumented_playwright_process(
                        real_run, argv, dict(kwargs)
                    )
                    registry_invocations.append(calls)
                else:
                    completed = real_run(argv, **kwargs)
                processes.append(
                    (command, present, dict(kwargs), str(completed.stdout))
                )
                return completed

            real_extract = executor._safe_extract_zip
            with (
                mock.patch.object(executor, "_download", side_effect=download),
                mock.patch.object(
                    executor, "_safe_extract_zip", wraps=real_extract
                ) as extract,
                mock.patch.object(executor.subprocess, "run", side_effect=run),
            ):
                acquisition = executor._acquire_browser(
                    workspace,
                    node,
                    lock,
                    package,
                )

            expected_archive = Path(workspace.root) / "chromium-headless-shell-1234.zip"
            test_integrity = lock["packages"]["node_modules/@playwright/test"][
                "integrity"
            ]
            self.assertEqual(
                acquisition,
                (
                    expected_archive,
                    expected_executable,
                    "linux-x86_64",
                    test_integrity,
                    PLAYWRIGHT_CORE_INTEGRITY,
                ),
            )
            self.assertEqual(downloads, [(PLAYWRIGHT_BROWSER_URL, expected_archive)])
            extract.assert_called_once_with(expected_archive, expected_directory)
            self.assertTrue(package_archive.is_file())
            self.assertTrue(expected_archive.is_file())
            self.assertTrue(expected_executable.is_file())
            self.assertFalse(expected_executable.is_symlink())
            self.assertFalse((browser_cache / "chromium-headless-shell-1234").exists())
            node_processes = [
                (command, present, kwargs, stdout)
                for command, present, kwargs, stdout in processes
                if command[:2] == (str(node), "-e")
            ]
            self.assertEqual(len(node_processes), 2)
            self.assertEqual(
                [present for _command, present, _kwargs, _stdout in node_processes],
                [False, True],
            )
            self.assertEqual(
                registry_invocations,
                [
                    ("chromium-headless-shell",),
                    ("chromium-headless-shell",),
                ],
            )
            for index, (command, _present, kwargs, stdout) in enumerate(node_processes):
                self.assertIn("lib/coreBundle.js", command[2])
                self.assertNotIn("lib/server/registry/index.js", command[2])
                self.assertIn(".registry.registry", command[2])
                self.assertNotIn("_downloadURLs", command[2])
                self.assertEqual(
                    kwargs["env"]["PLAYWRIGHT_BROWSERS_PATH"],  # type: ignore[index]
                    str(browser_cache),
                )
                if index == 0:
                    self.assertEqual(json.loads(stdout), expected)
                    self.assertIn("downloadURLs", command[2])
                else:
                    self.assertIn(str(expected_executable), stdout)
            self.assertIn(
                (str(expected_executable), "--version"),
                [command for command, _present, _kwargs, _stdout in processes],
            )

            shutil.copyfile(
                REPOSITORY_ROOT / "bindings/python/frontend/package-lock.json",
                Path(workspace.frontend) / "package-lock.json",
            )
            manifests = (("node_modules/playwright-core", {"version": "1.62.1"}),)
            with (
                mock.patch.object(
                    executor,
                    "_node_and_npm_identity",
                    return_value=(
                        node,
                        Path(workspace.root) / "npm-11.16.0.tgz",
                        Path(workspace.root) / "npm",
                    ),
                ),
                mock.patch.object(
                    executor,
                    "_prefetch_lock_packages",
                    return_value=(manifests, package),
                ) as prefetch,
                mock.patch.object(
                    executor,
                    "_acquire_python_wheels",
                    return_value=((), "d" * 64),
                ),
                mock.patch.object(
                    executor, "_acquire_browser", return_value=acquisition
                ) as acquire_browser,
            ):
                connected = executor.acquire_inputs(workspace)
            prefetch.assert_called_once()
            acquire_browser.assert_called_once_with(
                workspace,
                node,
                mock.ANY,
                package,
            )
            self.assertEqual(connected.browser_archive, expected_archive)
            self.assertEqual(connected.browser_executable, expected_executable)
            self.assertEqual(
                connected.browser_archive_sha256,
                hashlib.sha256(expected_archive.read_bytes()).hexdigest(),
            )
            self.assertEqual(
                connected.browser_executable_sha256,
                hashlib.sha256(expected_executable.read_bytes()).hexdigest(),
            )
            self.assertEqual(connected.package_manifests, manifests)
            self.assertEqual(
                expected_executable.parents[1],
                Path(workspace.browser_cache) / "chromium_headless_shell-1234",
            )

    def test_implementation_uses_both_exact_find_executable_results(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        node_location = shutil.which("node")
        if node_location is None:
            self.fail("the locked Playwright compatibility oracle requires Node")
        node = Path(node_location).resolve()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = self.playwright_workspace(root)
            lock, package, _package_archive = self.install_locked_playwright_core(
                workspace
            )
            browser_cache = Path(workspace.browser_cache).resolve()
            expected = self.expected_playwright_observation(browser_cache)
            real_run = subprocess.run
            mutations = (
                ("name", "chromium"),
                ("browserName", "firefox"),
                ("revision", "1235"),
                ("browserVersion", "151.0.7922.35"),
                ("installType", "install-by-default"),
                ("directory", str(root / "hard-coded-directory")),
                ("executablePath", str(root / "hard-coded-executable")),
                ("downloadURLs", [PLAYWRIGHT_BROWSER_URL + "?drift=1"]),
            )

            for field, mutant in mutations:
                with self.subTest(field=field):
                    calls: list[tuple[str, ...]] = []

                    def run(argv: list[str], **kwargs: object) -> object:
                        completed, invocation = self.instrumented_playwright_process(
                            real_run, argv, dict(kwargs), (field, mutant)
                        )
                        calls.append(invocation)
                        return completed

                    with (
                        mock.patch.object(executor.subprocess, "run", side_effect=run),
                        mock.patch.object(
                            executor,
                            "_download",
                            side_effect=AssertionError(
                                "hard-coded probe output reached browser download"
                            ),
                        ) as download,
                        mock.patch.object(
                            executor,
                            "_download_exact_browser",
                            side_effect=AssertionError(
                                "hard-coded probe output reached exact browser download"
                            ),
                            create=True,
                        ) as exact_download,
                    ):
                        with self.assertRaises(CandidateError):
                            executor._acquire_browser(workspace, node, lock, package)
                    self.assertEqual(calls, [("chromium-headless-shell",)])
                    download.assert_not_called()
                    exact_download.assert_not_called()
                    self.assertEqual(tuple(browser_cache.iterdir()), ())
                    self.assertNotEqual(mutant, expected[field])

            calls = []
            expected_archive = (
                Path(workspace.root) / "chromium-headless-shell-1234.zip"
            )
            expected_executable = Path(str(expected["executablePath"]))
            identity_paths: list[Path] = []
            real_file_sha256 = executor.file_sha256

            def download_exact(destination: Path) -> None:
                self.write_browser_archive(destination)

            def frozen_structural_identity(path: Path) -> str:
                observed = Path(path)
                if observed == expected_archive:
                    identity_paths.append(observed)
                    return str(
                        CONTENT_BOUND_BROWSER_PROFILE["raw_archive_sha256"]
                    )
                if observed == expected_executable:
                    identity_paths.append(observed)
                    return str(
                        CONTENT_BOUND_BROWSER_PROFILE["executable_sha256"]
                    )
                return real_file_sha256(observed)

            def run(argv: list[str], **kwargs: object) -> object:
                command = tuple(str(value) for value in argv)
                if command[:2] != (str(node), "-e"):
                    return real_run(argv, **kwargs)
                mutation = (
                    None
                    if not calls
                    else ("executablePath", str(root / "hard-coded-executable"))
                )
                completed, invocation = self.instrumented_playwright_process(
                    real_run, argv, dict(kwargs), mutation
                )
                calls.append(invocation)
                return completed

            with (
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=download_exact,
                    create=True,
                ) as exact_download,
                mock.patch.object(
                    executor,
                    "_browser_archive_inventory",
                    side_effect=self.structural_browser_inventory,
                    create=True,
                ) as inventory,
                mock.patch.object(
                    executor,
                    "file_sha256",
                    side_effect=frozen_structural_identity,
                    create=True,
                ) as identity,
                mock.patch.object(executor, "_download") as generic_download,
                mock.patch.object(executor.subprocess, "run", side_effect=run),
            ):
                with self.assertRaises(CandidateError):
                    executor._acquire_browser(workspace, node, lock, package)
            exact_download.assert_called_once_with(expected_archive)
            self.assertGreaterEqual(inventory.call_count, 1)
            identity.assert_has_calls(
                [mock.call(expected_archive), mock.call(expected_executable)],
                any_order=True,
            )
            self.assertEqual(
                set(identity_paths), {expected_archive, expected_executable}
            )
            generic_download.assert_not_called()
            self.assertEqual(
                calls,
                [
                    ("chromium-headless-shell",),
                    ("chromium-headless-shell",),
                ],
            )
            self.assertTrue(expected_executable.is_file())

    def test_playwright_probe_rejects_closed_observation_and_package_drift(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        node_location = shutil.which("node")
        if node_location is None:
            self.fail("the locked Playwright compatibility oracle requires Node")
        node = Path(node_location).resolve()
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = self.playwright_workspace(root)
            lock, package, _package_archive = self.install_locked_playwright_core(
                workspace
            )
            browser_cache = Path(workspace.browser_cache).resolve()
            expected = self.expected_playwright_observation(browser_cache)
            expected_directory = Path(str(expected["directory"]))

            invalid_observations: list[tuple[str, dict[str, object]]] = []

            def observation(name: str, key: str, value: object) -> None:
                mutated = dict(expected)
                mutated[key] = value
                invalid_observations.append((name, mutated))

            missing = dict(expected)
            missing.pop("browserName")
            invalid_observations.append(("missing-key", missing))
            invalid_observations.append(("extra-key", {**expected, "extra": True}))
            observation("wrong-executable-name", "name", "chromium")
            observation("wrong-browser-family", "browserName", "firefox")
            observation("wrong-revision", "revision", "1235")
            observation("wrong-browser-version", "browserVersion", "151.0.7922.35")
            observation("wrong-install-type", "installType", "install-by-default")
            observation("directory-not-string", "directory", 1)
            observation(
                "relative-directory", "directory", "chromium_headless_shell-1234"
            )
            observation(
                "parent-escaping-directory",
                "directory",
                str(browser_cache / ".." / "chromium_headless_shell-1234"),
            )
            observation(
                "hyphen-directory",
                "directory",
                str(browser_cache / "chromium-headless-shell-1234"),
            )
            alternate = root / "ambient-cache" / "chromium_headless_shell-1234"
            observation("ambient-directory", "directory", str(alternate))
            observation(
                "relative-executable", "executablePath", PLAYWRIGHT_BROWSER_MEMBER
            )
            observation(
                "recursive-basename",
                "executablePath",
                str(expected_directory / "other" / "chrome-headless-shell"),
            )
            observation(
                "parent-escaping-executable",
                "executablePath",
                str(expected_directory / ".." / PLAYWRIGHT_BROWSER_MEMBER),
            )
            observation("missing-url-value", "downloadURLs", None)
            observation("non-array-url-value", "downloadURLs", PLAYWRIGHT_BROWSER_URL)
            observation("empty-url-vector", "downloadURLs", [])
            observation(
                "extra-url",
                "downloadURLs",
                [PLAYWRIGHT_BROWSER_URL, "https://example.invalid/extra.zip"],
            )
            observation(
                "reordered-urls",
                "downloadURLs",
                ["https://example.invalid/extra.zip", PLAYWRIGHT_BROWSER_URL],
            )
            observation(
                "non-https-url",
                "downloadURLs",
                [PLAYWRIGHT_BROWSER_URL.replace("https://", "http://")],
            )
            observation(
                "url-byte-drift",
                "downloadURLs",
                [PLAYWRIGHT_BROWSER_URL + "?mirror=1"],
            )

            linked_root = root / "linked-browser-cache"
            linked_root.symlink_to(browser_cache, target_is_directory=True)
            observation(
                "symlinked-directory",
                "directory",
                str(linked_root / "chromium_headless_shell-1234"),
            )
            observation(
                "symlinked-executable",
                "executablePath",
                str(
                    linked_root
                    / "chromium_headless_shell-1234"
                    / PLAYWRIGHT_BROWSER_MEMBER
                ),
            )

            for name, mutated in invalid_observations:
                with self.subTest(observation=name):
                    completed = subprocess.CompletedProcess(
                        [str(node), "-e", "probe", str(package)],
                        0,
                        stdout=json.dumps(mutated),
                        stderr="",
                    )
                    with (
                        mock.patch.object(
                            executor.subprocess, "run", return_value=completed
                        ),
                        mock.patch.object(executor, "_download") as download,
                        mock.patch.object(
                            executor,
                            "_download_exact_browser",
                            side_effect=AssertionError(
                                "invalid observation reached exact browser download"
                            ),
                            create=True,
                        ) as exact_download,
                    ):
                        with self.assertRaises(CandidateError):
                            executor._acquire_browser(workspace, node, lock, package)
                    download.assert_not_called()
                    exact_download.assert_not_called()
                    self.assertFalse(expected_directory.exists())

            package_json = package / "package.json"
            browsers_json = package / "browsers.json"
            core_bundle = package / "lib/coreBundle.js"
            file_drifts = (
                (
                    "package-json",
                    package_json,
                    package_json.read_bytes().replace(b'"1.62.1"', b'"1.62.2"'),
                ),
                (
                    "browsers-json",
                    browsers_json,
                    browsers_json.read_bytes() + b"\n",
                ),
                ("core-bundle", core_bundle, core_bundle.read_bytes() + b"\n"),
            )
            for name, path, mutant in file_drifts:
                with self.subTest(package_file=name):
                    original = path.read_bytes()
                    path.write_bytes(mutant)
                    try:
                        with (
                            mock.patch.object(
                                executor.subprocess,
                                "run",
                                side_effect=AssertionError(
                                    "a drifted package reached the registry probe"
                                ),
                            ) as run,
                            mock.patch.object(executor, "_download") as download,
                            mock.patch.object(
                                executor,
                                "_download_exact_browser",
                                side_effect=AssertionError(
                                    "drifted package reached exact browser download"
                                ),
                                create=True,
                            ) as exact_download,
                        ):
                            with self.assertRaises(CandidateError):
                                executor._acquire_browser(
                                    workspace, node, lock, package
                                )
                        run.assert_not_called()
                        download.assert_not_called()
                        exact_download.assert_not_called()
                    finally:
                        path.write_bytes(original)

            lock_drifts = (
                ("core-version", "node_modules/playwright-core", "version", "1.62.2"),
                (
                    "core-integrity",
                    "node_modules/playwright-core",
                    "integrity",
                    "sha512-" + "A" * 88,
                ),
                ("test-version", "node_modules/@playwright/test", "version", "1.62.2"),
            )
            for name, lock_path, key, value in lock_drifts:
                with self.subTest(lock=name):
                    mutated_lock = json.loads(json.dumps(lock))
                    mutated_lock["packages"][lock_path][key] = value
                    with (
                        mock.patch.object(
                            executor.subprocess,
                            "run",
                            side_effect=AssertionError(
                                "a drifted lock reached the registry probe"
                            ),
                        ) as run,
                        mock.patch.object(executor, "_download") as download,
                        mock.patch.object(
                            executor,
                            "_download_exact_browser",
                            side_effect=AssertionError(
                                "drifted lock reached exact browser download"
                            ),
                            create=True,
                        ) as exact_download,
                    ):
                        with self.assertRaises(CandidateError):
                            executor._acquire_browser(
                                workspace, node, mutated_lock, package
                            )
                    run.assert_not_called()
                    download.assert_not_called()
                    exact_download.assert_not_called()

            ambient_packages = (
                (
                    "checkout-node-modules",
                    Path(workspace.frontend) / "node_modules/playwright-core",
                ),
                (
                    "workspace-home-cache",
                    Path(workspace.home) / ".cache/playwright/packages/playwright-core",
                ),
                ("parent-installation", root / "ambient/playwright-core"),
            )
            for name, ambient_package in ambient_packages:
                with self.subTest(package_root=name):
                    ambient_package.parent.mkdir(parents=True, exist_ok=True)
                    package.rename(ambient_package)
                    try:
                        with (
                            mock.patch.object(
                                executor.subprocess,
                                "run",
                                side_effect=AssertionError(
                                    "an ambient package reached the registry probe"
                                ),
                            ) as run,
                            mock.patch.object(executor, "_download") as download,
                            mock.patch.object(
                                executor,
                                "_download_exact_browser",
                                side_effect=AssertionError(
                                    "ambient package reached exact browser download"
                                ),
                                create=True,
                            ) as exact_download,
                        ):
                            with self.assertRaises(CandidateError):
                                executor._acquire_browser(
                                    workspace, node, lock, ambient_package
                                )
                        run.assert_not_called()
                        download.assert_not_called()
                        exact_download.assert_not_called()
                    finally:
                        ambient_package.rename(package)

            real_package = package.parent / "verified-package"
            package.rename(real_package)
            package.symlink_to(real_package, target_is_directory=True)
            try:
                with (
                    mock.patch.object(executor.subprocess, "run") as run,
                    mock.patch.object(executor, "_download") as download,
                    mock.patch.object(
                        executor,
                        "_download_exact_browser",
                        side_effect=AssertionError(
                            "symlinked package reached exact browser download"
                        ),
                        create=True,
                    ) as exact_download,
                ):
                    with self.assertRaises(CandidateError):
                        executor._acquire_browser(workspace, node, lock, package)
                run.assert_not_called()
                download.assert_not_called()
                exact_download.assert_not_called()
            finally:
                package.unlink()
                real_package.rename(package)

    def test_playwright_core_is_never_extracted_before_lock_sri_verification(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            workspace = self.playwright_workspace(Path(temporary))
            lock = {
                "packages": {
                    "node_modules/playwright-core": {
                        "version": "1.62.1",
                        "resolved": PLAYWRIGHT_CORE_URL,
                        "integrity": PLAYWRIGHT_CORE_INTEGRITY,
                    }
                }
            }

            def download(url: str, destination: Path) -> None:
                if url == "https://registry.npmjs.org/playwright-core":
                    destination.write_text(
                        json.dumps(
                            {
                                "name": "playwright-core",
                                "versions": {
                                    "1.62.1": {
                                        "name": "playwright-core",
                                        "version": "1.62.1",
                                    }
                                },
                            }
                        ),
                        encoding="utf-8",
                    )
                elif url == PLAYWRIGHT_CORE_URL:
                    destination.write_bytes(b"unverified package bytes")
                else:
                    raise AssertionError(f"unexpected registry URL: {url}")

            with (
                mock.patch.object(executor, "_download", side_effect=download),
                mock.patch.object(
                    executor,
                    "_verify_sri",
                    side_effect=CandidateError("forced SRI mismatch"),
                ) as verify_sri,
                mock.patch.object(
                    executor, "_safe_extract_registry_package"
                ) as extract,
            ):
                with self.assertRaisesRegex(CandidateError, "SRI"):
                    executor._prefetch_lock_packages(workspace, lock)
            verify_sri.assert_called_once_with(
                mock.ANY,
                PLAYWRIGHT_CORE_INTEGRITY,
            )
            extract.assert_not_called()
            self.assertFalse(
                (Path(workspace.root) / "playwright-core-package").exists()
            )

    def test_playwright_archive_layout_version_and_post_extract_registry_fail_closed(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        node_location = shutil.which("node")
        if node_location is None:
            self.fail("the locked Playwright compatibility oracle requires Node")
        node = Path(node_location).resolve()
        valid_executable = b"#!/bin/sh\nprintf '%s\\n' 'HeadlessChrome 151.0.7922.34'\n"
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            workspace = self.playwright_workspace(root)
            lock, package, _package_archive = self.install_locked_playwright_core(
                workspace
            )
            browser_cache = Path(workspace.browser_cache).resolve()
            expected = self.expected_playwright_observation(browser_cache)
            expected_directory = Path(str(expected["directory"]))
            expected_executable = Path(str(expected["executablePath"]))
            expected_archive = Path(workspace.root) / "chromium-headless-shell-1234.zip"
            real_run = subprocess.run

            structural_archive = root / "structural-browser-fixture.zip"
            structural_output = root / "structural-browser-output"
            self.write_browser_archive(structural_archive)
            with mock.patch.object(
                executor,
                "_browser_archive_inventory",
                side_effect=self.structural_browser_inventory,
                create=True,
            ) as inventory:
                executor._safe_extract_zip(structural_archive, structural_output)
            inventory.assert_called_once_with(structural_archive)
            structural_executable = structural_output / PLAYWRIGHT_BROWSER_MEMBER
            self.assertTrue(structural_executable.is_file())
            self.assertFalse(structural_executable.is_symlink())
            self.assertEqual(structural_executable.read_bytes(), valid_executable)

            def clean_acquisition() -> None:
                if expected_directory.exists() or expected_directory.is_symlink():
                    if expected_directory.is_symlink():
                        expected_directory.unlink()
                    else:
                        shutil.rmtree(expected_directory)
                expected_archive.unlink(missing_ok=True)

            def successful_process(
                argv: list[str], **kwargs: object
            ) -> subprocess.CompletedProcess[str]:
                if tuple(str(value) for value in argv[:2]) == (str(node), "-e"):
                    return real_run(argv, **kwargs)
                self.assertEqual(
                    tuple(str(value) for value in argv),
                    (
                        str(expected_executable),
                        "--version",
                    ),
                )
                return subprocess.CompletedProcess(
                    argv,
                    0,
                    stdout="HeadlessChrome 151.0.7922.34\n",
                    stderr="",
                )

            invalid_archives = (
                (
                    "missing-executable",
                    (("chrome-headless-shell-linux64/README", b"missing", 0o100644),),
                ),
                (
                    "recursive-basename-only",
                    (
                        (
                            "other/nested/chrome-headless-shell",
                            valid_executable,
                            0o100755,
                        ),
                    ),
                ),
                (
                    "duplicate-exact-member",
                    (
                        (PLAYWRIGHT_BROWSER_MEMBER, valid_executable, 0o100755),
                        (PLAYWRIGHT_BROWSER_MEMBER, valid_executable, 0o100755),
                    ),
                ),
                (
                    "duplicate-basename-elsewhere",
                    (
                        (PLAYWRIGHT_BROWSER_MEMBER, valid_executable, 0o100755),
                        (
                            "other/chrome-headless-shell",
                            valid_executable,
                            0o100755,
                        ),
                    ),
                ),
                (
                    "symlink-executable",
                    ((PLAYWRIGHT_BROWSER_MEMBER, b"elsewhere", 0o120777),),
                ),
                (
                    "non-regular-executable",
                    ((PLAYWRIGHT_BROWSER_MEMBER, valid_executable, 0o060644),),
                ),
            )
            for name, members in invalid_archives:
                with self.subTest(archive=name):
                    clean_acquisition()

                    def download_exact(destination: Path) -> None:
                        self.write_browser_archive(destination, members)

                    with (
                        mock.patch.object(
                            executor,
                            "_download_exact_browser",
                            side_effect=download_exact,
                            create=True,
                        ) as exact_download,
                        mock.patch.object(
                            executor,
                            "_browser_archive_inventory",
                            side_effect=self.structural_browser_inventory,
                            create=True,
                        ) as inventory,
                        mock.patch.object(executor, "_download") as generic_download,
                        mock.patch.object(
                            executor.subprocess,
                            "run",
                            side_effect=successful_process,
                        ),
                    ):
                        with self.assertRaises(CandidateError):
                            executor._acquire_browser(workspace, node, lock, package)
                    exact_download.assert_called_once_with(expected_archive)
                    self.assertGreaterEqual(inventory.call_count, 1)
                    generic_download.assert_not_called()

            clean_acquisition()
            expected_directory.mkdir()
            with (
                mock.patch.object(
                    executor.subprocess,
                    "run",
                    side_effect=successful_process,
                ),
                mock.patch.object(executor, "_download") as download,
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=AssertionError(
                        "pre-existing directory reached exact browser download"
                    ),
                    create=True,
                ) as exact_download,
            ):
                with self.assertRaisesRegex(CandidateError, "absent|exist|directory"):
                    executor._acquire_browser(workspace, node, lock, package)
            download.assert_not_called()
            exact_download.assert_not_called()
            clean_acquisition()

            symlink_target = root / "symlinked-browser-directory"
            symlink_target.mkdir()
            expected_directory.symlink_to(symlink_target, target_is_directory=True)
            with (
                mock.patch.object(
                    executor.subprocess,
                    "run",
                    side_effect=successful_process,
                ),
                mock.patch.object(executor, "_download") as download,
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=AssertionError(
                        "symlinked directory reached exact browser download"
                    ),
                    create=True,
                ) as exact_download,
            ):
                with self.assertRaises(CandidateError):
                    executor._acquire_browser(workspace, node, lock, package)
            download.assert_not_called()
            exact_download.assert_not_called()
            clean_acquisition()

            def valid_exact_download(destination: Path) -> None:
                self.write_browser_archive(destination)

            identity_paths: list[Path] = []
            real_file_sha256 = executor.file_sha256

            def frozen_structural_identity(path: Path) -> str:
                observed = Path(path)
                if observed == expected_archive:
                    identity_paths.append(observed)
                    return str(
                        CONTENT_BOUND_BROWSER_PROFILE["raw_archive_sha256"]
                    )
                if observed == expected_executable:
                    identity_paths.append(observed)
                    return str(
                        CONTENT_BOUND_BROWSER_PROFILE["executable_sha256"]
                    )
                return real_file_sha256(observed)

            def wrong_version(
                argv: list[str], **kwargs: object
            ) -> subprocess.CompletedProcess[str]:
                if tuple(str(value) for value in argv[:2]) == (str(node), "-e"):
                    return real_run(argv, **kwargs)
                return subprocess.CompletedProcess(
                    argv,
                    0,
                    stdout="HeadlessChrome 151.0.7922.35\n",
                    stderr="",
                )

            with (
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=valid_exact_download,
                    create=True,
                ) as exact_download,
                mock.patch.object(
                    executor,
                    "_browser_archive_inventory",
                    side_effect=self.structural_browser_inventory,
                    create=True,
                ) as inventory,
                mock.patch.object(
                    executor,
                    "file_sha256",
                    side_effect=frozen_structural_identity,
                    create=True,
                ) as identity,
                mock.patch.object(executor, "_download") as generic_download,
                mock.patch.object(
                    executor.subprocess, "run", side_effect=wrong_version
                ),
            ):
                with self.assertRaisesRegex(CandidateError, "version"):
                    executor._acquire_browser(workspace, node, lock, package)
            exact_download.assert_called_once_with(expected_archive)
            self.assertGreaterEqual(inventory.call_count, 1)
            identity.assert_has_calls(
                [mock.call(expected_archive), mock.call(expected_executable)],
                any_order=True,
            )
            self.assertEqual(
                set(identity_paths), {expected_archive, expected_executable}
            )
            generic_download.assert_not_called()
            clean_acquisition()
            identity_paths.clear()

            def launch_failure(argv: list[str], **kwargs: object) -> object:
                if tuple(str(value) for value in argv[:2]) == (str(node), "-e"):
                    return real_run(argv, **kwargs)
                raise subprocess.CalledProcessError(7, argv, output="launch failed")

            with (
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=valid_exact_download,
                    create=True,
                ) as exact_download,
                mock.patch.object(
                    executor,
                    "_browser_archive_inventory",
                    side_effect=self.structural_browser_inventory,
                    create=True,
                ) as inventory,
                mock.patch.object(
                    executor,
                    "file_sha256",
                    side_effect=frozen_structural_identity,
                    create=True,
                ) as identity,
                mock.patch.object(executor, "_download") as generic_download,
                mock.patch.object(
                    executor.subprocess, "run", side_effect=launch_failure
                ),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    executor._acquire_browser(workspace, node, lock, package)
            exact_download.assert_called_once_with(expected_archive)
            self.assertGreaterEqual(inventory.call_count, 1)
            identity.assert_has_calls(
                [mock.call(expected_archive), mock.call(expected_executable)],
                any_order=True,
            )
            self.assertEqual(
                set(identity_paths), {expected_archive, expected_executable}
            )
            generic_download.assert_not_called()
            clean_acquisition()
            identity_paths.clear()

            registry_calls = 0

            def registry_after_extract_failure(
                argv: list[str], **kwargs: object
            ) -> object:
                nonlocal registry_calls
                if tuple(str(value) for value in argv[:2]) == (str(node), "-e"):
                    registry_calls += 1
                    if registry_calls == 2:
                        raise subprocess.CalledProcessError(
                            8, argv, output="post-extract registry failure"
                        )
                    return real_run(argv, **kwargs)
                return subprocess.CompletedProcess(
                    argv,
                    0,
                    stdout="HeadlessChrome 151.0.7922.34\n",
                    stderr="",
                )

            with (
                mock.patch.object(
                    executor,
                    "_download_exact_browser",
                    side_effect=valid_exact_download,
                    create=True,
                ) as exact_download,
                mock.patch.object(
                    executor,
                    "_browser_archive_inventory",
                    side_effect=self.structural_browser_inventory,
                    create=True,
                ) as inventory,
                mock.patch.object(
                    executor,
                    "file_sha256",
                    side_effect=frozen_structural_identity,
                    create=True,
                ) as identity,
                mock.patch.object(executor, "_download") as generic_download,
                mock.patch.object(
                    executor.subprocess,
                    "run",
                    side_effect=registry_after_extract_failure,
                ),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    executor._acquire_browser(workspace, node, lock, package)
            exact_download.assert_called_once_with(expected_archive)
            self.assertGreaterEqual(inventory.call_count, 1)
            identity.assert_has_calls(
                [mock.call(expected_archive), mock.call(expected_executable)],
                any_order=True,
            )
            self.assertEqual(
                set(identity_paths), {expected_archive, expected_executable}
            )
            generic_download.assert_not_called()
            self.assertEqual(registry_calls, 2)
            self.assertTrue(expected_executable.is_file())

    def test_equal_h2_acquisition_identities_reach_existing_receipt_path(self) -> None:
        executor = importlib.import_module("python_candidate_h2")

        class ExistingReceiptPathReached(Exception):
            pass

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            baseline = self.diagnostic_acquired_inputs(executor, root)
            first = executor.RunObservation((), (), (), 0, baseline)
            second = executor.RunObservation((), (), (), 0, baseline)
            family = executor.CandidateFamily(
                root / "candidate.tar.gz", (), "0.1.0a1", ()
            )
            workspaces = (
                mock.Mock(root=root / "clean-run-1"),
                mock.Mock(root=root / "clean-run-2"),
            )
            with (
                mock.patch.object(executor, "_asset_equality") as asset_equality,
                mock.patch.object(
                    executor,
                    "_validate_abstract_resources",
                    side_effect=ExistingReceiptPathReached,
                ) as validate_resources,
            ):
                with self.assertRaises(ExistingReceiptPathReached):
                    executor.observe_h2(
                        expected_commit=self.REVISION,
                        family=family,
                        extracted=root / "source",
                        workspaces=workspaces,
                        runs=(first, second),
                        source_date_epoch=123456789,
                    )
            asset_equality.assert_called_once_with(root / "source", (first, second))
            validate_resources.assert_called_once_with(family, (first, second))

    def test_every_h2_acquisition_field_drift_is_exact_secret_free_and_closed(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            baseline = self.diagnostic_acquired_inputs(executor, root)
            baseline_wheel = baseline.python_wheels[1]
            mutant_wheel = {**baseline_wheel, "sha256": "f" * 64}
            mutant_wheels = (baseline.python_wheels[0], mutant_wheel)
            baseline_manifest = baseline.package_manifests[0]
            mutant_manifest = (
                baseline_manifest[0],
                {
                    **baseline_manifest[1],
                    "scripts": {"prepare": "manifest-mutant-secret-canary"},
                },
            )
            mutant_manifests = (mutant_manifest, baseline.package_manifests[1])
            baseline_packument = baseline.package_packuments[0]
            mutant_packument = (
                baseline_packument[0],
                {
                    **baseline_packument[1],
                    "dist-tags": {"latest": "packument-mutant-secret-canary"},
                },
            )
            mutant_packuments = (mutant_packument, baseline.package_packuments[1])
            mutations = (
                ("browser_archive_sha256", "1" * 64, None, None, None),
                ("browser_executable_sha256", "2" * 64, None, None, None),
                (
                    "browser_platform",
                    "private-platform-雪-canary",
                    None,
                    None,
                    None,
                ),
                (
                    "playwright_test_integrity",
                    "sha512-test-mutant-secret-canary",
                    None,
                    None,
                    None,
                ),
                (
                    "playwright_core_integrity",
                    "sha512-core-mutant-secret-canary",
                    None,
                    None,
                    None,
                ),
                (
                    "python_wheels",
                    mutant_wheels,
                    str(baseline_wheel["filename"]),
                    baseline_wheel,
                    mutant_wheel,
                ),
                ("anywidget_license_sha256", "3" * 64, None, None, None),
                (
                    "package_manifests",
                    mutant_manifests,
                    str(baseline_manifest[0]),
                    baseline_manifest,
                    mutant_manifest,
                ),
                (
                    "package_packuments",
                    mutant_packuments,
                    str(baseline_packument[0]),
                    baseline_packument,
                    mutant_packument,
                ),
                ("locked_package_bytes", 11, None, None, None),
                ("python_wheel_bytes", 21, None, None, None),
                ("browser_member_count", 288, None, None, None),
                (
                    "browser_expanded_regular_bytes",
                    273_378_829,
                    None,
                    None,
                    None,
                ),
            )
            family = executor.CandidateFamily(
                root / "candidate.tar.gz", (), "0.1.0a1", ()
            )
            workspaces = (
                mock.Mock(root=root / "clean-run-1"),
                mock.Mock(root=root / "clean-run-2"),
            )
            first = executor.RunObservation((), (), (), 0, baseline)
            forbidden = (
                str(root),
                "private-host-canary",
                "manifest-secret-canary",
                "manifest-mutant-secret-canary",
                "registry-token-canary",
                "header-secret-canary",
                "packument-mutant-secret-canary",
                "https://",
                "sha512-",
                "linux-x86_64",
                "private-platform-canary",
                "雪",
                "a" * 64,
                "b" * 64,
                "c" * 64,
                "d" * 64,
                "e" * 64,
                "f" * 64,
                "1" * 64,
                "2" * 64,
                "3" * 64,
            )
            for field, value, identity, first_member, second_member in mutations:
                with self.subTest(field=field):
                    acquired = replace(baseline, **{field: value})
                    second = executor.RunObservation((), (), (), 0, acquired)
                    expected = self.expected_acquisition_drift_message(
                        field,
                        getattr(baseline, field),
                        value,
                        member_identity=identity,
                        first_member=first_member,
                        second_member=second_member,
                    )
                    with (
                        mock.patch.object(
                            executor, "_asset_equality"
                        ) as asset_equality,
                        mock.patch.object(
                            executor, "_validate_abstract_resources"
                        ) as validate_resources,
                    ):
                        with self.assertRaises(CandidateError) as raised:
                            executor.observe_h2(
                                expected_commit=self.REVISION,
                                family=family,
                                extracted=root / "source",
                                workspaces=workspaces,
                                runs=(first, second),
                                source_date_epoch=123456789,
                            )
                    self.assertEqual(str(raised.exception), expected)
                    asset_equality.assert_called_once_with(
                        root / "source", (first, second)
                    )
                    validate_resources.assert_not_called()
                    for secret in forbidden:
                        self.assertNotIn(secret, str(raised.exception))

    def test_h2_collection_membership_drift_marks_only_added_or_removed_member(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            baseline = self.diagnostic_acquired_inputs(executor, root)
            added_wheel = {
                "name": "zeta-widget",
                "version": "1.0.0",
                "filename": "zeta_widget-1.0.0-py3-none-any.whl",
                "sha256": "4" * 64,
            }
            added_manifest = (
                "node_modules/zeta-widget",
                {
                    "name": "zeta-widget",
                    "version": "1.0.0",
                    "description": "added-manifest-secret-雪",
                },
            )
            added_packument = (
                "zeta-widget",
                {
                    "name": "zeta-widget",
                    "description": "added-packument-secret-雪",
                    "versions": {
                        "1.0.0": {"name": "zeta-widget", "version": "1.0.0"}
                    },
                },
            )
            membership_mutations = (
                (
                    "python_wheels",
                    baseline.python_wheels[1:],
                    str(baseline.python_wheels[0]["filename"]),
                    baseline.python_wheels[0],
                    None,
                ),
                (
                    "python_wheels",
                    (*baseline.python_wheels, added_wheel),
                    str(added_wheel["filename"]),
                    None,
                    added_wheel,
                ),
                (
                    "package_manifests",
                    baseline.package_manifests[1:],
                    str(baseline.package_manifests[0][0]),
                    baseline.package_manifests[0],
                    None,
                ),
                (
                    "package_manifests",
                    (*baseline.package_manifests, added_manifest),
                    str(added_manifest[0]),
                    None,
                    added_manifest,
                ),
                (
                    "package_packuments",
                    baseline.package_packuments[1:],
                    str(baseline.package_packuments[0][0]),
                    baseline.package_packuments[0],
                    None,
                ),
                (
                    "package_packuments",
                    (*baseline.package_packuments, added_packument),
                    str(added_packument[0]),
                    None,
                    added_packument,
                ),
            )
            family = executor.CandidateFamily(
                root / "candidate.tar.gz", (), "0.1.0a1", ()
            )
            workspaces = (
                mock.Mock(root=root / "clean-run-1"),
                mock.Mock(root=root / "clean-run-2"),
            )
            first = executor.RunObservation((), (), (), 0, baseline)
            for field, value, identity, first_member, second_member in (
                membership_mutations
            ):
                with self.subTest(field=field, identity=identity):
                    acquired = replace(baseline, **{field: value})
                    second = executor.RunObservation((), (), (), 0, acquired)
                    expected = self.expected_acquisition_drift_message(
                        field,
                        getattr(baseline, field),
                        value,
                        member_identity=identity,
                        first_member=first_member,
                        second_member=second_member,
                    )
                    with (
                        mock.patch.object(executor, "_asset_equality"),
                        mock.patch.object(
                            executor, "_validate_abstract_resources"
                        ) as validate_resources,
                    ):
                        with self.assertRaises(CandidateError) as raised:
                            executor.observe_h2(
                                expected_commit=self.REVISION,
                                family=family,
                                extracted=root / "source",
                                workspaces=workspaces,
                                runs=(first, second),
                                source_date_epoch=123456789,
                            )
                    self.assertEqual(str(raised.exception), expected)
                    validate_resources.assert_not_called()
                    self.assertIn('"absent"', str(raised.exception))
                    self.assertNotIn(str(root), str(raised.exception))
                    self.assertNotIn("secret-雪", str(raised.exception))

    def test_h2_multi_field_diagnostic_uses_existing_comparison_order(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            baseline = self.diagnostic_acquired_inputs(executor, root)
            mutant_wheels = tuple(
                {**record, "sha256": str(index + 6) * 64}
                for index, record in enumerate(baseline.python_wheels)
            )
            mutant_manifests = tuple(
                (
                    identity,
                    {
                        **record,
                        "ordering_mutant": f"manifest-order-{index}-雪",
                    },
                )
                for index, (identity, record) in enumerate(
                    baseline.package_manifests
                )
            )
            mutant_packuments = tuple(
                (
                    identity,
                    {
                        **record,
                        "ordering_mutant": f"packument-order-{index}-雪",
                    },
                )
                for index, (identity, record) in enumerate(
                    baseline.package_packuments
                )
            )
            changes = (
                ("browser_archive_sha256", "4" * 64),
                ("browser_executable_sha256", "5" * 64),
                ("browser_platform", "ordered-platform-雪"),
                ("playwright_test_integrity", "sha512-ordered-test-雪"),
                ("playwright_core_integrity", "sha512-ordered-core-雪"),
                ("python_wheels", mutant_wheels),
                ("anywidget_license_sha256", "8" * 64),
                ("package_manifests", mutant_manifests),
                ("package_packuments", mutant_packuments),
                ("locked_package_bytes", 11),
                ("python_wheel_bytes", 21),
                ("browser_member_count", 288),
                ("browser_expanded_regular_bytes", 273_378_829),
            )
            acquired = replace(baseline, **dict(changes))
            prefix = "H2 isolated runs acquired different external inputs: "
            collection_members = {
                "python_wheels": tuple(
                    (
                        str(first["filename"]),
                        first,
                        second,
                    )
                    for first, second in zip(
                        baseline.python_wheels, mutant_wheels, strict=True
                    )
                ),
                "package_manifests": tuple(
                    (str(first[0]), first, second)
                    for first, second in zip(
                        baseline.package_manifests,
                        mutant_manifests,
                        strict=True,
                    )
                ),
                "package_packuments": tuple(
                    (str(first[0]), first, second)
                    for first, second in zip(
                        baseline.package_packuments,
                        mutant_packuments,
                        strict=True,
                    )
                ),
            }
            for field, value in changes:
                self.assertNotEqual(getattr(baseline, field), value)
            differences = [
                self.independent_acquisition_difference(
                    field,
                    getattr(baseline, field),
                    value,
                    members=collection_members.get(field, ()),
                )
                for field, value in changes
            ]
            expected = prefix + json.dumps(
                {"differences": differences},
                ensure_ascii=False,
                allow_nan=False,
                sort_keys=True,
                separators=(",", ":"),
            )
            first = executor.RunObservation((), (), (), 0, baseline)
            second = executor.RunObservation((), (), (), 0, acquired)
            family = executor.CandidateFamily(
                root / "candidate.tar.gz", (), "0.1.0a1", ()
            )
            workspaces = (
                mock.Mock(root=root / "clean-run-1"),
                mock.Mock(root=root / "clean-run-2"),
            )
            with (
                mock.patch.object(executor, "_asset_equality"),
                mock.patch.object(
                    executor, "_validate_abstract_resources"
                ) as validate_resources,
            ):
                with self.assertRaises(CandidateError) as raised:
                    executor.observe_h2(
                        expected_commit=self.REVISION,
                        family=family,
                        extracted=root / "source",
                        workspaces=workspaces,
                        runs=(first, second),
                        source_date_epoch=123456789,
                    )
            self.assertEqual(str(raised.exception), expected)
            validate_resources.assert_not_called()
            self.assertNotIn(str(root), str(raised.exception))
            self.assertNotIn("雪", str(raised.exception))

    def test_h2_compatibility_failure_never_publishes_receipt(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family_path = root / "family"
            self.write_exact_family(family_path)
            admitted = executor.CandidateFamily(
                family_path / "eqiora-0.1.0a1.tar.gz",
                (),
                "0.1.0a1",
                (),
            )

            output = root / "h2-output"
            output.mkdir()
            scratch_parent = root / "h2-scratch"
            scratch_parent.mkdir()

            def extract(_archive: Path, destination: Path) -> Path:
                destination.mkdir(parents=True)
                return destination

            with (
                mock.patch.object(
                    executor, "_current_revision", return_value=self.REVISION
                ),
                mock.patch.object(
                    executor,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(executor, "checked_run", return_value="123456789"),
                mock.patch.object(
                    executor,
                    "home_scratch_parent",
                    return_value=scratch_parent,
                ),
                mock.patch.object(
                    executor,
                    "admit_candidate_family",
                    return_value=admitted,
                ),
                mock.patch.object(executor, "safe_extract_sdist", side_effect=extract),
                mock.patch.object(
                    executor,
                    "_retained_distribution_version",
                    return_value="0.1.0a1",
                ),
                mock.patch.object(executor, "stage_frontend"),
                mock.patch.object(
                    executor,
                    "run_frontend_commands",
                    wraps=executor.run_frontend_commands,
                ) as frontend,
                mock.patch.object(
                    executor,
                    "acquire_inputs",
                    side_effect=CandidateError(
                        "forced compatibility observation failure"
                    ),
                ) as acquire,
                mock.patch.object(executor, "observe_h2") as observe,
                mock.patch.object(executor, "write_canonical_receipt") as publish,
            ):
                with self.assertRaisesRegex(
                    CandidateError, "forced compatibility observation failure"
                ):
                    executor.execute_h2(
                        expected_commit=self.REVISION,
                        artifacts=family_path,
                        out=output,
                    )
            frontend.assert_called_once()
            acquire.assert_called_once()
            observe.assert_not_called()
            publish.assert_not_called()
            self.assertEqual(tuple(output.iterdir()), ())

    def test_real_finalizer_host_failure_withholds_dependent_evidence(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        profiles = importlib.import_module("python_candidate_profiles")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family_path = root / "family"
            metadata = root / "metadata"
            receipt_path = root / "eqiora-0.1.0a1-python-candidate-h2.json"
            self.write_exact_family(family_path)
            admitted = executor.CandidateFamily(
                family_path / "eqiora-0.1.0a1.tar.gz",
                (),
                "0.1.0a1",
                (),
            )
            receipt_path.write_bytes(b"sealed independent H2 receipt")
            acquired = self.acquired_inputs(executor, root)
            receipt = {
                "browser": {
                    "downloaded_archive_sha256": acquired.browser_archive_sha256,
                    "executable_sha256": acquired.browser_executable_sha256,
                    "platform": acquired.browser_platform,
                },
                "python_host": {
                    "resolved_environment_sha256": executor.structured_sha256(
                        acquired.python_wheels
                    )
                },
            }
            frontend = {
                "h2_receipt_sha256": hashlib.sha256(
                    executor.canonical_json_bytes(receipt)
                ).hexdigest()
            }
            extracted = root / "extracted"
            exact_app = (
                extracted / python_candidate_module.EXACT_CYLINDER_STOKES_MARIMO_APP
            )
            exact_app.parent.mkdir(parents=True)
            exact_app.write_text("# exact candidate app\n", encoding="utf-8")
            workspace_root = root / "notebook-profile"
            workspace = types.SimpleNamespace(
                root=workspace_root,
                environment=workspace_root / "environment",
                consumer=workspace_root / "consumer",
            )
            python = workspace.environment / (
                "Scripts/python.exe" if os.name == "nt" else "bin/python"
            )
            emitted: list[str] = []
            commands: list[tuple[str, ...]] = []
            run_calls: list[tuple[tuple[str, ...], dict[str, object]]] = []
            original_observer = profiles.run_notebook_profile

            def observe_checks(
                observations: tuple[tuple[str, Callable[[], None]], ...],
                *,
                emit: Callable[[str], None],
            ) -> tuple[str, ...]:
                def record(name: str) -> None:
                    emitted.append(name)
                    emit(name)

                return original_observer(observations, emit=record)

            def checked_run(argv: list[str], **kwargs: object) -> str:
                command = tuple(str(value) for value in argv)
                commands.append(command)
                run_calls.append((command, dict(kwargs)))
                if command == (
                    "npm",
                    "run",
                    "test:hosts",
                    "--",
                    "--project=marimo-0.23.16",
                    "tests/exact-cylinder-stokes-marimo.spec.ts",
                ):
                    raise CandidateError("forced downstream host launch failure")
                return ""

            def stage_frontend(_source: Path, build: object) -> None:
                Path(build.frontend).mkdir(parents=True)

            process = mock.Mock()
            process.poll.return_value = None
            process.wait.return_value = 0
            install = mock.Mock(return_value=python)
            checked = mock.Mock(side_effect=checked_run)
            popen = mock.Mock(return_value=process)

            def cleanup(**kwargs: object) -> None:
                primary_error = kwargs["primary_error"]
                if isinstance(primary_error, BaseException):
                    raise primary_error

            def run_profiles(**_kwargs: object) -> object:
                with (
                    mock.patch.object(
                        profiles,
                        "run_notebook_profile",
                        side_effect=observe_checks,
                    ),
                    mock.patch.object(
                        profiles,
                        "install_environment",
                        new=install,
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "checked_run",
                        new=checked,
                    ),
                    mock.patch.object(
                        python_candidate_module.subprocess,
                        "Popen",
                        new=popen,
                    ),
                    mock.patch.object(
                        python_candidate_module.socket,
                        "create_connection",
                        return_value=mock.MagicMock(),
                    ),
                    mock.patch.object(
                        executor,
                        "stage_frontend",
                        side_effect=stage_frontend,
                    ),
                    mock.patch.object(
                        executor,
                        "acquire_inputs",
                        return_value=acquired,
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "_notebook_cleanup_lifecycle",
                        side_effect=cleanup,
                    ),
                ):
                    return python_candidate_module.run_notebook_profile(
                        uv="/reviewed/uv",
                        interpreter="/reviewed/python3.13",
                        wheel=root / "candidate.whl",
                        extracted=extracted,
                        workspace=workspace,
                        config=python_candidate_module.load_config(),
                        receipt=receipt,
                        frontend=frontend,
                    )

            with (
                mock.patch.object(
                    python_candidate_module,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "admit_candidate_family",
                    return_value=admitted,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "family_inventory",
                    return_value=(),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "validate_h2_receipt",
                    return_value=receipt,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "derive_frontend_manifest",
                    return_value=frontend,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    side_effect=run_profiles,
                ) as profile_runner,
                mock.patch.object(python_candidate_module, "write_manifest") as write,
                mock.patch.dict(
                    os.environ,
                    {"EQIORA_GMSH": "/ambient/gmsh", "PATH": "/ambient/bin"},
                ),
            ):
                with self.assertRaisesRegex(
                    CandidateError, "forced downstream host launch failure"
                ):
                    python_candidate_module.finalize_candidate(
                        expected_commit=self.REVISION,
                        artifacts=family_path,
                        h2_receipt=receipt_path,
                        manifest_out=metadata,
                    )

            profile_runner.assert_called_once()
            write.assert_not_called()
            self.assertEqual(
                emitted,
                list(NOTEBOOK_PROFILE_CHECKS[:5]),
            )
            self.assertIn(
                (
                    "npm",
                    "run",
                    "test:hosts",
                    "--",
                    "--project=marimo-0.23.16",
                    "tests/exact-cylinder-stokes-marimo.spec.ts",
                ),
                commands,
            )
            install.assert_called_once_with(
                uv="/reviewed/uv",
                interpreter="/reviewed/python3.13",
                environment=workspace.environment,
                requirements=[
                    f"{root / 'candidate.whl'}[gmsh,matplotlib,notebook]",
                    python_candidate_module.load_config().pytest,
                    "anywidget==0.11.0",
                    "marimo==0.23.16",
                ],
                run=checked,
            )
            popen_calls = popen.call_args_list
            self.assertEqual(len(popen_calls), 1)
            self.assertEqual(
                [tuple(call.args[0][2:5]) for call in popen_calls],
                [("-m", "marimo", "run")],
            )
            self.assertEqual([call.args[0][0] for call in popen_calls], [str(python)])
            self.assertEqual(
                popen_calls[0].kwargs["cwd"],
                workspace.root / "exact-cylinder-stokes-marimo-positive",
            )
            expected_gmsh = str(
                python.parent / ("gmsh.exe" if os.name == "nt" else "gmsh")
            )
            environments = [
                (f"host-popen-{index}", call.kwargs["env"])
                for index, call in enumerate(popen_calls)
            ]
            for surface, environment in environments:
                with self.subTest(notebook_environment=surface):
                    self.assertEqual(environment.get("EQIORA_GMSH"), expected_gmsh)
                    self.assertEqual(
                        environment.get("PATH", "").split(os.pathsep)[0],
                        str(python.parent),
                    )
            self.assertEqual(
                receipt_path.read_bytes(), b"sealed independent H2 receipt"
            )
            self.assertEqual(tuple(metadata.iterdir()), ())

    def test_one_conventional_h2_executor_owns_the_exact_cli(self) -> None:
        matches = tuple(
            sorted((REPOSITORY_ROOT / "tools/release").glob("python_candidate_h2*.py"))
        )
        self.assertEqual(matches, (self.H2_EXECUTOR,))

        executor = importlib.import_module("python_candidate_h2")
        with mock.patch.object(
            sys,
            "argv",
            [
                str(self.H2_EXECUTOR),
                "--expected-commit",
                self.REVISION,
                "--artifacts",
                "/immutable-family",
                "--out",
                "/empty-h2-output",
            ],
        ):
            arguments = executor.parse_args()

        self.assertEqual(arguments.expected_commit, self.REVISION)
        self.assertEqual(arguments.artifacts, Path("/immutable-family"))
        self.assertEqual(arguments.out, Path("/empty-h2-output"))
        self.assertEqual(
            set(vars(arguments)),
            {"expected_commit", "artifacts", "out"},
        )

    def test_candidate_cli_is_exactly_prepare_or_finalize(self) -> None:
        invocations = (
            (
                [
                    "python_candidate.py",
                    "prepare",
                    "--expected-commit",
                    self.REVISION,
                    "--out",
                    "/immutable-family",
                ],
                {
                    "command": "prepare",
                    "expected_commit": self.REVISION,
                    "out": Path("/immutable-family"),
                    "require_tag": False,
                },
            ),
            (
                [
                    "python_candidate.py",
                    "prepare",
                    "--expected-commit",
                    self.REVISION,
                    "--out",
                    "/immutable-family",
                    "--require-tag",
                ],
                {
                    "command": "prepare",
                    "expected_commit": self.REVISION,
                    "out": Path("/immutable-family"),
                    "require_tag": True,
                },
            ),
            (
                [
                    "python_candidate.py",
                    "finalize",
                    "--expected-commit",
                    self.REVISION,
                    "--artifacts",
                    "/immutable-family",
                    "--h2-receipt",
                    "/h2/eqiora-0.1.0a1-python-candidate-h2.json",
                    "--manifest-out",
                    "/metadata",
                ],
                {
                    "command": "finalize",
                    "expected_commit": self.REVISION,
                    "artifacts": Path("/immutable-family"),
                    "h2_receipt": Path("/h2/eqiora-0.1.0a1-python-candidate-h2.json"),
                    "manifest_out": Path("/metadata"),
                },
            ),
        )
        for argv, expected in invocations:
            with self.subTest(command=argv[1]), mock.patch.object(sys, "argv", argv):
                arguments = python_candidate_module.parse_args()
                self.assertEqual(vars(arguments), expected)

    def test_frontend_command_callback_sequence_and_environment_unit(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        expected_commands = (
            ("npm", "ci", "--ignore-scripts"),
            ("npm", "run", "typecheck"),
            ("npm", "run", "lint"),
            ("npm", "run", "test"),
            ("npm", "run", "build"),
        )
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            scratch = Path(temporary)
            workspaces = executor.create_isolated_build_workspaces(scratch)
            calls: list[tuple[tuple[str, ...], Path, dict[str, str]]] = []

            def run(argv: list[str], **kwargs: object) -> str:
                calls.append(
                    (
                        tuple(argv),
                        Path(kwargs["cwd"]),
                        dict(kwargs["extra_environment"]),
                    )
                )
                return ""

            for workspace in workspaces:
                executor.run_frontend_commands(
                    workspace,
                    source_date_epoch=123456789,
                    run=run,
                )

        self.assertEqual(len(workspaces), 2)
        self.assertEqual(
            tuple(command for command, _, _ in calls),
            expected_commands * 2,
        )
        for index, workspace in enumerate(workspaces):
            owned = calls[
                index * len(expected_commands) : (index + 1) * len(expected_commands)
            ]
            for _, cwd, environment in owned:
                self.assertEqual(cwd, workspace.frontend)
                self.assertEqual(environment["HOME"], str(workspace.home))
                self.assertEqual(
                    environment["npm_config_cache"], str(workspace.npm_cache)
                )
                self.assertEqual(environment["TMPDIR"], str(workspace.temporary))
                self.assertEqual(
                    environment["PLAYWRIGHT_BROWSERS_PATH"],
                    str(workspace.browser_cache),
                )
                self.assertEqual(environment["SOURCE_DATE_EPOCH"], "123456789")
                self.assertEqual(environment["LC_ALL"], "C.UTF-8")
                self.assertEqual(environment["TZ"], "UTC")
            self.assertEqual(
                workspace.installation, workspace.frontend / "node_modules"
            )
            self.assertEqual(workspace.output, workspace.frontend / "dist")

        for failure_index in range(len(expected_commands)):
            with self.subTest(nonzero=expected_commands[failure_index]):
                observed: list[tuple[str, ...]] = []

                def fail(argv: list[str], **_kwargs: object) -> str:
                    observed.append(tuple(argv))
                    if len(observed) - 1 == failure_index:
                        raise RuntimeError("forced nonzero command")
                    return ""

                with self.assertRaisesRegex(RuntimeError, "forced nonzero"):
                    executor.run_frontend_commands(
                        workspaces[0],
                        source_date_epoch=123456789,
                        run=fail,
                    )
                self.assertEqual(observed, list(expected_commands[: failure_index + 1]))

    def test_executor_rejects_wrong_revision_without_partial_receipt(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "family"
            output = root / "h2-output"
            family.mkdir()
            output.mkdir()
            completed = subprocess.run(
                [
                    sys.executable,
                    str(self.H2_EXECUTOR),
                    "--expected-commit",
                    "0" * 40,
                    "--artifacts",
                    str(family),
                    "--out",
                    str(output),
                ],
                cwd=REPOSITORY_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            output_members = tuple(output.iterdir())

        self.assertEqual(completed.returncode, 2)
        self.assertRegex(completed.stderr, "revision|commit")
        self.assertEqual(output_members, ())

    def test_h2_family_admission_is_one_sdist_and_four_exact_cpython_wheels(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "family"
            self.write_exact_family(family)
            expected_inventory = self.expected_family_inventory(family)
            admitted = executor.admit_candidate_family(family)
            self.assertEqual(admitted.sdist, family / "eqiora-0.1.0a1.tar.gz")
            self.assertTrue(admitted.sdist.is_file())
            self.assertFalse(admitted.sdist.is_symlink())
            self.assertEqual(admitted.sdist.stat().st_nlink, 1)
            self.assertEqual(
                tuple(path.name for path in admitted.wheels),
                tuple(exact_wheel_name(python) for python in EXACT_WHEEL_INTERPRETERS),
            )
            self.assertEqual(admitted.inventory, expected_inventory)
            self.assertEqual(executor.family_inventory(family), expected_inventory)

            for compact, wheel in zip(
                EXACT_WHEEL_INTERPRETERS,
                admitted.wheels,
                strict=True,
            ):
                self.assertTrue(wheel.is_file())
                self.assertFalse(wheel.is_symlink())
                self.assertEqual(wheel.stat().st_nlink, 1)
                _name, _version, _build, filename_tags = parse_wheel_filename(
                    wheel.name
                )
                with zipfile.ZipFile(wheel) as archive:
                    wheel_members = tuple(
                        member
                        for member in archive.infolist()
                        if member.filename.endswith(".dist-info/WHEEL")
                    )
                    self.assertEqual(len(wheel_members), 1)
                    payload = archive.read(wheel_members[0])
                    record_payload = archive.read(EXACT_RECORD_MEMBER)
                self.assertEqual(len(payload), 147)
                self.assertEqual(payload, maturin_wheel_payload(compact))
                self.assertEqual(record_payload, maturin_record_payload(compact))
                self.assertEqual(
                    hashlib.sha256(payload).hexdigest(),
                    EXACT_WHEEL_PAYLOAD_SHA256[compact],
                )
                self.assertEqual(
                    {str(tag) for tag in filename_tags},
                    set(exact_wheel_tags(compact)),
                )

            def replace_exact_wheel_with_symlink(directory: Path) -> None:
                wheel = directory / exact_wheel_name("311")
                outside_target = directory.parent / "exact-cp311-symlink-target.whl"
                wheel.rename(outside_target)
                wheel.symlink_to(outside_target)

            mutations = {
                "missing-wheel": lambda path: next(path.glob("*cp311*.whl")).unlink(),
                "second-sdist": lambda path: (path / "eqiora-0.1.0a1.zip").write_bytes(
                    b"other sdist"
                ),
                "extra-file": lambda path: (path / "manifest.json").write_bytes(b"{}"),
                "directory": lambda path: (path / "nested").mkdir(),
                "symlink": replace_exact_wheel_with_symlink,
                "hard-link": lambda path: os.link(
                    next(path.glob("*cp311*.whl")),
                    path.parent / "hard-link-outside-family.whl",
                ),
                "fifth-wheel": lambda path: write_maturin_wheel(
                    path / exact_wheel_name("315"),
                    "315",
                ),
                "both-canonical-and-compressed": lambda path: (
                    path / "eqiora-0.1.0a1-cp311-cp311-manylinux_2_17_x86_64.whl"
                ).write_bytes((path / exact_wheel_name("311")).read_bytes()),
            }
            for name, mutate in mutations.items():
                with self.subTest(name=name):
                    mutant = root / name
                    self.write_exact_family(mutant)
                    mutate(mutant)
                    with self.assertRaises(RuntimeError):
                        executor.admit_candidate_family(mutant)

    def test_alpha2_retained_source_and_raw_frontend_mirrors_precede_node(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")

        class SourceBoundaryReached(RuntimeError):
            pass

        def cross_boundary(family: Path, output: Path) -> None:
            with (
                mock.patch.object(
                    executor,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    executor,
                    "_current_revision",
                    return_value=self.REVISION,
                ),
                mock.patch.object(executor, "checked_run", return_value="123456789"),
                mock.patch.object(
                    executor,
                    "create_isolated_build_workspaces",
                    side_effect=SourceBoundaryReached("source boundary reached"),
                ),
                mock.patch.object(
                    executor,
                    "_node_and_npm_identity",
                    side_effect=AssertionError("Node boundary ran too early"),
                ),
            ):
                with self.assertRaisesRegex(
                    SourceBoundaryReached, "source boundary reached"
                ):
                    executor.execute_h2(
                        expected_commit=self.REVISION,
                        artifacts=family,
                        out=output,
                    )
            self.assertEqual(tuple(output.iterdir()), ())

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "positive-family"
            output = root / "positive-output"
            self.write_source_derived_family(family)
            cross_boundary(family, output)

            metadata_family = root / "stale-wheel-metadata"
            metadata_output = root / "stale-wheel-metadata-output"
            self.write_source_derived_family(metadata_family)
            wheel = metadata_family / exact_wheel_name("311", version="0.1.0a2")
            with zipfile.ZipFile(wheel) as archive:
                members = tuple(
                    (
                        name,
                        (
                            archive.read(name).replace(
                                b"Version: 0.1.0a2", b"Version: 0.1.0a1"
                            )
                            if name.endswith(".dist-info/METADATA")
                            else archive.read(name)
                        ),
                    )
                    for name in archive.namelist()
                )
            write_maturin_wheel(wheel, "311", members=members)
            with (
                mock.patch.object(
                    executor,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    executor,
                    "_current_revision",
                    return_value=self.REVISION,
                ),
                mock.patch.object(executor, "safe_extract_sdist") as extract,
                mock.patch.object(executor, "_node_and_npm_identity") as node,
                mock.patch.object(executor, "write_canonical_receipt") as publish,
            ):
                with self.assertRaisesRegex(CandidateError, "distribution version"):
                    executor.execute_h2(
                        expected_commit=self.REVISION,
                        artifacts=metadata_family,
                        out=metadata_output,
                    )
            extract.assert_not_called()
            node.assert_not_called()
            publish.assert_not_called()
            self.assertFalse(metadata_output.exists())

            mutations: dict[str, dict[str, object]] = {
                "stale-cargo-family": {
                    "cargo_version": "0.1.0-alpha.1",
                    "package_version": "0.1.0-alpha.1",
                    "lock_version": "0.1.0-alpha.1",
                    "lock_root_version": "0.1.0-alpha.1",
                },
                "package-version": {"package_version": "0.1.0-alpha.1"},
                "lock-version": {"lock_version": "0.1.0-alpha.1"},
                "lock-root-version": {"lock_root_version": "0.1.0-alpha.1"},
                "missing-package-version": {"package_version": None},
                "non-string-package-version": {"package_version": 2},
                "missing-lock-version": {"lock_version": None},
                "non-string-lock-version": {"lock_version": 2},
                "missing-lock-root-version": {"lock_root_version": None},
                "non-string-lock-root-version": {"lock_root_version": False},
                "malformed-package": {"malformed_package": True},
                "malformed-lock": {"malformed_lock": True},
                "authored-python-version": {"authored_python_version": True},
                "unsupported-cargo-version": {
                    "cargo_version": "0.1.0-preview.2",
                    "package_version": "0.1.0-preview.2",
                    "lock_version": "0.1.0-preview.2",
                    "lock_root_version": "0.1.0-preview.2",
                },
            }
            for name, options in mutations.items():
                with self.subTest(name=name):
                    mutant = root / name
                    output = root / f"{name}-output"
                    self.write_source_derived_family(mutant, **options)
                    with (
                        mock.patch.object(
                            executor,
                            "source_identity",
                            return_value=SourceIdentity(self.REVISION, ()),
                        ),
                        mock.patch.object(
                            executor,
                            "_current_revision",
                            return_value=self.REVISION,
                        ),
                        mock.patch.object(
                            executor, "checked_run", return_value="123456789"
                        ),
                        mock.patch.object(
                            executor, "create_isolated_build_workspaces"
                        ) as later_work,
                        mock.patch.object(
                            executor, "_node_and_npm_identity"
                        ) as node,
                        mock.patch.object(
                            executor, "write_canonical_receipt"
                        ) as publish,
                    ):
                        with self.assertRaises(RuntimeError):
                            executor.execute_h2(
                                expected_commit=self.REVISION,
                                artifacts=mutant,
                                out=output,
                            )
                    later_work.assert_not_called()
                    node.assert_not_called()
                    publish.assert_not_called()
                    self.assertEqual(tuple(output.iterdir()), ())

    def test_h2_rejects_every_filename_and_internal_tag_widening_before_work(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        wheel_member = EXACT_WHEEL_MEMBER

        def rename_311(directory: Path, replacement: str) -> None:
            (directory / exact_wheel_name("311")).rename(directory / replacement)

        def retag_311(directory: Path, tags: tuple[str, ...]) -> None:
            write_maturin_wheel(
                directory / exact_wheel_name("311"),
                "311",
                tags=tags,
            )

        def remap_311_members(
            directory: Path,
            members: tuple[tuple[str, bytes], ...],
        ) -> None:
            write_maturin_wheel(
                directory / exact_wheel_name("311"),
                "311",
                members=members,
            )

        def make_311_wheel_member_mode(directory: Path, mode: int) -> None:
            wheel = directory / exact_wheel_name("311")
            wheel_entry = zipfile.ZipInfo(wheel_member)
            wheel_entry.create_system = 3
            wheel_entry.external_attr = mode << 16
            record_entry = zipfile.ZipInfo(EXACT_RECORD_MEMBER)
            record_entry.create_system = 3
            record_entry.external_attr = 0o100644 << 16
            with zipfile.ZipFile(wheel, mode="w") as archive:
                archive.writestr(wheel_entry, maturin_wheel_payload("311"))
                archive.writestr(record_entry, maturin_record_payload("311"))

        tag_pair = exact_wheel_tags("311")
        filename_mutations: dict[str, Callable[[Path], None]] = {
            "outer-rename-canonical-only-optional-alias": lambda path: rename_311(
                path,
                "eqiora-0.1.0a1-cp311-cp311-manylinux_2_17_x86_64.whl",
            ),
            "legacy-only": lambda path: rename_311(
                path,
                "eqiora-0.1.0a1-cp311-cp311-manylinux2014_x86_64.whl",
            ),
            "alias-first": lambda path: rename_311(
                path,
                "eqiora-0.1.0a1-cp311-cp311-"
                "manylinux2014_x86_64.manylinux_2_17_x86_64.whl",
            ),
            "broadened-extra-suffix": lambda path: rename_311(
                path,
                "eqiora-0.1.0a1-cp311-cp311-"
                f"{EXACT_PHYSICAL_PLATFORM}.manylinux_2_28_x86_64.whl",
            ),
            "other-manylinux-floor": lambda path: rename_311(
                path,
                "eqiora-0.1.0a1-cp311-cp311-"
                "manylinux_2_28_x86_64.manylinux2014_x86_64.whl",
            ),
            "other-architecture": lambda path: rename_311(
                path,
                "eqiora-0.1.0a1-cp311-cp311-"
                "manylinux_2_17_aarch64.manylinux2014_aarch64.whl",
            ),
            "wrong-version": lambda path: rename_311(
                path,
                exact_wheel_name("311", version="0.1.0a2"),
            ),
            "cross-interpreter-basename": lambda path: rename_311(
                path,
                f"eqiora-0.1.0a1-cp311-cp312-{EXACT_PHYSICAL_PLATFORM}.whl",
            ),
        }
        metadata_mutations: dict[str, Callable[[Path], None]] = {
            "no-internal-tags": lambda path: retag_311(path, ()),
            "canonical-internal-only": lambda path: retag_311(path, tag_pair[:1]),
            "legacy-internal-only": lambda path: retag_311(path, tag_pair[1:]),
            "internal-tags-reversed": lambda path: retag_311(
                path,
                tuple(reversed(tag_pair)),
            ),
            "internal-tag-duplicated": lambda path: retag_311(
                path,
                (tag_pair[0], tag_pair[1], tag_pair[1]),
            ),
            "internal-cross-interpreter": lambda path: retag_311(
                path,
                exact_wheel_tags("312"),
            ),
            "internal-wrong-abi": lambda path: retag_311(
                path,
                (
                    "cp311-abi3-manylinux_2_17_x86_64",
                    "cp311-abi3-manylinux2014_x86_64",
                ),
            ),
            "internal-wrong-architecture": lambda path: retag_311(
                path,
                (
                    "cp311-cp311-manylinux_2_17_aarch64",
                    "cp311-cp311-manylinux2014_aarch64",
                ),
            ),
            "internal-third-tag": lambda path: retag_311(
                path,
                (*tag_pair, "cp311-cp311-manylinux_2_28_x86_64"),
            ),
            "missing-wheel-metadata": lambda path: remap_311_members(
                path,
                (("eqiora-0.1.0a1.dist-info/METADATA", b"metadata\n"),),
            ),
            "duplicate-wheel-metadata": lambda path: remap_311_members(
                path,
                (
                    (wheel_member, maturin_wheel_payload("311")),
                    (wheel_member, maturin_wheel_payload("311")),
                ),
            ),
            "malformed-wheel-metadata": lambda path: remap_311_members(
                path,
                ((wheel_member, b"Wheel-Version: 1.0\nTag: malformed\n"),),
            ),
            "wrong-wheel-version": lambda path: remap_311_members(
                path,
                (
                    (
                        wheel_member,
                        maturin_wheel_payload("311").replace(
                            b"Wheel-Version: 1.0",
                            b"Wheel-Version: 1.1",
                        ),
                    ),
                ),
            ),
            "wrong-generator": lambda path: remap_311_members(
                path,
                (
                    (
                        wheel_member,
                        maturin_wheel_payload("311").replace(
                            b"maturin (1.14.1)",
                            b"maturin (1.14.2)",
                        ),
                    ),
                ),
            ),
            "purelib-wheel": lambda path: remap_311_members(
                path,
                (
                    (
                        wheel_member,
                        maturin_wheel_payload("311").replace(
                            b"Root-Is-Purelib: false",
                            b"Root-Is-Purelib: true",
                        ),
                    ),
                ),
            ),
            "ambiguous-wheel-metadata": lambda path: remap_311_members(
                path,
                (
                    (wheel_member, maturin_wheel_payload("311")),
                    (
                        "other-0.1.0a1.dist-info/WHEEL",
                        maturin_wheel_payload("311"),
                    ),
                ),
            ),
            "sole-wrong-distribution-wheel-owner": lambda path: remap_311_members(
                path,
                (
                    (
                        "other-0.1.0a1.dist-info/WHEEL",
                        maturin_wheel_payload("311"),
                    ),
                ),
            ),
            "sole-wrong-version-wheel-owner": lambda path: remap_311_members(
                path,
                (
                    (
                        "eqiora-0.1.0a2.dist-info/WHEEL",
                        maturin_wheel_payload("311"),
                    ),
                ),
            ),
            "wheel-metadata-symlink-mode": lambda path: make_311_wheel_member_mode(
                path,
                0o120777,
            ),
            "wheel-metadata-directory-mode": lambda path: make_311_wheel_member_mode(
                path,
                0o040755,
            ),
        }
        mutations = {**filename_mutations, **metadata_mutations}

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            for name, mutate in mutations.items():
                with self.subTest(mutant=name):
                    family = root / name / "family"
                    output = root / name / "h2-output"
                    self.write_exact_family(family)
                    output.mkdir()
                    mutate(family)
                    with (
                        mock.patch.object(
                            executor,
                            "source_identity",
                            return_value=SourceIdentity(self.REVISION, ()),
                        ),
                        mock.patch.object(
                            executor,
                            "safe_extract_sdist",
                        ) as extract,
                        mock.patch.object(
                            executor,
                            "stage_frontend",
                            create=True,
                        ) as stage,
                        mock.patch.object(
                            executor,
                            "run_frontend_commands",
                        ) as frontend,
                        mock.patch.object(
                            executor,
                            "observe_h2",
                            create=True,
                        ) as observe,
                        mock.patch.object(
                            executor,
                            "write_canonical_receipt",
                        ) as publish,
                    ):
                        with self.assertRaises(RuntimeError):
                            executor.execute_h2(
                                expected_commit=self.REVISION,
                                artifacts=family,
                                out=output,
                            )
                    extract.assert_not_called()
                    stage.assert_not_called()
                    frontend.assert_not_called()
                    observe.assert_not_called()
                    publish.assert_not_called()
                    self.assertEqual(tuple(output.iterdir()), ())

    def test_twine_and_installer_green_cannot_admit_outer_only_renames(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            family = Path(temporary) / "family"
            self.write_exact_family(family)
            for compact in EXACT_WHEEL_INTERPRETERS:
                compressed = family / exact_wheel_name(compact)
                canonical_only = family / (
                    f"eqiora-0.1.0a1-cp{compact}-cp{compact}-manylinux_2_17_x86_64.whl"
                )
                compressed.rename(canonical_only)
                _name, _version, _build, filename_tags = parse_wheel_filename(
                    canonical_only.name
                )
                with zipfile.ZipFile(canonical_only) as archive:
                    payload = archive.read("eqiora-0.1.0a1.dist-info/WHEEL")
                internal_tags = {
                    line.removeprefix("Tag: ")
                    for line in payload.decode("utf-8").splitlines()
                    if line.startswith("Tag: ")
                }
                self.assertEqual(
                    {str(tag) for tag in filename_tags},
                    {exact_wheel_tags(compact)[0]},
                )
                self.assertEqual(internal_tags, set(exact_wheel_tags(compact)))

            supplemental_consumers = {
                "twine-7.0.0-strict": True,
                "managed-interpreter-installations": True,
            }
            self.assertTrue(all(supplemental_consumers.values()))
            with self.assertRaises(RuntimeError):
                executor.admit_candidate_family(family)

    @staticmethod
    def write_exact_family(
        directory: Path,
        *,
        version: str = "0.1.0a1",
    ) -> None:
        directory.mkdir(parents=True)
        (directory / f"eqiora-{version}.tar.gz").write_bytes(b"sdist")
        for python in EXACT_WHEEL_INTERPRETERS:
            write_maturin_wheel(
                directory / exact_wheel_name(python, version=version),
                python,
            )

    @staticmethod
    def write_source_derived_family(
        directory: Path,
        *,
        cargo_version: str = "0.1.0-alpha.2",
        package_version: object = "0.1.0-alpha.2",
        lock_version: object = "0.1.0-alpha.2",
        lock_root_version: object = "0.1.0-alpha.2",
        malformed_package: bool = False,
        malformed_lock: bool = False,
        authored_python_version: bool = False,
    ) -> None:
        directory.mkdir(parents=True)
        normalized = "0.1.0a2"
        sdist = directory / f"eqiora-{normalized}.tar.gz"
        package: dict[str, object] = {"name": "frontend"}
        if package_version is not None:
            package["version"] = package_version
        package_bytes = (
            b"{"
            if malformed_package
            else json.dumps(package, sort_keys=True).encode("utf-8")
        )
        lock: dict[str, object] = {
            "lockfileVersion": 3,
            "packages": {"": {}},
        }
        if lock_version is not None:
            lock["version"] = lock_version
        if lock_root_version is not None:
            lock["packages"][""]["version"] = lock_root_version  # type: ignore[index]
        lock_bytes = (
            b"{"
            if malformed_lock
            else json.dumps(lock, sort_keys=True).encode("utf-8")
        )
        pyproject = (
            b'[project]\nname = "eqiora"\nversion = "0.1.0a2"\n'
            if authored_python_version
            else b'[project]\nname = "eqiora"\ndynamic = ["version"]\n'
        )
        members = {
            "eqiora-0.1.0a2/Cargo.toml": (
                f'[workspace.package]\nversion = "{cargo_version}"\n'.encode()
            ),
            "eqiora-0.1.0a2/Cargo.lock": b"# synthetic\n",
            "eqiora-0.1.0a2/pyproject.toml": pyproject,
            "eqiora-0.1.0a2/crates/eqiora-python/Cargo.toml": b"[package]\nname='eqiora-python'\n",
            "eqiora-0.1.0a2/bindings/python/frontend/package.json": package_bytes,
            "eqiora-0.1.0a2/bindings/python/frontend/package-lock.json": lock_bytes,
        }
        with tarfile.open(sdist, mode="w:gz") as archive:
            for name, payload in sorted(members.items()):
                member = tarfile.TarInfo(name)
                member.mode = 0o644
                member.mtime = 0
                member.size = len(payload)
                archive.addfile(member, io.BytesIO(payload))
        for python in EXACT_WHEEL_INTERPRETERS:
            write_maturin_wheel(
                directory / exact_wheel_name(python, version=normalized),
                python,
            )

    @staticmethod
    def expected_family_inventory(directory: Path) -> tuple[dict[str, object], ...]:
        return tuple(
            {
                "filename": path.name,
                "kind": "sdist" if path.name.endswith(".tar.gz") else "wheel",
                "size": path.stat().st_size,
                "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            }
            for path in sorted(directory.iterdir(), key=lambda item: item.name.encode())
        )

    @staticmethod
    def complete_candidate_profile_checks() -> tuple[str, ...]:
        base_checks = tuple(
            check
            for compact in EXACT_WHEEL_INTERPRETERS
            for check in (
                f"cp{compact}:installed-wheel",
                f"cp{compact}:base-and-numpy",
                f"cp{compact}:packaged-exact-cylinder-model-demo",
                f"cp{compact}:packaged-mixed-boundary-elasticity-demo",
                f"cp{compact}:packaged-fixed-reference-fsi-demo",
                f"cp{compact}:async-and-cancellation",
                f"cp{compact}:strict-base-typing",
                f"cp{compact}:public-smoke-base",
                f"cp{compact}:matplotlib-free-base",
            )
        )
        return (
            "twine-strict",
            "sdist-to-wheel-rebuild",
            *base_checks,
            "cp312:numpy-2.1.0-floor",
            "check:generated-public-api",
            *NOTEBOOK_PROFILE_CHECKS,
            "cp313:torch",
            "cp313:public-smoke-torch",
            "cp313:jax",
            "cp313:public-smoke-jax",
            "cp313:matplotlib",
            "cp313:packaged-exact-cylinder-pressure-demo",
            "cp313:packaged-mixed-boundary-displacement-demo",
            "cp313:packaged-fixed-reference-fsi-still",
            "cp313:complete-public-typing",
        )

    @classmethod
    def complete_candidate_profile_summary(
        cls,
        family: object,
        *,
        uv: str = "uv",
        checks: tuple[str, ...] | None = None,
        wheel_records: tuple[dict[str, object], ...] | None = None,
    ) -> object:
        config = python_candidate_module.load_config()
        if wheel_records is None:
            wheel_records = tuple(
                {
                    "filename": wheel.name,
                    "kind": "wheel",
                    "python": f"{compact[0]}.{compact[1:]}",
                    "abi": f"cp{compact}",
                    "platform": config.wheel_platform,
                    "size": wheel.stat().st_size,
                    "sha256": hashlib.sha256(wheel.read_bytes()).hexdigest(),
                }
                for compact, wheel in zip(
                    EXACT_WHEEL_INTERPRETERS,
                    family.wheels,
                    strict=True,
                )
            )
        numpy_version = config.numpy_floor.split("==", maxsplit=1)[1]
        return python_candidate_module.CandidateProfileSummary(
            config=config,
            uv=uv,
            wheel_records=wheel_records,
            checks=(
                cls.complete_candidate_profile_checks()
                if checks is None
                else checks
            ),
            dependency_profiles={
                "numpy_floor": {
                    "python": config.numpy_floor_interpreter,
                    "requirement": config.numpy_floor,
                    "observed": numpy_version,
                    "profile": (
                        "cp"
                        f"{config.numpy_floor_interpreter.replace('.', '')}:"
                        f"numpy-{numpy_version}-floor"
                    ),
                }
            },
        )

    def test_execute_h2_sequencing_uses_real_admission_and_inventory_unit(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "family"
            output = root / "h2-output"
            self.write_exact_family(family)
            expected_inventory = self.expected_family_inventory(family)
            output.mkdir()
            receipt = {"candidate": {"version": "0.1.0a1"}}
            receipt_path = output / "eqiora-0.1.0a1-python-candidate-h2.json"

            def extract(_archive: Path, destination: Path) -> Path:
                destination.mkdir(parents=True)
                return destination

            def publish(_receipt: dict, destination: Path) -> Path:
                self.assertEqual(destination, output)
                receipt_path.write_bytes(b"opaque execution-orchestration receipt")
                return receipt_path

            admission = executor.admit_candidate_family
            inventory = executor.family_inventory
            observed_inventories: list[tuple[dict[str, object], ...]] = []

            def observe_inventory(directory: Path) -> tuple[dict[str, object], ...]:
                observed_inventory = inventory(directory)
                observed_inventories.append(observed_inventory)
                return observed_inventory

            with (
                mock.patch.object(
                    executor,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    executor,
                    "admit_candidate_family",
                    wraps=admission,
                ) as admit_family,
                mock.patch.object(
                    executor,
                    "family_inventory",
                    side_effect=observe_inventory,
                ) as family_inventory,
                mock.patch.object(
                    executor,
                    "safe_extract_sdist",
                    side_effect=extract,
                ) as safe_extract,
                mock.patch.object(
                    executor,
                    "_retained_distribution_version",
                    return_value="0.1.0a1",
                ) as retained_version,
                mock.patch.object(
                    executor,
                    "stage_frontend",
                    create=True,
                ) as stage_frontend,
                mock.patch.object(
                    executor,
                    "run_frontend_commands",
                ) as run_frontend_commands,
                mock.patch.object(
                    executor,
                    "observe_h2",
                    return_value=receipt,
                    create=True,
                ) as observe_h2,
                mock.patch.object(
                    executor,
                    "write_canonical_receipt",
                    side_effect=publish,
                ) as write_receipt,
            ):
                observed = executor.execute_h2(
                    expected_commit=self.REVISION,
                    artifacts=family,
                    out=output,
                )

            self.assertEqual(observed, receipt_path)
            admit_family.assert_called_once_with(family)
            self.assertGreaterEqual(family_inventory.call_count, 2)
            self.assertTrue(
                all(call.args == (family,) for call in family_inventory.call_args_list)
            )
            self.assertTrue(observed_inventories)
            self.assertTrue(
                all(observed == expected_inventory for observed in observed_inventories)
            )
            self.assertEqual(safe_extract.call_count, 1)
            retained_version.assert_called_once()
            self.assertEqual(stage_frontend.call_count, 2)
            self.assertEqual(run_frontend_commands.call_count, 2)
            observe_h2.assert_called_once()
            write_receipt.assert_called_once_with(receipt, output)
            self.assertEqual(executor.family_inventory(family), expected_inventory)

    def test_h2_admission_to_entry_hash_drift_rejects_before_h2_work(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "family"
            output = root / "h2-output"
            self.write_exact_family(family)
            output.mkdir()
            real_admission = executor.admit_candidate_family

            def admit_then_mutate(directory: Path) -> object:
                admitted = real_admission(directory)
                wheel = directory / exact_wheel_name("311")
                wheel.write_bytes(wheel.read_bytes() + b"post-admission mutation")
                return admitted

            with (
                mock.patch.object(
                    executor,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    executor,
                    "admit_candidate_family",
                    side_effect=admit_then_mutate,
                ),
                mock.patch.object(executor, "safe_extract_sdist") as extract,
                mock.patch.object(
                    executor,
                    "stage_frontend",
                    create=True,
                ) as stage,
                mock.patch.object(executor, "run_frontend_commands") as frontend,
                mock.patch.object(
                    executor,
                    "observe_h2",
                    create=True,
                ) as observe,
                mock.patch.object(executor, "write_canonical_receipt") as publish,
            ):
                with self.assertRaises(RuntimeError):
                    executor.execute_h2(
                        expected_commit=self.REVISION,
                        artifacts=family,
                        out=output,
                    )

            extract.assert_not_called()
            stage.assert_not_called()
            frontend.assert_not_called()
            observe.assert_not_called()
            publish.assert_not_called()
            self.assertEqual(tuple(output.iterdir()), ())

    def test_h2_receipt_is_canonical_complete_or_absent(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        receipt = {
            "probe": {"verdict": "PASS"},
            "candidate": {"version": "0.1.0a1"},
        }
        expected_bytes = json.dumps(
            receipt,
            ensure_ascii=False,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            output = Path(temporary) / "h2-output"
            output.mkdir()
            expected_path = output / "eqiora-0.1.0a1-python-candidate-h2.json"
            with mock.patch.object(executor, "validate_h2_receipt") as validate:
                observed = executor.write_canonical_receipt(receipt, output)

            self.assertEqual(observed, expected_path)
            validate.assert_called_once_with(receipt)
            self.assertEqual(expected_path.read_bytes(), expected_bytes)
            self.assertEqual(tuple(output.iterdir()), (expected_path,))

        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            output = Path(temporary) / "not-empty"
            output.mkdir()
            partial = output / "partial.json"
            partial.write_bytes(b"partial")
            with mock.patch.object(
                executor,
                "validate_h2_receipt",
            ) as validate:
                with self.assertRaisesRegex(RuntimeError, "empty|output"):
                    executor.write_canonical_receipt(receipt, output)
            validate.assert_not_called()
            self.assertEqual(tuple(output.iterdir()), (partial,))

    def test_two_h2_workspaces_have_disjoint_home_cache_and_output_paths(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            scratch = Path(temporary)
            workspaces = executor.create_isolated_build_workspaces(scratch)

        self.assertEqual(len(workspaces), 2)
        owned_names = (
            "home",
            "npm_cache",
            "temporary",
            "installation",
            "output",
            "browser_cache",
        )
        all_paths: list[Path] = []
        for workspace in workspaces:
            root = Path(workspace.root)
            self.assertTrue(root.is_relative_to(scratch))
            owned = [Path(getattr(workspace, name)) for name in owned_names]
            self.assertTrue(all(path.is_relative_to(root) for path in owned))
            self.assertEqual(len(set(owned)), len(owned))
            all_paths.extend(owned)
        self.assertEqual(len(set(all_paths)), len(all_paths))

    def _superseded_synthetic_omitted_host_unit(
        self,
    ) -> None:
        executor = importlib.import_module("python_candidate_h2")
        notebook_checks = (
            "frontend:lock-integrity",
            "frontend:license-inventory",
            "frontend:bundle-byte-rebuild",
            "wheel-family:notebook-metadata",
            "cp313:notebook-anywidget-0.11.0",
            "cp313:marimo-0.23.16-exact-cylinder-stokes",
            "cp313:notebook-managed-chromium-r1234",
            "cp313:notebook-no-external-network",
            "cp313:notebook-cleanup-and-mutation",
        )
        dependent = notebook_checks[6:]
        for omitted in notebook_checks[5:6]:
            with (
                self.subTest(omitted_host=omitted),
                tempfile.TemporaryDirectory(dir=Path.home()) as temporary,
            ):
                root = Path(temporary)
                family_path = root / "family"
                metadata = root / "metadata"
                self.write_exact_family(family_path)
                family = executor.admit_candidate_family(family_path)
                receipt_path, _ = self.write_valid_h2_receipt(root, family)
                forged = python_candidate_module.CandidateProfileSummary(
                    config=python_candidate_module.load_config(),
                    uv="/reviewed/uv",
                    wheel_records=(),
                    checks=(
                        "twine-strict",
                        "sdist-to-wheel-rebuild",
                        *(name for name in notebook_checks if name != omitted),
                    ),
                    dependency_profiles={},
                )

                def write_forged_manifest(
                    *_args: object, **_kwargs: object
                ) -> Path:
                    path = metadata / "eqiora-0.1.0a1-python-candidate.json"
                    path.write_bytes(b"forged incomplete profile manifest")
                    return path

                with (
                    mock.patch.object(
                        python_candidate_module,
                        "source_identity",
                        return_value=SourceIdentity(self.REVISION, ()),
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "validate_h2_receipt",
                        wraps=python_candidate_module.validate_h2_receipt,
                    ) as validate,
                    mock.patch.object(
                        python_candidate_module,
                        "derive_frontend_manifest",
                        return_value={"h2_receipt_sha256": "0" * 64},
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "run_candidate_profiles",
                        return_value=forged,
                    ) as profiles,
                    mock.patch.object(
                        python_candidate_module,
                        "write_manifest",
                        side_effect=write_forged_manifest,
                    ) as write_manifest,
                    mock.patch.object(
                        python_candidate_module,
                        "load_candidate_family",
                        return_value=mock.sentinel.candidate,
                    ),
                    mock.patch.object(
                        python_candidate_module,
                        "verify_artifacts",
                    ),
                ):
                    with self.assertRaises(CandidateError):
                        python_candidate_module.finalize_candidate(
                            expected_commit=self.REVISION,
                            artifacts=family_path,
                            h2_receipt=receipt_path,
                            manifest_out=metadata,
                        )

                validate.assert_called_once()
                profiles.assert_called_once()
                write_manifest.assert_not_called()
                self.assertNotIn(omitted, forged.checks)
                self.assertTrue(all(name in forged.checks for name in dependent))
                if metadata.exists():
                    self.assertEqual(tuple(metadata.iterdir()), ())

    def _superseded_synthetic_finalizer_bypass_unit(self) -> None:
        executor = importlib.import_module("python_candidate_h2")
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family_path = root / "family"
            metadata = root / "metadata"
            self.write_exact_family(family_path)
            family = executor.admit_candidate_family(family_path)
            receipt_path, _ = self.write_valid_h2_receipt(root, family)

            def write_false_success(*_args: object, **_kwargs: object) -> Path:
                path = metadata / "eqiora-0.1.0a1-python-candidate.json"
                path.write_bytes(b"forged bypass manifest")
                return path

            with (
                mock.patch.object(
                    python_candidate_module,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "validate_h2_receipt",
                    wraps=python_candidate_module.validate_h2_receipt,
                ) as validate,
                mock.patch.object(
                    python_candidate_module,
                    "derive_frontend_manifest",
                    return_value={"h2_receipt_sha256": "0" * 64},
                ),
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    return_value=mock.sentinel.false_success,
                ) as profiles,
                mock.patch.object(
                    python_candidate_module,
                    "write_manifest",
                    side_effect=write_false_success,
                ) as write_manifest,
                mock.patch.object(
                    python_candidate_module,
                    "load_candidate_family",
                    return_value=mock.sentinel.candidate,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "verify_artifacts",
                ),
            ):
                with self.assertRaises(CandidateError):
                    python_candidate_module.finalize_candidate(
                        expected_commit=self.REVISION,
                        artifacts=family_path,
                        h2_receipt=receipt_path,
                        manifest_out=metadata,
                    )

            validate.assert_called_once()
            profiles.assert_called_once()
            write_manifest.assert_not_called()
            if metadata.exists():
                self.assertEqual(tuple(metadata.iterdir()), ())

    def test_finalizer_admits_complete_profile_summary_before_publication(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            candidate_version = python_candidate_module.load_config().python_version
            family_path = root / "family"
            receipt_path = root / f"eqiora-{candidate_version}-python-candidate-h2.json"
            self.write_exact_family(family_path, version=candidate_version)
            receipt_bytes = json.dumps(
                {"candidate": {"version": candidate_version}},
                sort_keys=True,
                separators=(",", ":"),
            ).encode("utf-8")
            receipt_path.write_bytes(receipt_bytes)
            executor = importlib.import_module("python_candidate_h2")
            family = executor.admit_candidate_family(family_path)
            entry_inventory = executor.family_inventory(family_path)
            complete = self.complete_candidate_profile_summary(family)
            expected_manifest_name = f"eqiora-{candidate_version}-python-candidate.json"

            positive_metadata = root / "positive-metadata"
            positive_manifest = positive_metadata / expected_manifest_name

            def write_positive_manifest(
                *_args: object, **_kwargs: object
            ) -> Path:
                positive_manifest.write_bytes(b"complete profile manifest")
                return positive_manifest

            with (
                mock.patch.object(
                    python_candidate_module,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "validate_h2_receipt",
                    return_value={"validated": True},
                ),
                mock.patch.object(
                    python_candidate_module,
                    "derive_frontend_manifest",
                    return_value={"h2_receipt_sha256": "0" * 64},
                ),
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    return_value=complete,
                ) as profiles,
                mock.patch.object(
                    python_candidate_module,
                    "write_manifest",
                    side_effect=write_positive_manifest,
                ) as write_manifest,
                mock.patch.object(
                    python_candidate_module,
                    "load_candidate_family",
                    return_value=mock.sentinel.candidate,
                ) as load_family,
                mock.patch.object(
                    python_candidate_module,
                    "verify_artifacts",
                ) as verify_artifacts,
            ):
                observed = python_candidate_module.finalize_candidate(
                    expected_commit=self.REVISION,
                    artifacts=family_path,
                    h2_receipt=receipt_path,
                    manifest_out=positive_metadata,
                )

            self.assertEqual(observed, positive_manifest)
            profiles.assert_called_once()
            write_manifest.assert_called_once()
            load_family.assert_called_once()
            verify_artifacts.assert_called_once_with(
                mock.sentinel.candidate,
                family_path,
            )
            self.assertEqual(
                {path.name for path in positive_metadata.iterdir()},
                {expected_manifest_name, receipt_path.name},
            )
            self.assertEqual(
                (positive_metadata / receipt_path.name).read_bytes(),
                receipt_bytes,
            )
            self.assertEqual(executor.family_inventory(family_path), entry_inventory)
            self.assertEqual(receipt_path.read_bytes(), receipt_bytes)

            invalid_summaries = (
                *(
                    (
                        f"omitted-{host}",
                        self.complete_candidate_profile_summary(
                            family,
                            uv="/reviewed/uv",
                            checks=tuple(
                                check
                                for check in complete.checks
                                if check != host
                            ),
                        ),
                        (
                            "candidate profile summary does not contain the exact "
                            "complete check set"
                        ),
                    )
                    for host in NOTEBOOK_PROFILE_CHECKS[5:7]
                ),
                (
                    "non-summary-false-success",
                    mock.sentinel.false_success,
                    "candidate profile owner did not return CandidateProfileSummary",
                ),
                (
                    "omitted-wheel-record",
                    self.complete_candidate_profile_summary(
                        family,
                        uv="/reviewed/uv",
                        wheel_records=complete.wheel_records[:-1],
                    ),
                    (
                        "candidate profile summary does not bind the exact "
                        "four-wheel family"
                    ),
                ),
            )

            def forbidden_manifest_write(
                *_args: object, **_kwargs: object
            ) -> Path:
                raise AssertionError(
                    "write_manifest reached before profile-summary rejection"
                )

            for name, invalid, message in invalid_summaries:
                with self.subTest(profile_summary_mutant=name):
                    metadata = root / name
                    with (
                        mock.patch.object(
                            python_candidate_module,
                            "source_identity",
                            return_value=SourceIdentity(self.REVISION, ()),
                        ),
                        mock.patch.object(
                            python_candidate_module,
                            "validate_h2_receipt",
                            return_value={"validated": True},
                        ),
                        mock.patch.object(
                            python_candidate_module,
                            "derive_frontend_manifest",
                            return_value={"h2_receipt_sha256": "0" * 64},
                        ),
                        mock.patch.object(
                            python_candidate_module,
                            "run_candidate_profiles",
                            return_value=invalid,
                        ) as profiles,
                        mock.patch.object(
                            python_candidate_module,
                            "write_manifest",
                            side_effect=forbidden_manifest_write,
                        ) as write_manifest,
                        mock.patch.object(
                            python_candidate_module,
                            "tool_version",
                        ) as tool_version,
                        mock.patch.object(
                            python_candidate_module,
                            "load_candidate_family",
                        ) as load_family,
                        mock.patch.object(
                            python_candidate_module,
                            "verify_artifacts",
                        ) as verify_artifacts,
                    ):
                        with self.assertRaisesRegex(
                            CandidateError,
                            rf"\A{message}\Z",
                        ):
                            python_candidate_module.finalize_candidate(
                                expected_commit=self.REVISION,
                                artifacts=family_path,
                                h2_receipt=receipt_path,
                                manifest_out=metadata,
                            )

                    profiles.assert_called_once()
                    write_manifest.assert_not_called()
                    tool_version.assert_not_called()
                    load_family.assert_not_called()
                    verify_artifacts.assert_not_called()
                    if metadata.exists():
                        self.assertEqual(tuple(metadata.iterdir()), ())
                    self.assertEqual(
                        executor.family_inventory(family_path),
                        entry_inventory,
                    )
                    self.assertEqual(receipt_path.read_bytes(), receipt_bytes)

    def test_finalizer_consumes_receipt_and_never_rebuilds_or_synthesizes_it(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            candidate_version = python_candidate_module.load_config().python_version
            family = root / "family"
            metadata = root / "metadata"
            receipt = root / f"eqiora-{candidate_version}-python-candidate-h2.json"
            self.write_exact_family(family, version=candidate_version)
            receipt_bytes = json.dumps(
                {"candidate": {"version": candidate_version}},
                sort_keys=True,
                separators=(",", ":"),
            ).encode("utf-8")
            receipt.write_bytes(receipt_bytes)
            manifest = metadata / f"eqiora-{candidate_version}-python-candidate.json"
            candidate = mock.sentinel.candidate
            finalizer_entry_identities = {
                compact: wheel_byte_identity(
                    family / exact_wheel_name(compact, version=candidate_version),
                    version=candidate_version,
                )
                for compact in EXACT_WHEEL_INTERPRETERS
            }
            sealed_wheel_paths = {
                (
                    family / exact_wheel_name(compact, version=candidate_version)
                ).resolve()
                for compact in EXACT_WHEEL_INTERPRETERS
            }
            executor = importlib.import_module("python_candidate_h2")
            profile_summary = self.complete_candidate_profile_summary(
                executor.admit_candidate_family(family)
            )

            def write_manifest(*_args: object, **_kwargs: object) -> Path:
                self.assertTrue(metadata.is_dir())
                manifest.write_bytes(b"opaque finalized manifest")
                return manifest

            inventory = self.expected_family_inventory(family)
            observed_inventories: list[tuple[dict[str, object], ...]] = []

            def observe_inventory(directory: Path) -> tuple[dict[str, object], ...]:
                observed_inventory = executor.family_inventory(directory)
                observed_inventories.append(observed_inventory)
                return observed_inventory

            with (
                reject_post_producer_wheel_writes(sealed_wheel_paths),
                mock.patch.object(
                    python_candidate_module,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "admit_candidate_family",
                    wraps=executor.admit_candidate_family,
                    create=True,
                ) as admit_family,
                mock.patch.object(
                    python_candidate_module,
                    "family_inventory",
                    side_effect=observe_inventory,
                    create=True,
                ) as family_inventory,
                mock.patch.object(
                    python_candidate_module,
                    "validate_h2_receipt",
                    return_value=mock.sentinel.validated_receipt,
                    create=True,
                ) as validate_receipt,
                mock.patch.object(
                    python_candidate_module,
                    "derive_frontend_manifest",
                    return_value=mock.sentinel.frontend,
                    create=True,
                ) as derive_frontend,
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    return_value=profile_summary,
                    create=True,
                ) as run_profiles,
                mock.patch.object(
                    python_candidate_module,
                    "write_manifest",
                    side_effect=write_manifest,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "load_candidate_family",
                    return_value=candidate,
                    create=True,
                ) as load_family,
                mock.patch.object(
                    python_candidate_module,
                    "verify_artifacts",
                    create=True,
                ) as verify_artifacts,
                mock.patch.object(
                    python_candidate_module,
                    "build_artifacts",
                ) as rebuild_family,
                mock.patch.object(
                    python_candidate_module,
                    "build_h2_receipt",
                    create=True,
                ) as synthesize_receipt,
                mock.patch.object(
                    python_candidate_module,
                    "write_canonical_receipt",
                    create=True,
                ) as rewrite_receipt,
                mock.patch.object(
                    python_candidate_module,
                    "run_frontend_commands",
                    create=True,
                ) as rebuild_frontend,
            ):
                observed = python_candidate_module.finalize_candidate(
                    expected_commit=self.REVISION,
                    artifacts=family,
                    h2_receipt=receipt,
                    manifest_out=metadata,
                )
                finalizer_exit_identities = {
                    compact: wheel_byte_identity(
                        family
                        / exact_wheel_name(
                            compact,
                            version=candidate_version,
                        ),
                        version=candidate_version,
                    )
                    for compact in EXACT_WHEEL_INTERPRETERS
                }

            self.assertEqual(observed, manifest)
            self.assertEqual(finalizer_entry_identities, finalizer_exit_identities)
            admit_family.assert_called_once_with(family)
            self.assertGreaterEqual(family_inventory.call_count, 2)
            self.assertTrue(
                all(observed == inventory for observed in observed_inventories)
            )
            validate_receipt.assert_called_once()
            derive_frontend.assert_called_once()
            run_profiles.assert_called_once()
            load_family.assert_called_once()
            selected_artifacts = (
                load_family.call_args.args[1]
                if len(load_family.call_args.args) > 1
                else load_family.call_args.kwargs["artifacts"]
            )
            self.assertEqual(selected_artifacts, family)
            verify_artifacts.assert_called_once_with(candidate, family)
            rebuild_family.assert_not_called()
            synthesize_receipt.assert_not_called()
            rewrite_receipt.assert_not_called()
            rebuild_frontend.assert_not_called()
            retained_receipt = metadata / receipt.name
            self.assertEqual(retained_receipt.read_bytes(), receipt_bytes)
            self.assertEqual(receipt.read_bytes(), receipt_bytes)
            self.assertEqual(
                {path.name for path in metadata.iterdir()},
                {manifest.name, receipt.name},
            )
            self.assertEqual(
                tuple(
                    path.name
                    for path in sorted(
                        family.glob("*.whl"),
                        key=lambda item: item.name.encode(),
                    )
                ),
                tuple(
                    exact_wheel_name(python, version=candidate_version)
                    for python in EXACT_WHEEL_INTERPRETERS
                ),
            )

    def test_finalizer_hash_drift_leaves_no_manifest_or_retained_receipt(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "family"
            metadata = root / "metadata"
            receipt = root / "eqiora-0.1.0a1-python-candidate-h2.json"
            self.write_exact_family(family)
            receipt.write_bytes(b'{"candidate":{"version":"0.1.0a1"}}')
            executor = importlib.import_module("python_candidate_h2")

            def mutate_during_profiles(*_args: object, **_kwargs: object) -> object:
                wheel = family / exact_wheel_name("311")
                wheel.write_bytes(wheel.read_bytes() + b"finalizer mutation")
                return mock.sentinel.profiles

            with (
                mock.patch.object(
                    python_candidate_module,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "admit_candidate_family",
                    wraps=executor.admit_candidate_family,
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "family_inventory",
                    wraps=executor.family_inventory,
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "validate_h2_receipt",
                    return_value=mock.sentinel.validated_receipt,
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "derive_frontend_manifest",
                    return_value=mock.sentinel.frontend,
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    side_effect=mutate_during_profiles,
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "load_candidate_family",
                    return_value=mock.sentinel.candidate,
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "verify_artifacts",
                    create=True,
                ),
                mock.patch.object(
                    python_candidate_module,
                    "write_manifest",
                ) as write_manifest,
            ):
                with self.assertRaises((CandidateError, RuntimeError)):
                    python_candidate_module.finalize_candidate(
                        expected_commit=self.REVISION,
                        artifacts=family,
                        h2_receipt=receipt,
                        manifest_out=metadata,
                    )

            write_manifest.assert_not_called()
            self.assertEqual(
                receipt.read_bytes(), b'{"candidate":{"version":"0.1.0a1"}}'
            )
            if metadata.exists():
                self.assertEqual(tuple(metadata.iterdir()), ())

    def test_finalizer_rejects_canonical_only_family_before_profiles(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            family = root / "family"
            metadata = root / "metadata"
            receipt = root / "eqiora-0.1.0a1-python-candidate-h2.json"
            self.write_exact_family(family)
            receipt.write_bytes(b'{"candidate":{"version":"0.1.0a1"}}')
            for compact in EXACT_WHEEL_INTERPRETERS:
                (family / exact_wheel_name(compact)).rename(
                    family
                    / (
                        f"eqiora-0.1.0a1-cp{compact}-cp{compact}-"
                        "manylinux_2_17_x86_64.whl"
                    )
                )

            executor = importlib.import_module("python_candidate_h2")
            with (
                mock.patch.object(
                    python_candidate_module,
                    "source_identity",
                    return_value=SourceIdentity(self.REVISION, ()),
                ),
                mock.patch.object(
                    python_candidate_module,
                    "admit_candidate_family",
                    wraps=executor.admit_candidate_family,
                    create=True,
                ) as admit,
                mock.patch.object(
                    python_candidate_module,
                    "validate_h2_receipt",
                    create=True,
                ) as validate,
                mock.patch.object(
                    python_candidate_module,
                    "derive_frontend_manifest",
                    return_value=mock.sentinel.frontend,
                    create=True,
                ) as derive_frontend,
                mock.patch.object(
                    python_candidate_module,
                    "load_candidate_family",
                    return_value=mock.sentinel.candidate,
                    create=True,
                ) as load,
                mock.patch.object(
                    python_candidate_module,
                    "verify_artifacts",
                    create=True,
                ) as verify_artifacts,
                mock.patch.object(
                    python_candidate_module,
                    "run_candidate_profiles",
                    create=True,
                ) as profiles,
                mock.patch.object(
                    python_candidate_module,
                    "write_manifest",
                ) as write_manifest,
            ):
                with self.assertRaises((CandidateError, RuntimeError)):
                    python_candidate_module.finalize_candidate(
                        expected_commit=self.REVISION,
                        artifacts=family,
                        h2_receipt=receipt,
                        manifest_out=metadata,
                    )

            admit.assert_called_once_with(family)
            validate.assert_not_called()
            derive_frontend.assert_not_called()
            load.assert_not_called()
            verify_artifacts.assert_not_called()
            profiles.assert_not_called()
            write_manifest.assert_not_called()
            if metadata.exists():
                self.assertEqual(tuple(metadata.iterdir()), ())

    def test_notebook_checks_are_emitted_only_after_required_observations(
        self,
    ) -> None:
        profiles = importlib.import_module("python_candidate_profiles")
        check_names = (
            "frontend:lock-integrity",
            "frontend:license-inventory",
            "frontend:bundle-byte-rebuild",
            "wheel-family:notebook-metadata",
            "cp313:notebook-anywidget-0.11.0",
            "cp313:marimo-0.23.16-exact-cylinder-stokes",
            "cp313:notebook-managed-chromium-r1234",
            "cp313:notebook-no-external-network",
            "cp313:notebook-cleanup-and-mutation",
        )
        events: list[tuple[str, str]] = []
        observations = tuple(
            (
                name,
                lambda name=name: events.append(("observe", name)),
            )
            for name in check_names
        )

        observed = profiles.run_notebook_profile(
            observations,
            emit=lambda name: events.append(("emit", name)),
        )

        self.assertEqual(observed, check_names)
        self.assertEqual(
            events,
            [
                event
                for name in check_names
                for event in (("observe", name), ("emit", name))
            ],
        )

        for failure_index, failed_name in enumerate(check_names):
            with self.subTest(failed_observation=failed_name):
                executed: list[str] = []
                emitted: list[str] = []

                def observe(name: str) -> None:
                    executed.append(name)
                    if name == failed_name:
                        raise RuntimeError(f"forced observation failure: {name}")

                failing_observations = tuple(
                    (name, lambda name=name: observe(name)) for name in check_names
                )
                with self.assertRaisesRegex(RuntimeError, "forced observation failure"):
                    profiles.run_notebook_profile(
                        failing_observations,
                        emit=emitted.append,
                    )
                self.assertEqual(executed, list(check_names[: failure_index + 1]))
                self.assertEqual(emitted, list(check_names[:failure_index]))


class NotebookOwnedProcessDecisionTests(unittest.TestCase):
    """Independent outcome oracle for the bounded #495 cleanup decision.

    The private decision seam consumes only observations fixed by the accepted
    Issue contract. Process discovery, stable handles, and signalling remain
    implementation choices. The registered installed-wheel profile, not these
    synthetic inputs, owns the real marimo positive.
    """

    SCENARIO = "marimo-0.23.16"
    DECISION_PARAMETERS = (
        "scenario",
        "primary_error",
        "forced_escalation",
        "observation",
        "survivors",
        "diagnostic",
        "cleanup_started",
        "observed_at",
    )

    @staticmethod
    def survivor(
        *,
        role: str = "kernel",
        pid: int = 4102,
        start_time: int = 908_172,
        state: str = "sleeping",
        authority_denied: bool = False,
    ) -> dict[str, object]:
        return {
            "scenario": NotebookOwnedProcessDecisionTests.SCENARIO,
            "role": role,
            "pid": pid,
            "start_time": start_time,
            "state": state,
            "requested_stages": ("shutdown", "sigterm", "sigkill"),
            "stage_results": (
                "shutdown=acknowledged",
                "sigterm=sent",
                "sigkill=not-required",
            ),
            "authority_denied": authority_denied,
        }

    def decision(self) -> Callable[..., None]:
        decision = getattr(
            python_candidate_module,
            "_notebook_cleanup_decision",
            None,
        )
        if decision is None:
            self.skipTest("precommitted #495 private decision seam is absent")
        self.assertTrue(callable(decision))
        return decision

    def invoke(
        self,
        *,
        primary_error: BaseException | None = None,
        forced_escalation: bool = False,
        observation: str = "complete-empty",
        survivors: tuple[dict[str, object], ...] = (),
        diagnostic: str | None = None,
        cleanup_started: float = 100.0,
        observed_at: float = 134.999,
    ) -> None:
        self.decision()(
            scenario=self.SCENARIO,
            primary_error=primary_error,
            forced_escalation=forced_escalation,
            observation=observation,
            survivors=survivors,
            diagnostic=diagnostic,
            cleanup_started=cleanup_started,
            observed_at=observed_at,
        )

    def test_00_ordinary_complete_empty_terminal_is_the_only_success(self) -> None:
        decision = getattr(
            python_candidate_module,
            "_notebook_cleanup_decision",
            None,
        )
        self.assertTrue(
            callable(decision),
            "#495 requires the precommitted private cleanup-decision seam",
        )
        self.assertEqual(
            tuple(inspect.signature(decision).parameters),
            self.DECISION_PARAMETERS,
        )
        self.assertEqual(
            python_candidate_module._NOTEBOOK_CLEANUP_GRACE_SECONDS,
            30.0,
        )
        self.assertEqual(
            python_candidate_module._NOTEBOOK_CLEANUP_DECISION_SECONDS,
            35.0,
        )
        self.assertEqual(
            python_candidate_module._NOTEBOOK_CLEANUP_IDENTITY_LIMIT,
            256,
        )
        self.assertEqual(
            python_candidate_module._NOTEBOOK_CLEANUP_DIAGNOSTIC_BYTES,
            65_536,
        )

        self.invoke()

    def test_host_exit_with_owned_child_is_not_empty_success(self) -> None:
        child = self.survivor(role="browser-helper", pid=5103, start_time=77)
        with self.assertRaises(CandidateError) as raised:
            self.invoke(observation="complete-nonempty", survivors=(child,))

        diagnostic = str(raised.exception)
        self.assertIn("complete-nonempty", diagnostic)
        self.assertIn("role=browser-helper", diagnostic)
        self.assertIn("pid=5103", diagnostic)
        self.assertIn("start=77", diagnostic)

    def test_primary_failure_still_has_a_cleanup_terminal(self) -> None:
        primary = RuntimeError("host-payload-failed")
        self.assertRaises(CandidateError, self.invoke, primary_error=primary)

        survivor = self.survivor(pid=6104, start_time=88)
        with self.assertRaises(CandidateError) as raised:
            self.invoke(
                primary_error=primary,
                observation="complete-nonempty",
                survivors=(survivor,),
            )

        diagnostic = str(raised.exception)
        self.assertIn("primary=RuntimeError: host-payload-failed", diagnostic)
        self.assertIn("cleanup=complete-nonempty", diagnostic)
        self.assertIn("pid=6104", diagnostic)
        self.assertIs(raised.exception.__cause__, primary)

    def test_forced_escalation_rejects_even_after_complete_empty(self) -> None:
        with self.assertRaisesRegex(CandidateError, "forced-escalation"):
            self.invoke(forced_escalation=True)

    def test_absolute_decision_deadline_rejects_nonempty_and_incomplete(self) -> None:
        survivor = self.survivor(pid=7105, start_time=99)
        for observation, survivors in (
            ("complete-empty", ()),
            ("complete-nonempty", (survivor,)),
            ("incomplete(observer-unavailable)", ()),
        ):
            with self.subTest(observation=observation):
                with self.assertRaises(CandidateError) as raised:
                    self.invoke(
                        observation=observation,
                        survivors=survivors,
                        observed_at=135.0,
                    )
                diagnostic = str(raised.exception)
                self.assertIn("cleanup-deadline", diagnostic)
                self.assertIn(observation, diagnostic)
                self.assertLessEqual(len(diagnostic.encode("utf-8")), 65_536)

    def test_stable_identity_distinguishes_pid_reuse_and_foreign_roles(self) -> None:
        stale = self.survivor(role="kernel", pid=8106, start_time=100)
        replacement = self.survivor(
            role="foreign",
            pid=8106,
            start_time=101,
        )
        with self.assertRaises(CandidateError) as first:
            self.invoke(
                observation="complete-nonempty",
                survivors=(replacement, stale),
            )
        with self.assertRaises(CandidateError) as second:
            self.invoke(
                observation="complete-nonempty",
                survivors=(stale, replacement),
            )

        first_diagnostic = str(first.exception)
        self.assertEqual(first_diagnostic, str(second.exception))
        self.assertIn("role=kernel", first_diagnostic)
        self.assertIn("role=foreign", first_diagnostic)
        self.assertIn("pid=8106", first_diagnostic)
        self.assertIn("start=100", first_diagnostic)
        self.assertIn("start=101", first_diagnostic)

    def test_cleanup_diagnostic_never_hides_primary_or_stable_identity(self) -> None:
        primary = subprocess.CalledProcessError(9, ("npm", "run", "test:hosts"))
        survivor = self.survivor(
            role="unknown",
            pid=9107,
            start_time=111,
            state="inaccessible",
            authority_denied=True,
        )
        with self.assertRaises(CandidateError) as raised:
            self.invoke(
                primary_error=primary,
                observation="incomplete(authority-denied)",
                survivors=(survivor,),
            )

        diagnostic = str(raised.exception)
        self.assertIn("primary=CalledProcessError", diagnostic)
        self.assertIn("cleanup=incomplete(authority-denied)", diagnostic)
        self.assertIn("role=unknown", diagnostic)
        self.assertIn("pid=9107", diagnostic)
        self.assertIn("start=111", diagnostic)
        self.assertIn("state=inaccessible", diagnostic)
        self.assertIn(f"scenario={self.SCENARIO}", diagnostic)
        self.assertIn("requested_stages=shutdown,sigterm,sigkill", diagnostic)
        self.assertIn("shutdown=acknowledged", diagnostic)
        self.assertIn("sigterm=sent", diagnostic)
        self.assertIn("sigkill=not-required", diagnostic)
        self.assertIn("authority_denied=true", diagnostic)
        self.assertIs(raised.exception.__cause__, primary)

    def test_identity_and_diagnostic_overflow_are_incomplete_rejections(self) -> None:
        self.decision()
        admitted = tuple(
            self.survivor(pid=10_000 + index, start_time=20_000 + index)
            for index in range(256)
        )
        admitted_diagnostics: list[str] = []
        for ordered in (admitted, tuple(reversed(admitted))):
            with self.subTest(
                identity_count=len(ordered), reversed=ordered != admitted
            ):
                with self.assertRaises(CandidateError) as within_identity_limit:
                    self.invoke(
                        observation="complete-nonempty",
                        survivors=ordered,
                    )
                admitted_diagnostic = str(within_identity_limit.exception)
                self.assertIn("complete-nonempty", admitted_diagnostic)
                self.assertNotIn(
                    "incomplete(observation-overflow)",
                    admitted_diagnostic,
                )
                admitted_diagnostics.append(admitted_diagnostic)
        self.assertEqual(admitted_diagnostics[0], admitted_diagnostics[1])

        identities = tuple(
            self.survivor(pid=10_000 + index, start_time=20_000 + index)
            for index in range(257)
        )
        identity_diagnostics: list[str] = []
        for ordered in (identities, tuple(reversed(identities))):
            with self.assertRaises(CandidateError) as identity_overflow:
                self.invoke(
                    observation="complete-nonempty",
                    survivors=ordered,
                    diagnostic="cleanup=complete-nonempty",
                )
            identity_diagnostics.append(str(identity_overflow.exception))
        self.assertEqual(identity_diagnostics[0], identity_diagnostics[1])
        self.assertIn(
            "incomplete(observation-overflow)",
            identity_diagnostics[0],
        )
        self.assertLessEqual(
            len(identity_diagnostics[0].encode("utf-8")),
            65_536,
        )

        prefix = (
            f"cleanup=complete-nonempty\nscenario={self.SCENARIO}\n"
            "role=kernel pid=4102 start=908172 state=sleeping\n"
            "requested_stages=shutdown,sigterm,sigkill\n"
            "stage_results=shutdown=acknowledged,sigterm=sent,"
            "sigkill=not-required\nauthority_denied=false\npadding="
        )
        padding = "x" * (65_536 - len(prefix.encode("utf-8")))
        exact_limit = prefix + padding
        self.assertEqual(len(exact_limit.encode("utf-8")), 65_536)
        with self.assertRaises(CandidateError) as within_output_limit:
            self.invoke(
                observation="complete-nonempty",
                survivors=(self.survivor(),),
                diagnostic=exact_limit,
            )
        self.assertEqual(str(within_output_limit.exception), exact_limit)

        with self.assertRaises(CandidateError) as output_overflow:
            self.invoke(
                observation="complete-nonempty",
                survivors=(self.survivor(),),
                diagnostic=exact_limit + "x",
            )
        output_diagnostic = str(output_overflow.exception)
        self.assertIn("incomplete(observation-overflow)", output_diagnostic)
        self.assertLessEqual(len(output_diagnostic.encode("utf-8")), 65_536)

    def test_pid_reuse_and_foreign_identity_never_authorize_an_action(self) -> None:
        matches = getattr(
            python_candidate_module,
            "_notebook_owned_identity_matches",
            None,
        )
        self.assertTrue(callable(matches))
        self.assertEqual(
            tuple(inspect.signature(matches).parameters),
            ("expected", "observed"),
        )
        expected = self.survivor(role="browser-helper", pid=11_108, start_time=120)
        self.assertTrue(matches(expected=expected, observed=dict(expected)))

        reused_pid = dict(expected, start_time=121)
        foreign_role = dict(expected, role="foreign")
        foreign_scenario = dict(expected, scenario="foreign-notebook-host")
        for observed in (reused_pid, foreign_role, foreign_scenario, None):
            with self.subTest(observed=observed):
                self.assertFalse(matches(expected=expected, observed=observed))


class NotebookOwnedProcessBActionBoundaryTests(unittest.TestCase):
    """Behavioral oracle for the private mechanism-neutral cleanup runner."""

    LIFECYCLE_PARAMETERS = (
        "scenario",
        "primary_error",
        "observe",
        "observe_identity",
        "request_stage",
        "wait",
        "monotonic",
    )

    @staticmethod
    def survivor(*, start_time: int = 908_172) -> dict[str, object]:
        return NotebookOwnedProcessDecisionTests.survivor(start_time=start_time)

    def lifecycle(self) -> Callable[..., None]:
        lifecycle = getattr(
            python_candidate_module,
            "_notebook_cleanup_lifecycle",
            None,
        )
        self.assertTrue(
            callable(lifecycle),
            "#495 requires the precommitted private cleanup lifecycle seam",
        )
        self.assertEqual(
            tuple(inspect.signature(lifecycle).parameters),
            self.LIFECYCLE_PARAMETERS,
        )
        return lifecycle

    def test_00_exact_grace_and_decision_boundaries_drive_actions(self) -> None:
        lifecycle = self.lifecycle()
        survivor = self.survivor()
        clock = types.SimpleNamespace(now=100.0)

        def clock_read() -> float:
            return clock.now
        observations = iter(
            (
                ("complete-nonempty", (survivor,)),
                ("complete-empty", ()),
            )
        )
        actions: list[tuple[object, ...]] = []

        def observe(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, tuple[dict[str, object], ...]]:
            actions.append(("observe", stage, deadline, timeout, clock.now))
            return next(observations)

        def observe_identity(*, expected: dict[str, object]) -> dict[str, object]:
            actions.append(("identity", expected["start_time"]))
            return dict(expected)

        def request_stage(
            *,
            stage: str,
            identity: dict[str, object],
            deadline: float,
            monotonic: Callable[[], float],
        ) -> str:
            actions.append(
                (
                    "request",
                    stage,
                    identity["start_time"],
                    deadline,
                    monotonic is clock_read,
                )
            )
            return f"{stage}=sent"

        def wait(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, int | str | None]:
            actions.append(("wait", stage, deadline, timeout))
            self.assertEqual(stage, "graceful")
            self.assertEqual(deadline, 135.0)
            self.assertEqual(timeout, 30.0)
            clock.now = 129.999
            return "reaped-complete-empty", 0

        lifecycle(
            scenario=NotebookOwnedProcessDecisionTests.SCENARIO,
            primary_error=None,
            observe=observe,
            observe_identity=observe_identity,
            request_stage=request_stage,
            wait=wait,
            monotonic=clock_read,
        )
        self.assertEqual(
            actions[:4],
            [
                ("observe", "graceful", 135.0, 30.0, 100.0),
                ("identity", 908_172),
                ("request", "sigterm", 908_172, 135.0, True),
                ("wait", "graceful", 135.0, 30.0),
            ],
        )
        self.assertEqual(
            (actions[4][0], actions[4][1], actions[4][2], actions[4][4]),
            ("observe", "graceful", 135.0, 129.999),
        )
        self.assertAlmostEqual(float(actions[4][3]), 0.001, places=9)

        clock.now = 100.0
        observations = iter(
            (
                ("complete-nonempty", (survivor,)),
                ("complete-nonempty", (survivor,)),
            )
        )
        actions.clear()

        def boundary_wait(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, int | str | None]:
            actions.append(("wait", stage, deadline, timeout))
            self.assertEqual(deadline, 135.0)
            if stage == "graceful":
                self.assertEqual(timeout, 30.0)
                clock.now = 130.0
                return "host-still-running", None
            self.assertEqual(stage, "forced")
            self.assertEqual(timeout, 5.0)
            clock.now = 135.0
            return "reaped-complete-empty", 0

        with self.assertRaises(CandidateError) as raised:
            lifecycle(
                scenario=NotebookOwnedProcessDecisionTests.SCENARIO,
                primary_error=None,
                observe=observe,
                observe_identity=observe_identity,
                request_stage=request_stage,
                wait=boundary_wait,
                monotonic=clock_read,
            )
        diagnostic = str(raised.exception)
        self.assertIn("forced-escalation", diagnostic)
        self.assertIn("cleanup-deadline", diagnostic)
        self.assertIn(("request", "sigkill", 908_172, 135.0, True), actions)
        self.assertIn(("wait", "forced", 135.0, 5.0), actions)
        self.assertIn(("observe", "forced", 135.0, 5.0, 130.0), actions)
        self.assertNotIn(("observe", "final", 135.0, 0.0, 135.0), actions)

    def test_exhausted_budget_starts_no_blocking_forced_action(self) -> None:
        lifecycle = self.lifecycle()
        survivor = self.survivor()
        survivor["requested_stages"] = ()
        survivor["stage_results"] = ()
        clock = types.SimpleNamespace(now=100.0)

        def clock_read() -> float:
            return clock.now
        observations = iter(
            (
                ("complete-nonempty", (survivor,)),
                ("complete-nonempty", (survivor,)),
            )
        )
        actions: list[tuple[object, ...]] = []
        identity_calls: list[tuple[float, object]] = []

        def wait(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, int | str | None]:
            actions.append(("wait", stage, deadline, timeout))
            self.assertEqual((stage, deadline, timeout), ("graceful", 135.0, 30.0))
            clock.now = 135.0
            return "host-still-running", None

        def request_stage(
            *,
            stage: str,
            identity: dict[str, object],
            deadline: float,
            monotonic: Callable[[], float],
        ) -> str:
            actions.append(
                (
                    "request",
                    stage,
                    identity["start_time"],
                    deadline,
                    monotonic is clock_read,
                )
            )
            return f"{stage}=sent"

        def observe(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, tuple[dict[str, object], ...]]:
            actions.append(("observe", stage, deadline, timeout, clock.now))
            return next(observations)

        def observe_identity(
            *, expected: dict[str, object]
        ) -> dict[str, object]:
            identity_calls.append((clock.now, expected["start_time"]))
            self.assertLess(
                clock.now,
                135.0,
                "identity observation may not begin at an exhausted deadline",
            )
            return dict(expected)

        with self.assertRaises(CandidateError) as raised:
            lifecycle(
                scenario=NotebookOwnedProcessDecisionTests.SCENARIO,
                primary_error=None,
                observe=observe,
                observe_identity=observe_identity,
                request_stage=request_stage,
                wait=wait,
                monotonic=clock_read,
            )
        diagnostic = str(raised.exception)
        self.assertIn("cleanup-deadline", diagnostic)
        self.assertIn(
            f"scenario={NotebookOwnedProcessDecisionTests.SCENARIO}",
            diagnostic,
        )
        self.assertIn("requested_stages=sigterm", diagnostic)
        self.assertIn("sigterm=sent", diagnostic)
        self.assertEqual(
            [action[1] for action in actions if action[0] == "request"],
            ["sigterm"],
        )
        self.assertEqual(
            [action[1] for action in actions if action[0] == "wait"],
            ["graceful"],
        )
        self.assertEqual(
            actions,
            [
                ("observe", "graceful", 135.0, 30.0, 100.0),
                ("request", "sigterm", 908_172, 135.0, True),
                ("wait", "graceful", 135.0, 30.0),
            ],
        )
        self.assertEqual(identity_calls, [(100.0, 908_172)])

    def test_pid_reuse_is_revalidated_immediately_before_each_action(self) -> None:
        lifecycle = self.lifecycle()
        matches = getattr(
            python_candidate_module,
            "_notebook_owned_identity_matches",
            None,
        )
        self.assertTrue(callable(matches))
        expected = self.survivor(start_time=908_172)
        replacement = dict(expected, start_time=908_173, role="foreign")
        clock = types.SimpleNamespace(now=100.0)

        def clock_read() -> float:
            return clock.now
        observations = iter(
            (
                ("complete-nonempty", (expected,)),
                ("complete-nonempty", (expected,)),
                ("incomplete(stable-identity-mismatch)", (expected,)),
            )
        )
        current_identities = iter((dict(expected), replacement))
        actions: list[tuple[object, ...]] = []
        identity_checks: list[
            tuple[tuple[object, ...], tuple[object, ...]]
        ] = []

        def snapshot_match(
            *, expected: dict[str, object], observed: dict[str, object] | None
        ) -> bool:
            identity_checks.append(
                (
                    tuple(
                        expected.get(field)
                        for field in ("scenario", "role", "pid", "start_time")
                    ),
                    tuple(
                        observed.get(field) if observed is not None else None
                        for field in ("scenario", "role", "pid", "start_time")
                    ),
                )
            )
            return matches(expected=expected, observed=observed)

        def observe_identity(*, expected: dict[str, object]) -> dict[str, object]:
            observed = next(current_identities)
            actions.append(("identity", observed["start_time"]))
            return observed

        def request_stage(
            *,
            stage: str,
            identity: dict[str, object],
            deadline: float,
            monotonic: Callable[[], float],
        ) -> str:
            actions.append(
                (
                    "request",
                    stage,
                    identity["start_time"],
                    deadline,
                    monotonic is clock_read,
                )
            )
            return f"{stage}=sent"

        def wait(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, int | str | None]:
            actions.append(("wait", stage, deadline, timeout))
            self.assertIn(stage, ("graceful", "forced"))
            clock.now = 130.0
            return "host-still-running", None

        with mock.patch.object(
            python_candidate_module,
            "_notebook_owned_identity_matches",
            side_effect=snapshot_match,
        ) as checked_identity:
            with self.assertRaises(CandidateError) as raised:
                lifecycle(
                    scenario=NotebookOwnedProcessDecisionTests.SCENARIO,
                    primary_error=None,
                    observe=lambda *, stage, deadline, timeout: next(observations),
                    observe_identity=observe_identity,
                    request_stage=request_stage,
                    wait=wait,
                    monotonic=clock_read,
                )

        self.assertEqual(
            actions[:4],
            [
                ("identity", 908_172),
                ("request", "sigterm", 908_172, 135.0, True),
                ("wait", "graceful", 135.0, 30.0),
                ("identity", 908_173),
            ],
        )
        self.assertEqual(
            [action for action in actions if action[0] == "request"],
            [("request", "sigterm", 908_172, 135.0, True)],
        )
        replacement_observation = actions.index(("identity", 908_173))
        self.assertEqual(
            [
                action
                for action in actions[replacement_observation + 1 :]
                if action[0] == "request"
            ],
            [],
        )
        self.assertEqual(
            checked_identity.call_count,
            2,
        )
        self.assertEqual(
            identity_checks,
            [
                (
                    ("marimo-0.23.16", "kernel", 4102, 908_172),
                    ("marimo-0.23.16", "kernel", 4102, 908_172),
                ),
                (
                    ("marimo-0.23.16", "kernel", 4102, 908_172),
                    ("marimo-0.23.16", "foreign", 4102, 908_173),
                ),
            ],
        )
        diagnostic = str(raised.exception)
        self.assertIn("incomplete(wait-host-still-running)", diagnostic)
        self.assertIn("pid=4102", diagnostic)
        self.assertIn("start=908172", diagnostic)

    def test_cleanup_source_contains_no_unbounded_wait(self) -> None:
        self.lifecycle()
        source = inspect.getsource(python_candidate_module)
        tree = compile(
            source,
            str(python_candidate_module.__file__),
            "exec",
            ast.PyCF_ONLY_AST,
        )
        unbounded_waits = [
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr == "wait"
            and not node.args
            and not node.keywords
        ]
        self.assertEqual(
            unbounded_waits,
            [],
            "cleanup may not contain a final unbounded wait()",
        )


class NotebookOwnedProcessARealPathTests(unittest.TestCase):
    """Real-process falsifiers routed through the existing Notebook host path."""

    CHILD = """
import signal
import time

signal.signal(signal.SIGTERM, lambda _signum, _frame: raise_exit())

def raise_exit():
    raise SystemExit(0)

while True:
    time.sleep(0.05)
"""
    ROOT = """
import os
import signal
import subprocess
import sys
import time
import warnings

warnings.simplefilter("ignore", ResourceWarning)

child = subprocess.Popen([sys.executable, "-I", "-c", os.environ["EQIORA_CHILD"]])
with open(os.environ["EQIORA_CHILD_PID"], "w", encoding="ascii") as stream:
    stream.write(str(child.pid))

def stop(_signum, _frame):
    if os.environ["EQIORA_CASCADE"] == "1":
        child.send_signal(signal.SIGTERM)
        deadline = time.monotonic() + 1.0
        while child.poll() is None and time.monotonic() < deadline:
            time.sleep(0.01)
    raise SystemExit(0)

signal.signal(signal.SIGTERM, stop)
while True:
    time.sleep(0.05)
"""

    @staticmethod
    def process_is_live(pid: int) -> bool:
        try:
            state = (
                (Path("/proc") / str(pid) / "stat")
                .read_text(encoding="ascii")
                .split()[2]
            )
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            return False
        return state != "Z"

    @staticmethod
    def linux_start_identity(pid: int) -> int:
        stat_record = (Path("/proc") / str(pid) / "stat").read_text(
            encoding="ascii"
        )
        comm_end = stat_record.rfind(")")
        if comm_end < 0:
            raise AssertionError(f"missing comm terminator in /proc/{pid}/stat")
        fields_from_state = stat_record[comm_end + 2 :].split()
        return int(fields_from_state[19])

    @classmethod
    def bounded_test_cleanup(cls, *pids: int) -> None:
        for pid in pids:
            try:
                os.kill(pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
        deadline = time.monotonic() + 1.0
        while time.monotonic() < deadline:
            for pid in pids:
                try:
                    os.waitpid(pid, os.WNOHANG)
                except (ChildProcessError, ProcessLookupError):
                    pass
            if not any(cls.process_is_live(pid) for pid in pids):
                return
            time.sleep(0.01)
        survivors = tuple(pid for pid in pids if cls.process_is_live(pid))
        if survivors:
            raise AssertionError(f"test harness cleanup retained PIDs: {survivors}")

    def run_one_real_host(
        self,
        root: Path,
        *,
        cascade: bool = False,
        operation_error: BaseException | None = None,
        capture: dict[str, object] | None = None,
        guard_lifecycle_actions: bool = False,
    ) -> int:
        executor = importlib.import_module("python_candidate_h2")
        profiles = importlib.import_module("python_candidate_profiles")
        extracted = root / "extracted"
        fixture = extracted / "bindings/python/tests/fixtures/host.ipynb"
        fixture.parent.mkdir(parents=True)
        fixture.write_text("{}", encoding="utf-8")
        exact_app = extracted / python_candidate_module.EXACT_CYLINDER_STOKES_MARIMO_APP
        exact_app.parent.mkdir(parents=True, exist_ok=True)
        exact_app.write_text("import marimo\n", encoding="utf-8")
        exact_mutant = (
            extracted / python_candidate_module.EXACT_CYLINDER_STOKES_MARIMO_MUTANT
        )
        exact_mutant.parent.mkdir(parents=True, exist_ok=True)
        exact_mutant.write_text("raise RuntimeError\n", encoding="utf-8")
        browser = root / "browser"
        browser.write_bytes(b"browser")
        npm = root / "npm"
        node = root / "node"
        npm.write_bytes(b"npm")
        node.write_bytes(b"node")
        acquired = types.SimpleNamespace(
            browser_archive_sha256="a" * 64,
            browser_executable_sha256="b" * 64,
            browser_platform="linux-x86_64",
            browser_executable=browser,
            python_wheels=(),
            npm=npm,
            node=node,
        )
        receipt = {
            "browser": {
                "downloaded_archive_sha256": acquired.browser_archive_sha256,
                "executable_sha256": acquired.browser_executable_sha256,
                "platform": acquired.browser_platform,
            },
            "python_host": {
                "resolved_environment_sha256": executor.structured_sha256(())
            },
        }
        frontend = {
            "h2_receipt_sha256": hashlib.sha256(
                executor.canonical_json_bytes(receipt)
            ).hexdigest()
        }
        workspace_root = root / "profile"
        workspace = types.SimpleNamespace(
            root=workspace_root,
            environment=workspace_root / "environment",
            consumer=workspace_root / "consumer",
        )
        child_pid_path = root / "owned-child.pid"
        real_popen = subprocess.Popen
        real_os_kill = os.kill
        launched_root: subprocess.Popen[str] | None = None
        callback_state = {
            "request_depth": 0,
            "wait_depth": 0,
            "harness_cleanup": False,
        }

        def launch_root(
            _argv: list[str],
            **kwargs: object,
        ) -> subprocess.Popen[str]:
            nonlocal launched_root
            environment = dict(kwargs["env"])
            environment.update(
                {
                    "EQIORA_CHILD": self.CHILD,
                    "EQIORA_CHILD_PID": str(child_pid_path),
                    "EQIORA_CASCADE": "1" if cascade else "0",
                }
            )
            launched_root = real_popen(
                [sys.executable, "-I", "-c", self.ROOT],
                cwd=kwargs["cwd"],
                env=environment,
                stdout=kwargs["stdout"],
                stderr=kwargs["stderr"],
                text=kwargs["text"],
            )
            if capture is not None:
                capture["root_pid"] = launched_root.pid
            deadline = time.monotonic() + 2.0
            child_pid = None
            while time.monotonic() < deadline:
                if launched_root.poll() is not None:
                    break
                try:
                    child_pid = int(child_pid_path.read_text(encoding="ascii"))
                except (FileNotFoundError, ValueError):
                    pass
                else:
                    break
                time.sleep(0.01)
            if child_pid is None:
                raise AssertionError("controlled host did not publish its child PID")
            if capture is not None:
                capture["child_pid"] = child_pid
                capture["owned_identities"] = (
                    (launched_root.pid, self.linux_start_identity(launched_root.pid)),
                    (child_pid, self.linux_start_identity(child_pid)),
                )
                event_log = capture["events"]
                event_log.append(
                    (
                        "launch",
                        launched_root.pid,
                        child_pid,
                        self.process_is_live(launched_root.pid),
                        self.process_is_live(child_pid),
                    )
                )
            real_send_signal = launched_root.send_signal
            real_wait = launched_root.wait

            def guarded_send_signal(signum: int) -> None:
                if callback_state["harness_cleanup"]:
                    real_send_signal(signum)
                    return
                event_log.append(
                    (
                        "host-signal"
                        if callback_state["request_depth"]
                        else "bypass-signal",
                        launched_root.pid,
                        signum,
                    )
                )
                real_send_signal(signum)

            def guarded_wait(
                *args: object,
                **kwargs: object,
            ) -> int:
                if callback_state["harness_cleanup"]:
                    return real_wait(*args, **kwargs)
                event_log.append(
                    (
                        "host-wait"
                        if callback_state["wait_depth"]
                        else "bypass-wait",
                        launched_root.pid,
                        kwargs.get("timeout"),
                    )
                )
                return real_wait(*args, **kwargs)

            if guard_lifecycle_actions:
                launched_root.send_signal = guarded_send_signal
                launched_root.wait = guarded_wait
            return launched_root

        def guarded_os_kill(pid: int, signum: int) -> None:
            if callback_state["harness_cleanup"]:
                real_os_kill(pid, signum)
                return
            event_log.append(
                (
                    "os-signal"
                    if callback_state["request_depth"]
                    else "bypass-os-signal",
                    pid,
                    signum,
                )
            )
            real_os_kill(pid, signum)

        def checked_run(argv: list[str], **_kwargs: object) -> str:
            if tuple(argv[:4]) == ("npm", "run", "test:hosts", "--"):
                if operation_error is not None:
                    raise operation_error
            if any(Path(value).name == exact_mutant.name for value in argv):
                raise subprocess.CalledProcessError(
                    1,
                    argv,
                    output=python_candidate_module.EXACT_CYLINDER_STOKES_MARIMO_MUTANT_FAILURE,
                )
            return ""

        def run_first_host(
            observations: tuple[tuple[str, Callable[[], None]], ...],
            *,
            emit: Callable[[str], None],
        ) -> tuple[str, ...]:
            selected = observations[:6]
            for name, observe in selected:
                observe()
                emit(name)
            return tuple(name for name, _ in selected)

        def stage_frontend(_source: Path, build: object) -> None:
            Path(build.frontend).mkdir(parents=True)

        decision_calls: list[tuple[tuple[object, ...], dict[str, object]]] = []
        lifecycle_calls: list[tuple[tuple[object, ...], dict[str, object]]] = []
        event_log: list[tuple[object, ...]] = []
        if capture is not None:
            capture["decision_calls"] = decision_calls
            capture["lifecycle_calls"] = lifecycle_calls
            capture["events"] = event_log

        try:
            with contextlib.ExitStack() as stack:
                stack.enter_context(
                    mock.patch.object(
                        profiles,
                        "run_notebook_profile",
                        side_effect=run_first_host,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        profiles,
                        "install_environment",
                        return_value=root / "python",
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module,
                        "checked_run",
                        side_effect=checked_run,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module.subprocess,
                        "Popen",
                        side_effect=launch_root,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module.socket,
                        "create_connection",
                        return_value=mock.MagicMock(),
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        executor,
                        "stage_frontend",
                        side_effect=stage_frontend,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        executor,
                        "acquire_inputs",
                        return_value=acquired,
                    )
                )
                if guard_lifecycle_actions:
                    stack.enter_context(
                        mock.patch.object(
                            python_candidate_module.os,
                            "kill",
                            side_effect=guarded_os_kill,
                        )
                    )

                decision = getattr(
                    python_candidate_module,
                    "_notebook_cleanup_decision",
                    None,
                )
                if callable(decision):

                    def record_decision(
                        *args: object,
                        **kwargs: object,
                    ) -> None:
                        decision_calls.append((args, dict(kwargs)))
                        if capture is not None:
                            root_pid = int(capture["root_pid"])
                            child_pid = int(capture["child_pid"])
                            event_log.append(
                                (
                                    "decision",
                                    root_pid,
                                    child_pid,
                                    self.process_is_live(root_pid),
                                    self.process_is_live(child_pid),
                                    kwargs.get("observation"),
                                )
                            )
                        return decision(*args, **kwargs)

                    stack.enter_context(
                        mock.patch.object(
                            python_candidate_module,
                            "_notebook_cleanup_decision",
                            side_effect=record_decision,
                        )
                    )

                lifecycle = getattr(
                    python_candidate_module,
                    "_notebook_cleanup_lifecycle",
                    None,
                )
                if callable(lifecycle):

                    def record_lifecycle(
                        *args: object,
                        **kwargs: object,
                    ) -> None:
                        lifecycle_calls.append((args, dict(kwargs)))
                        wrapped = dict(kwargs)
                        if capture is not None:
                            root_pid = int(capture["root_pid"])
                            child_pid = int(capture["child_pid"])
                            event_log.append(
                                (
                                    "lifecycle-enter",
                                    root_pid,
                                    child_pid,
                                    self.process_is_live(root_pid),
                                    self.process_is_live(child_pid),
                                )
                            )
                            observe = kwargs["observe"]
                            observe_identity = kwargs["observe_identity"]
                            request_stage = kwargs["request_stage"]
                            wait = kwargs["wait"]

                            def record_observe(
                                *, stage: str, deadline: float, timeout: float
                            ) -> tuple[str, tuple[dict[str, object], ...]]:
                                event_log.append(
                                    (
                                        "observe-enter",
                                        stage,
                                        deadline,
                                        timeout,
                                        self.process_is_live(root_pid),
                                        self.process_is_live(child_pid),
                                    )
                                )
                                terminal, survivors = observe(
                                    stage=stage,
                                    deadline=deadline,
                                    timeout=timeout,
                                )
                                membership = tuple(
                                    sorted(
                                        (
                                            int(survivor["pid"]),
                                            int(survivor["start_time"]),
                                        )
                                        for survivor in survivors
                                    )
                                )
                                event_log.append(
                                    (
                                        "observe-exit",
                                        terminal,
                                        membership,
                                        self.process_is_live(root_pid),
                                        self.process_is_live(child_pid),
                                    )
                                )
                                return terminal, survivors

                            def record_identity(
                                *, expected: dict[str, object]
                            ) -> dict[str, object] | None:
                                observed = observe_identity(expected=expected)
                                event_log.append(
                                    (
                                        "identity",
                                        int(expected["pid"]),
                                        int(expected["start_time"]),
                                        None
                                        if observed is None
                                        else int(observed["pid"]),
                                        None
                                        if observed is None
                                        else int(observed["start_time"]),
                                    )
                                )
                                return observed

                            def record_request(
                                *,
                                stage: str,
                                identity: dict[str, object],
                                deadline: float,
                                monotonic: Callable[[], float],
                            ) -> str:
                                event_log.append(
                                    (
                                        "request-enter",
                                        stage,
                                        int(identity["pid"]),
                                        int(identity["start_time"]),
                                        deadline,
                                        monotonic is kwargs["monotonic"],
                                    )
                                )
                                callback_state["request_depth"] += 1
                                try:
                                    result = request_stage(
                                        stage=stage,
                                        identity=identity,
                                        deadline=deadline,
                                        monotonic=monotonic,
                                    )
                                finally:
                                    callback_state["request_depth"] -= 1
                                event_log.append(("request-exit", stage, result))
                                return result

                            def record_wait(
                                *, stage: str, deadline: float, timeout: float
                            ) -> tuple[str, int | str | None]:
                                event_log.append(
                                    ("wait-enter", stage, deadline, timeout)
                                )
                                callback_state["wait_depth"] += 1
                                try:
                                    result = wait(
                                        stage=stage,
                                        deadline=deadline,
                                        timeout=timeout,
                                    )
                                finally:
                                    callback_state["wait_depth"] -= 1
                                event_log.append(("wait-exit", stage, result))
                                return result

                            wrapped.update(
                                {
                                    "observe": record_observe,
                                    "observe_identity": record_identity,
                                    "request_stage": record_request,
                                    "wait": record_wait,
                                }
                            )
                        try:
                            return lifecycle(*args, **wrapped)
                        finally:
                            event_log.append(("lifecycle-exit",))

                    stack.enter_context(
                        mock.patch.object(
                            python_candidate_module,
                            "_notebook_cleanup_lifecycle",
                            side_effect=record_lifecycle,
                        )
                    )

                python_candidate_module.run_notebook_profile(
                    uv="/reviewed/uv",
                    interpreter="/reviewed/python3.13",
                    wheel=root / "candidate.whl",
                    extracted=extracted,
                    workspace=workspace,
                    config=python_candidate_module.load_config(),
                    receipt=receipt,
                    frontend=frontend,
                )
        finally:
            if launched_root is not None and launched_root.poll() is None:
                callback_state["harness_cleanup"] = True
                launched_root.kill()
                launched_root.wait(timeout=1.0)

        return int(child_pid_path.read_text(encoding="ascii"))

    def test_00_ordinary_host_path_reaches_complete_empty(self) -> None:
        owned_pid = -1
        root_pid = -1
        capture: dict[str, object] = {}
        try:
            with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                owned_pid = self.run_one_real_host(
                    Path(temporary),
                    cascade=True,
                    capture=capture,
                    guard_lifecycle_actions=True,
                )
                root_pid = int(capture["root_pid"])
                owned_identities = set(capture["owned_identities"])
                self.assertEqual(len(owned_identities), 2)
                self.assertFalse(self.process_is_live(root_pid))
                self.assertFalse(self.process_is_live(owned_pid))
                events = capture["events"]
                event_names = [event[0] for event in events]
                self.assertEqual(
                    event_names[:2],
                    ["launch", "lifecycle-enter"],
                )
                self.assertEqual(event_names[-1], "lifecycle-exit")
                self.assertNotIn("bypass-signal", event_names)
                self.assertNotIn("bypass-os-signal", event_names)
                self.assertNotIn("bypass-wait", event_names)
                self.assertEqual(
                    events[0],
                    ("launch", root_pid, owned_pid, True, True),
                )
                self.assertEqual(
                    events[1],
                    ("lifecycle-enter", root_pid, owned_pid, True, True),
                )

                observations = [
                    event for event in events if event[0] == "observe-exit"
                ]
                self.assertEqual(
                    observations[0],
                    (
                        "observe-exit",
                        "complete-nonempty",
                        observations[0][2],
                        True,
                        True,
                    ),
                )
                self.assertEqual(set(observations[0][2]), owned_identities)
                self.assertEqual(
                    observations[-1],
                    ("observe-exit", "complete-empty", (), False, False),
                )

                identities = [event for event in events if event[0] == "identity"]
                self.assertTrue(identities)
                self.assertTrue(
                    all(
                        expected_pid == observed_pid
                        and expected_start == observed_start
                        and (expected_pid, expected_start) in owned_identities
                        and (observed_pid, observed_start) in owned_identities
                        for (
                            _event,
                            expected_pid,
                            expected_start,
                            observed_pid,
                            observed_start,
                        ) in identities
                    )
                )
                requests = [
                    event for event in events if event[0] == "request-enter"
                ]
                self.assertTrue(requests)
                self.assertTrue(
                    all(
                        (request[2], request[3]) in owned_identities
                        for request in requests
                    )
                )
                self.assertIn("wait-enter", event_names)
                self.assertIn("request-exit", event_names)
                self.assertIn("wait-exit", event_names)

                decision_events = [
                    (index, event)
                    for index, event in enumerate(events)
                    if event[0] == "decision"
                ]
                self.assertEqual(len(decision_events), 1)
                decision_index, decision_event = decision_events[0]
                self.assertEqual(
                    decision_event,
                    (
                        "decision",
                        root_pid,
                        owned_pid,
                        False,
                        False,
                        "complete-empty",
                    ),
                )
                last_observation = max(
                    index
                    for index, event in enumerate(events)
                    if event[0] == "observe-exit"
                )
                self.assertLess(last_observation, decision_index)
                self.assertLess(decision_index, len(events) - 1)
                lifecycle_calls = capture["lifecycle_calls"]
                decision_calls = capture["decision_calls"]
                self.assertEqual(len(lifecycle_calls), 1)
                self.assertEqual(len(decision_calls), 1)
                decision_args, decision = decision_calls[0]
                self.assertEqual(decision_args, ())
                self.assertEqual(
                    decision["scenario"],
                    NotebookOwnedProcessDecisionTests.SCENARIO,
                )
                self.assertIsNone(decision["primary_error"])
                self.assertFalse(decision["forced_escalation"])
                self.assertEqual(decision["observation"], "complete-empty")
                self.assertEqual(decision["survivors"], ())
                self.assertLess(
                    decision["observed_at"],
                    decision["cleanup_started"] + 35.0,
                )
        finally:
            self.bounded_test_cleanup(
                *(pid for pid in (root_pid, owned_pid) if pid > 0)
            )

    def test_owned_helper_is_cleaned_while_same_argv_foreign_process_survives(
        self,
    ) -> None:
        real_popen = subprocess.Popen
        foreign = real_popen([sys.executable, "-I", "-c", self.CHILD])
        owned_pid = -1
        try:
            with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                owned_pid = self.run_one_real_host(Path(temporary))
                self.assertFalse(self.process_is_live(owned_pid))
                self.assertIsNone(foreign.poll())
        finally:
            self.bounded_test_cleanup(
                *(pid for pid in (owned_pid, foreign.pid) if pid > 0)
            )
            try:
                foreign.wait(timeout=0.1)
            except (ChildProcessError, subprocess.TimeoutExpired):
                pass

    def test_primary_failure_cannot_skip_owned_helper_cleanup(self) -> None:
        owned_pid = -1
        capture: dict[str, object] = {}
        primary = RuntimeError("host-payload-failed")
        try:
            with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                with self.assertRaises((CandidateError, RuntimeError)) as raised:
                    owned_pid = self.run_one_real_host(
                        Path(temporary),
                        operation_error=primary,
                        capture=capture,
                    )
                if owned_pid < 0:
                    owned_pid = int(capture.get("child_pid", -1))
                self.assertGreater(owned_pid, 0)
                self.assertFalse(self.process_is_live(owned_pid))
                self.assertIsInstance(raised.exception, CandidateError)
                self.assertIs(raised.exception.__cause__, primary)
                diagnostic = str(raised.exception)
                self.assertIn("primary=RuntimeError: host-payload-failed", diagnostic)
                self.assertIn("cleanup=complete-empty", diagnostic)
                decision_calls = capture["decision_calls"]
                self.assertEqual(len(decision_calls), 1)
                decision_args, decision = decision_calls[0]
                self.assertEqual(decision_args, ())
                self.assertIs(decision["primary_error"], primary)
                self.assertEqual(decision["observation"], "complete-empty")
                self.assertEqual(decision["survivors"], ())
        finally:
            if owned_pid > 0:
                self.bounded_test_cleanup(owned_pid)


if __name__ == "__main__":
    unittest.main()
