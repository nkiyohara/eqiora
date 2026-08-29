#!/usr/bin/env python3
"""Build and verify one commit-bound Eqiora Python distribution candidate."""

from __future__ import annotations

import argparse
import email.parser
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import socket
import subprocess
import sys
import tarfile
import tempfile
import time
import tomllib
import zipfile
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import python_candidate_profiles as candidate_profiles
from candidate_manifest import (
    BROWSERS_JSON_SHA256,
    NODE_EXECUTABLE_SHA256,
    NOTEBOOK_CHECKS,
    NPM_PACKAGE_INTEGRITY,
    PROFILE_CHECKS,
    REQUIRED_PROFILES,
    load_candidate_family,
    verify_artifacts,
)
from python_candidate_common import (
    CandidateError,
    DistributionConfig,
    candidate_payload_identity,
    checked_run,
    command_context,
    home_scratch_parent,
    python_distribution_version as python_distribution_version,
    sha256 as sha256,
    source_tree_sha256,
)


ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT
PYPROJECT = ROOT / "pyproject.toml"
MANIFEST_FORMAT = "eqiora.python-distribution-candidate/v3"
EXACT_CYLINDER_STOKES_MARIMO_APP = Path(
    "examples/python/exact_cylinder_stokes_marimo.py"
)
EXACT_CYLINDER_STOKES_MARIMO_MUTANT = Path(
    "verify/interfaces/python-exact-cylinder-stokes-marimo/references/exact_cylinder_stokes_marimo_repository_helper_mutant.py"
)
EXACT_CYLINDER_STOKES_MARIMO_CHECK = (
    "cp313:marimo-0.23.16-exact-cylinder-stokes"
)
EXACT_CYLINDER_STOKES_MARIMO_ORACLE_FLAG = (
    "EQIORA_EXACT_CYLINDER_STOKES_MARIMO_ORACLE"
)
EXACT_CYLINDER_STOKES_MARIMO_MUTANT_FAILURE = (
    "ModuleNotFoundError: No module named 'examples'"
)
SHARED_SEMANTIC_VIEWER_MARIMO_APP = Path(
    "examples/python/shared_semantic_viewer_marimo.py"
)
SHARED_SEMANTIC_VIEWER_MARIMO_CHECK = (
    "cp313:marimo-0.23.16-shared-semantic-viewer"
)
SHARED_SEMANTIC_VIEWER_MARIMO_ORACLE_FLAG = (
    "EQIORA_SHARED_SEMANTIC_VIEWER_MARIMO_ORACLE"
)
PYTHON_TEST_FIXTURES = candidate_profiles.PYTHON_TEST_FIXTURES
PYTHON_TEST_RESOURCES = candidate_profiles.PYTHON_TEST_RESOURCES
GIT_SHA = re.compile(r"[0-9a-f]{40}")


@dataclass(frozen=True)
class SourceIdentity:
    """Exact source state from which a candidate is built."""

    commit: str
    tags: tuple[str, ...]


@dataclass(frozen=True)
class CandidateProfileSummary:
    """Accepted profile observations used only to finalize one manifest."""

    config: DistributionConfig
    uv: str
    wheel_records: tuple[dict[str, Any], ...]
    checks: tuple[str, ...]
    dependency_profiles: dict[str, dict[str, str]]


def exact_group_requirement(
    document: dict[str, Any],
    group: str,
    package: str,
) -> str:
    """Return one exact standard dependency-group requirement."""

    requirements = document.get("dependency-groups", {}).get(group, [])
    prefix = f"{package}=="
    matches = [
        requirement
        for requirement in requirements
        if isinstance(requirement, str) and requirement.startswith(prefix)
    ]
    if len(matches) != 1 or matches[0] == prefix:
        raise CandidateError(
            f"dependency-groups.{group} must contain one exact {package} requirement"
        )
    return matches[0]


def load_config() -> DistributionConfig:
    """Load the single reviewed matrix and tool inventory."""

    document = tomllib.loads(PYPROJECT.read_text(encoding="utf-8"))
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    project = document["project"]
    if "version" in project or project.get("dynamic") != ["version"]:
        raise CandidateError("Python version must derive only from Cargo")
    build_requires = document["build-system"]["requires"]
    maturin = [item for item in build_requires if item.startswith("maturin==")]
    if len(maturin) != 1:
        raise CandidateError(
            "build-system.requires must contain one exact maturin== version"
        )

    raw = document["tool"]["eqiora-distribution"]
    uv = exact_group_requirement(document, "release-tools", "uv")
    twine = exact_group_requirement(document, "release-tools", "twine")
    interpreters = tuple(raw["ordinary-gil-cpython"])
    jax = tuple(raw["tested-jax"])
    config = DistributionConfig(
        cargo_version=cargo["workspace"]["package"]["version"],
        interpreters=interpreters,
        wheel_platform=raw["wheel-platform"],
        extras_interpreter=raw["extras-python"],
        numpy_floor_interpreter=raw["numpy-floor-python"],
        numpy_floor=raw["tested-numpy-floor"],
        uv=uv,
        maturin=maturin[0],
        pytest=raw["pytest"],
        mypy=raw["mypy"],
        twine=twine,
        torch=raw["tested-torch"],
        jax=jax,
        matplotlib=raw["tested-matplotlib"],
        rust=cargo["workspace"]["package"]["rust-version"],
    )
    if config.interpreters != ("3.11", "3.12", "3.13", "3.14"):
        raise CandidateError("the first candidate matrix must be CPython 3.11-3.14")
    if config.extras_interpreter not in config.interpreters:
        raise CandidateError("extras-python must belong to the wheel matrix")
    if config.numpy_floor_interpreter not in config.interpreters:
        raise CandidateError("numpy-floor-python must belong to the wheel matrix")
    if config.numpy_floor_interpreter != "3.12" or config.numpy_floor != "numpy==2.1.0":
        raise CandidateError(
            "the first candidate must verify the exact NumPy 2.1.0 floor on CPython 3.12"
        )
    if config.wheel_platform != "manylinux_2_17_x86_64":
        raise CandidateError(
            "the first candidate must retain the manylinux_2_17_x86_64 floor"
        )
    exact_uv_version(config.uv)
    return config


def require_executable(name: str) -> str:
    """Resolve one conventional release tool or fail clearly."""

    executable = shutil.which(name)
    if executable is None:
        raise CandidateError(f"required executable is unavailable: {name}")
    return executable


def exact_uv_version(requirement: str) -> str:
    """Return the path-safe version from one exact ``uv`` requirement."""
    name, separator, expected = requirement.partition("==")
    components = expected.split(".")
    if (
        name != "uv"
        or not separator
        or len(components) != 3
        or any(
            not component.isascii() or not component.isdecimal()
            for component in components
        )
    ):
        raise CandidateError("the uv build-tool requirement is malformed")
    return expected


def require_exact_uv(executable: str, requirement: str) -> None:
    """Require the declared release tool rather than an ambient compatible one."""

    expected = exact_uv_version(requirement)
    observed = tool_version([executable, "--version"]).split()
    if len(observed) < 2 or observed[0] != "uv" or observed[1] != expected:
        raise CandidateError(
            f"candidate requires uv {expected}, observed {' '.join(observed)!r}"
        )


def _virtual_environment_executable(environment: Path, name: str) -> Path:
    directory = "Scripts" if os.name == "nt" else "bin"
    suffix = ".exe" if os.name == "nt" else ""
    return environment / directory / f"{name}{suffix}"


def ensure_exact_uv(
    requirement: str,
    *,
    cache_root: Path | None = None,
) -> str:
    """Resolve the reviewed ``uv`` release into an immutable home cache entry."""

    version = exact_uv_version(requirement)
    home = Path.home().resolve()
    root = (
        cache_root.resolve()
        if cache_root is not None
        else home / ".cache" / "eqiora" / "tools"
    )
    if cache_root is None and not root.is_relative_to(home):
        raise CandidateError("the uv tool cache must remain below the home directory")
    uv_root = root / "uv"
    uv_root.mkdir(parents=True, exist_ok=True)
    resolved_root = uv_root.resolve()
    if cache_root is None and not resolved_root.is_relative_to(home):
        raise CandidateError("the uv tool cache must remain below the home directory")

    tool_root = resolved_root / version
    executable = _virtual_environment_executable(tool_root, "uv")
    if executable.is_file():
        require_exact_uv(str(executable), requirement)
        return str(executable)
    if tool_root.exists():
        raise CandidateError(f"the cached uv tool is incomplete: {tool_root}")

    with tempfile.TemporaryDirectory(
        prefix=f".{version}-", dir=resolved_root
    ) as temporary:
        staged = Path(temporary) / "tool"
        checked_run([sys.executable, "-m", "venv", str(staged)])
        staged_python = _virtual_environment_executable(staged, "python")
        checked_run(
            [
                str(staged_python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--only-binary=:all:",
                requirement,
            ]
        )
        staged_uv = _virtual_environment_executable(staged, "uv")
        require_exact_uv(str(staged_uv), requirement)
        try:
            staged.rename(tool_root)
        except OSError as error:
            if executable.is_file():
                require_exact_uv(str(executable), requirement)
                return str(executable)
            raise CandidateError(
                f"failed to publish cached uv tool: {tool_root}"
            ) from error

    require_exact_uv(str(executable), requirement)
    return str(executable)


def source_identity() -> SourceIdentity:
    """Require a clean commit and return its full identity."""

    status = checked_run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        capture=True,
    )
    if status:
        raise CandidateError("a Python candidate requires a clean source tree")
    commit = checked_run(["git", "rev-parse", "HEAD"], capture=True)
    if len(commit) != 40:
        raise CandidateError("git did not return a full source commit")
    tags = tuple(
        line
        for line in checked_run(
            ["git", "tag", "--points-at", commit],
            capture=True,
        ).splitlines()
        if line
    )
    return SourceIdentity(commit=commit, tags=tags)


def require_expected_tag(source: SourceIdentity, expected_tag: str) -> None:
    """Require the release tag derived from the normalized artifact version."""

    if expected_tag not in source.tags:
        raise CandidateError(
            f"publication requires exact tag {expected_tag} on the source commit"
        )


def require_annotated_expected_tag(
    source: SourceIdentity,
    expected_tag: str,
    *,
    git_query: Callable[..., str] = checked_run,
) -> None:
    """Require the exact annotated tag to peel to the candidate commit."""

    require_expected_tag(source, expected_tag)
    reference = f"refs/tags/{expected_tag}"
    if git_query(["git", "cat-file", "-t", reference], capture=True) != "tag":
        raise CandidateError(
            f"publication requires annotated tag {expected_tag}, not a lightweight tag"
        )
    peeled = git_query(
        ["git", "rev-parse", f"{reference}^{{commit}}"],
        capture=True,
    )
    if peeled != source.commit:
        raise CandidateError(
            f"annotated tag {expected_tag} does not peel to the candidate commit"
        )


def cargo_workspace_version(root: Path) -> str:
    """Read the sole authored release version from one source root."""

    document = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    return document["workspace"]["package"]["version"]


def safe_extract_sdist(archive: Path, destination: Path) -> Path:
    """Extract a regular-file/directory sdist without path traversal."""

    root = destination.resolve()
    with tarfile.open(archive, mode="r:gz") as source:
        members = source.getmembers()
        for member in members:
            target = (root / member.name).resolve()
            if not target.is_relative_to(root):
                raise CandidateError(f"sdist path escapes its root: {member.name}")
            if not (member.isfile() or member.isdir()):
                raise CandidateError(
                    f"sdist contains a non-regular member: {member.name}"
                )
        source.extractall(destination, members=members)

    children = [path for path in destination.iterdir() if path.is_dir()]
    if len(children) != 1:
        raise CandidateError("sdist must contain exactly one top-level directory")
    extracted = children[0]
    required = (
        extracted / "Cargo.toml",
        extracted / "Cargo.lock",
        extracted / "pyproject.toml",
        extracted / "crates/eqiora-python/Cargo.toml",
    )
    missing = [
        str(path.relative_to(extracted)) for path in required if not path.is_file()
    ]
    if missing:
        raise CandidateError(f"sdist is incomplete: {', '.join(missing)}")
    return extracted


def uv_interpreter(uv: str, version: str) -> str:
    """Resolve an exact managed CPython interpreter."""

    checked_run([uv, "python", "install", version])
    interpreter = checked_run(
        [uv, "python", "find", "--managed-python", version],
        capture=True,
    ).splitlines()[-1]
    if not Path(interpreter).is_file():
        raise CandidateError(f"uv did not resolve CPython {version}")
    observed = checked_run(
        [
            interpreter,
            "-I",
            "-c",
            "import platform,sys; print(platform.python_implementation()); "
            "print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ],
        capture=True,
    ).splitlines()
    if observed != ["CPython", version]:
        raise CandidateError(
            f"requested CPython {version}, resolved {' '.join(observed)}"
        )
    return interpreter


def maturin_package(config: DistributionConfig, *, zig: bool) -> str:
    """Return the exact maturin tool requirement, optionally with Zig."""

    name, version = config.maturin.split("==", maxsplit=1)
    suffix = "[zig]" if zig else ""
    return f"{name}{suffix}=={version}"


def _producer_file_identity(path: Path) -> tuple[int, str]:
    if path.is_symlink() or not path.is_file() or path.stat().st_nlink != 1:
        raise CandidateError(
            f"candidate producer emitted a non-regular file: {path.name}"
        )
    return path.stat().st_size, sha256(path)


def build_artifacts(
    *,
    output: Path,
    scratch: Path,
    config: DistributionConfig,
    uv: str,
    interpreters: dict[str, str],
) -> tuple[Path, dict[str, Path], Path]:
    """Build one sdist, then every wheel solely from its extracted content."""

    if not output.is_dir() or output.is_symlink() or any(output.iterdir()):
        raise CandidateError("candidate producer output directory must be empty")
    rust_environment = {"RUSTUP_TOOLCHAIN": config.rust}
    sdist_tool = maturin_package(config, zig=False)
    checked_run(
        [
            uv,
            "tool",
            "run",
            "--from",
            sdist_tool,
            "maturin",
            "sdist",
            "--out",
            str(output),
        ],
        cwd=PACKAGE,
        extra_environment=rust_environment,
    )
    expected_sdist = f"eqiora-{config.python_version}.tar.gz"
    members = {path.name: path for path in output.iterdir()}
    if set(members) != {expected_sdist}:
        raise CandidateError(
            f"source distribution identity drifted: expected {expected_sdist}, "
            f"received {', '.join(sorted(members)) or '<nothing>'}"
        )
    sdist = members[expected_sdist]
    retained_identities = {sdist.name: _producer_file_identity(sdist)}

    extracted = safe_extract_sdist(sdist, scratch / "source")
    if cargo_workspace_version(extracted) != config.cargo_version:
        raise CandidateError(
            "source distribution Cargo version differs from the candidate source"
        )
    target = scratch / "cargo-target"
    wheels: dict[str, Path] = {}
    wheel_tool = maturin_package(config, zig=True)
    for version in config.interpreters:
        before = {path.name for path in output.iterdir()}
        checked_run(
            [
                uv,
                "tool",
                "run",
                "--from",
                wheel_tool,
                "maturin",
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
                str(target),
                "--out",
                str(output),
            ],
            cwd=extracted,
            extra_environment=rust_environment,
        )
        after = {path.name: path for path in output.iterdir()}
        compact = version.replace(".", "")
        expected_wheel = (
            f"eqiora-{config.python_version}-cp{compact}-cp{compact}-"
            "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
        )
        created = set(after) - before
        if not before.issubset(after) or created != {expected_wheel}:
            raise CandidateError(f"CPython {version} producer output identity drifted")
        for name, identity in retained_identities.items():
            if _producer_file_identity(after[name]) != identity:
                raise CandidateError(
                    f"CPython {version} build mutated retained artifact {name}"
                )
        wheel = after[expected_wheel]
        retained_identities[wheel.name] = _producer_file_identity(wheel)
        wheels[version] = wheel
    return sdist, wheels, extracted


def parse_metadata(payload: bytes) -> email.message.Message:
    """Parse one package metadata document under the standard email grammar."""

    return email.parser.BytesParser().parsebytes(payload)


def inspect_wheel(
    wheel: Path,
    *,
    python_version: str,
    config: DistributionConfig,
    license_bytes: bytes,
    notice_bytes: bytes,
) -> tuple[str, dict[str, Any]]:
    """Verify tags, package files, metadata, and notices in one wheel."""

    compact = python_version.replace(".", "")
    expected_name = (
        f"eqiora-{config.python_version}-cp{compact}-cp{compact}-"
        "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
    )
    if wheel.name != expected_name:
        raise CandidateError(f"wheel has the wrong exact identity: {wheel.name}")

    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        metadata_names = [
            name for name in names if name.endswith(".dist-info/METADATA")
        ]
        if len(metadata_names) != 1:
            raise CandidateError(f"wheel has an ambiguous METADATA: {wheel.name}")
        metadata = parse_metadata(archive.read(metadata_names[0]))
        dist_info = metadata_names[0].removesuffix("METADATA")

        required = (
            "eqiora/__init__.py",
            "eqiora/__init__.pyi",
            "eqiora/diff.pyi",
            "eqiora/fsi.pyi",
            "eqiora/jax.pyi",
            "eqiora/matplotlib.pyi",
            "eqiora/viewer.pyi",
            "eqiora/solid.pyi",
            "eqiora/torch.pyi",
            "eqiora/py.typed",
            "eqiora/_viewer/THIRD_PARTY_NOTICES.txt",
            "eqiora/_viewer/static/viewer.css",
            "eqiora/_viewer/static/viewer.mjs",
            "eqiora/examples/steady-flow-past-cylinder.eqi",
            "eqiora/examples/transient-flow-past-cylinder.eqi",
            "eqiora/examples/mixed-boundary-elasticity.eqi",
            "eqiora/examples/fixed-reference-fsi.eqi",
            f"{dist_info}licenses/LICENSE",
            f"{dist_info}licenses/NOTICE",
        )
        missing = [name for name in required if name not in names]
        if missing:
            raise CandidateError(
                f"wheel omits required package files: {', '.join(missing)}"
            )
        if archive.read(f"{dist_info}licenses/LICENSE") != license_bytes:
            raise CandidateError("wheel LICENSE does not match the source license")
        if archive.read(f"{dist_info}licenses/NOTICE") != notice_bytes:
            raise CandidateError("wheel NOTICE does not match the source notice")
        if not any(
            name.startswith("eqiora/_eqiora") and name.endswith(".so") for name in names
        ):
            raise CandidateError("wheel omits the native extension")
        if "eqiora/_eqiora.pyi" in names:
            raise CandidateError("the private native module must not be published")
        if not any(".dist-info/sboms/" in name for name in names):
            raise CandidateError("wheel omits its generated native dependency SBOM")
    if metadata["Name"] != "eqiora":
        raise CandidateError("wheel distribution name is not eqiora")
    version = metadata["Version"]
    if version != config.python_version:
        raise CandidateError(
            f"wheel metadata version {version!r} differs from {config.python_version!r}"
        )
    requires_python = {item.strip() for item in metadata["Requires-Python"].split(",")}
    if requires_python != {">=3.11", "<3.15"}:
        raise CandidateError("wheel has an unexpected Requires-Python")
    if metadata["License-Expression"] != "Apache-2.0":
        raise CandidateError("wheel has an unexpected license expression")
    if sorted(metadata.get_all("License-File", [])) != ["LICENSE", "NOTICE"]:
        raise CandidateError("wheel has incomplete PEP 639 license metadata")

    dependencies = metadata.get_all("Requires-Dist", [])
    normalized = [
        dependency.lower().replace(" ", "").replace("'", '"')
        for dependency in dependencies
    ]
    numpy_requirements = [item for item in normalized if item.startswith("numpy")]
    if (
        len(numpy_requirements) != 1
        or ">=2.1" not in numpy_requirements[0]
        or "<3" not in numpy_requirements[0]
        or ";" in numpy_requirements[0]
    ):
        raise CandidateError("wheel must declare the reviewed NumPy range")
    gmsh_requirements = [item for item in normalized if item.startswith("gmsh")]
    if gmsh_requirements != ['gmsh==4.15.2;extra=="gmsh"']:
        raise CandidateError("wheel must declare exactly the Gmsh 4.15.2 extra")
    for framework in ("torch", "jax", "jaxlib", "matplotlib", "anywidget"):
        declarations = [item for item in normalized if item.startswith(framework)]
        if not declarations or any("extra==" not in item for item in declarations):
            raise CandidateError(
                f"{framework} must remain an optional-extra dependency"
            )
    anywidget_requirements = [
        item for item in normalized if item.startswith("anywidget")
    ]
    if anywidget_requirements != ['anywidget==0.11.0;extra=="viewer"']:
        raise CandidateError("wheel must declare exactly the anywidget 0.11.0 extra")
    expected_extras = ["gmsh", "jax", "matplotlib", "torch", "viewer"]
    if sorted(metadata.get_all("Provides-Extra", [])) != expected_extras:
        raise CandidateError(
            "wheel must expose exactly the reviewed optional extras"
        )

    return version, {
        "filename": wheel.name,
        "kind": "wheel",
        "python": python_version,
        "abi": f"cp{compact}",
        "platform": config.wheel_platform,
        "size": wheel.stat().st_size,
        "sha256": sha256(wheel),
    }


prepare_base_consumer_tree = candidate_profiles.prepare_base_consumer_tree
prepare_exact_cylinder_demo_consumer = (
    candidate_profiles.prepare_exact_cylinder_demo_consumer
)
prepare_mixed_boundary_elasticity_demo_consumer = (
    candidate_profiles.prepare_mixed_boundary_elasticity_demo_consumer
)
def run_public_smoke(
    *,
    python: Path,
    extracted: Path,
    run_root: Path,
    expected_version: str,
    profile: str,
) -> None:
    """Replay one published quick start against an installed wheel."""

    candidate_profiles.run_public_smoke(
        python=python,
        extracted=extracted,
        run_root=run_root,
        expected_version=expected_version,
        profile=profile,
        run=checked_run,
    )


def run_base_profile(
    *,
    uv: str,
    interpreter: str,
    python_version: str,
    wheel: Path,
    extracted: Path,
    workspace: candidate_profiles.ProfileWorkspace,
    config: DistributionConfig,
) -> list[str]:
    return candidate_profiles.run_base_profile(
        uv=uv,
        interpreter=interpreter,
        python_version=python_version,
        wheel=wheel,
        extracted=extracted,
        workspace=workspace,
        config=config,
        run=checked_run,
    )


def run_optional_profile(
    *,
    name: str,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    workspace: candidate_profiles.ProfileWorkspace,
    config: DistributionConfig,
) -> list[str]:
    return candidate_profiles.run_optional_profile(
        name=name,
        uv=uv,
        interpreter=interpreter,
        wheel=wheel,
        extracted=extracted,
        workspace=workspace,
        config=config,
        run=checked_run,
    )


def run_numpy_floor_profile(
    *,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    workspace: candidate_profiles.ProfileWorkspace,
    config: DistributionConfig,
) -> tuple[list[str], dict[str, str]]:
    return candidate_profiles.run_numpy_floor_profile(
        uv=uv,
        interpreter=interpreter,
        wheel=wheel,
        extracted=extracted,
        workspace=workspace,
        config=config,
        run=checked_run,
    )


def run_full_typing_profile(
    *,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    workspace: candidate_profiles.ProfileWorkspace,
    config: DistributionConfig,
) -> str:
    return candidate_profiles.run_full_typing_profile(
        uv=uv,
        interpreter=interpreter,
        wheel=wheel,
        extracted=extracted,
        workspace=workspace,
        config=config,
        run=checked_run,
    )


def _reserve_loopback_port() -> int:
    """Return one loopback port this process just held itself."""

    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
        probe.bind(("127.0.0.1", 0))
        return int(probe.getsockname()[1])


_NOTEBOOK_CLEANUP_GRACE_SECONDS = 30.0
_NOTEBOOK_CLEANUP_DECISION_SECONDS = 35.0
_NOTEBOOK_CLEANUP_IDENTITY_LIMIT = 256
_NOTEBOOK_CLEANUP_DIAGNOSTIC_BYTES = 65_536


def _notebook_identity_key(identity: dict[str, object]) -> tuple[str, str, int, int]:
    """Return the deterministic stable-identity ordering used in diagnostics."""

    def integer(field: str) -> int:
        try:
            return int(identity.get(field, -1))
        except (TypeError, ValueError):
            return -1

    return (
        str(identity.get("scenario", "unknown")),
        str(identity.get("role", "unknown")),
        integer("pid"),
        integer("start_time"),
    )


def _notebook_owned_identity_matches(
    *,
    expected: dict[str, object],
    observed: dict[str, object] | None,
) -> bool:
    """Match the causal role and Linux process identity, never a PID alone."""

    if not isinstance(observed, dict):
        return False
    return all(
        observed.get(field) == expected.get(field)
        for field in ("scenario", "role", "pid", "start_time")
    )


def _notebook_bounded_text(text: str, limit: int) -> str:
    """Return a valid UTF-8 prefix whose encoded representation fits ``limit``."""

    encoded = text.encode("utf-8", errors="replace")
    if len(encoded) <= limit:
        return text
    return encoded[:limit].decode("utf-8", errors="ignore")


def _notebook_diagnostic_field(value: object) -> str:
    """Keep one diagnostic field canonical and independently bounded."""

    return _notebook_bounded_text(str(value).replace("\n", "\\n"), 8_192)


def _notebook_cleanup_diagnostic(
    *,
    scenario: str,
    primary_error: BaseException | None,
    forced_escalation: bool,
    observation: str,
    survivors: tuple[dict[str, object], ...],
    deadline_expired: bool,
    secondary: str | None = None,
) -> str:
    """Render one canonical, deterministically ordered cleanup diagnostic."""

    lines: list[str] = []
    if primary_error is not None:
        lines.append(
            f"primary={type(primary_error).__name__}: "
            f"{_notebook_diagnostic_field(primary_error)}"
        )
    lines.append(f"cleanup={observation}")
    if forced_escalation:
        lines.append("cleanup-result=forced-escalation")
    if deadline_expired:
        lines.append("cleanup-result=cleanup-deadline")
    if secondary is not None:
        lines.append(
            f"cleanup-secondary={_notebook_diagnostic_field(secondary)}"
        )
    if not survivors:
        lines.append(f"scenario={scenario}")
    for survivor in sorted(survivors, key=_notebook_identity_key):
        requested = survivor.get("requested_stages", ())
        results = survivor.get("stage_results", ())
        if not isinstance(requested, (tuple, list)):
            requested = (requested,)
        if not isinstance(results, (tuple, list)):
            results = (results,)
        lines.extend(
            (
                "scenario="
                + _notebook_diagnostic_field(
                    survivor.get("scenario", scenario)
                ),
                " ".join(
                    (
                        "role="
                        + _notebook_diagnostic_field(
                            survivor.get("role", "unknown")
                        ),
                        "pid="
                        + _notebook_diagnostic_field(
                            survivor.get("pid", "unknown")
                        ),
                        "start="
                        + _notebook_diagnostic_field(
                            survivor.get("start_time", "unknown")
                        ),
                        "state="
                        + _notebook_diagnostic_field(
                            survivor.get("state", "unknown")
                        ),
                    )
                ),
                "requested_stages="
                + ",".join(_notebook_diagnostic_field(item) for item in requested),
                "stage_results="
                + ",".join(_notebook_diagnostic_field(item) for item in results),
                "authority_denied="
                + ("true" if survivor.get("authority_denied") else "false"),
            )
        )
    return "\n".join(lines)


def _notebook_cleanup_decision(
    *,
    scenario: str,
    primary_error: BaseException | None,
    forced_escalation: bool,
    observation: str,
    survivors: tuple[dict[str, object], ...],
    diagnostic: str | None,
    cleanup_started: float,
    observed_at: float,
) -> None:
    """Accept only a timely, non-escalated, authoritative empty owned set."""

    stable_survivors = tuple(survivors)
    overflow = len(stable_survivors) > _NOTEBOOK_CLEANUP_IDENTITY_LIMIT
    if overflow:
        observation = "incomplete(observation-overflow)"
        stable_survivors = ()
        diagnostic = None

    deadline_expired = (
        observed_at >= cleanup_started + _NOTEBOOK_CLEANUP_DECISION_SECONDS
    )
    successful = (
        primary_error is None
        and not forced_escalation
        and not deadline_expired
        and observation == "complete-empty"
        and not stable_survivors
    )
    if successful:
        return

    if diagnostic is None:
        rendered = _notebook_cleanup_diagnostic(
            scenario=scenario,
            primary_error=primary_error,
            forced_escalation=forced_escalation,
            observation=observation,
            survivors=stable_survivors,
            deadline_expired=deadline_expired,
        )
    else:
        rendered = diagnostic

    if len(rendered.encode("utf-8", errors="replace")) > _NOTEBOOK_CLEANUP_DIAGNOSTIC_BYTES:
        overflow_observation = "incomplete(observation-overflow)"
        rendered = _notebook_cleanup_diagnostic(
            scenario=scenario,
            primary_error=primary_error,
            forced_escalation=forced_escalation,
            observation=overflow_observation,
            survivors=(),
            deadline_expired=deadline_expired,
        )
    rendered = _notebook_bounded_text(
        rendered,
        _NOTEBOOK_CLEANUP_DIAGNOSTIC_BYTES,
    )
    error = CandidateError(rendered)
    if primary_error is not None:
        raise error from primary_error
    raise error


def _notebook_cleanup_lifecycle(
    *,
    scenario: str,
    primary_error: BaseException | None,
    observe: Callable[
        ...,
        tuple[str, tuple[dict[str, object], ...]],
    ],
    observe_identity: Callable[..., dict[str, object] | None],
    request_stage: Callable[..., str],
    wait: Callable[..., tuple[str, int | str | None]],
    monotonic: Callable[[], float],
) -> None:
    """Drive all owned-process actions through one bounded callback seam."""

    cleanup_started = monotonic()
    grace_deadline = cleanup_started + _NOTEBOOK_CLEANUP_GRACE_SECONDS
    decision_deadline = cleanup_started + _NOTEBOOK_CLEANUP_DECISION_SECONDS
    forced_escalation = False
    terminal = "incomplete(observer-unavailable)"
    sticky_terminal: str | None = None
    survivors: tuple[dict[str, object], ...] = ()
    cleanup_secondary: str | None = None
    action_authority_failed = False
    wait_acknowledged = False
    direct_wait_stage = "reap"
    action_history: dict[
        tuple[str, str, int, int],
        tuple[tuple[str, ...], tuple[str, ...], bool],
    ] = {}

    def ordered_for_action(
        identities: tuple[dict[str, object], ...],
    ) -> tuple[dict[str, object], ...]:
        return tuple(
            sorted(
                identities,
                key=lambda identity: (
                    identity.get("role") == "host",
                    _notebook_identity_key(identity),
                ),
            )
        )

    def make_sticky(value: str) -> None:
        nonlocal sticky_terminal, terminal
        terminal = value
        if sticky_terminal is None:
            sticky_terminal = value

    def callback_failed(error: BaseException) -> None:
        nonlocal cleanup_secondary
        if cleanup_secondary is None:
            cleanup_secondary = f"{type(error).__name__}: {error}"
        make_sticky("incomplete(cleanup-callback-error)")

    def observe_now(*, stage: str, timeout: float, post_ack: bool = False) -> bool:
        nonlocal terminal, survivors
        now = monotonic()
        if now >= decision_deadline:
            make_sticky(
                "incomplete(post-reap-observation-missing)"
                if post_ack
                else "incomplete(cleanup-deadline)"
            )
            return False
        try:
            result = observe(
                stage=stage,
                deadline=decision_deadline,
                timeout=min(max(0.0, timeout), decision_deadline - now),
            )
        except BaseException as error:
            callback_failed(error)
            return False
        if (
            not isinstance(result, tuple)
            or len(result) != 2
            or not isinstance(result[0], str)
            or not isinstance(result[1], tuple)
        ):
            make_sticky("incomplete(malformed-observation)")
            survivors = ()
            return True
        terminal, survivors = result
        if any(not isinstance(survivor, dict) for survivor in survivors):
            make_sticky("incomplete(malformed-observation)")
            survivors = ()
            return True
        enriched: list[dict[str, object]] = []
        for survivor in survivors:
            current = dict(survivor)
            history = action_history.get(_notebook_identity_key(current))
            if history is not None and not current.get("requested_stages"):
                requested, results, authority_denied = history
                current["requested_stages"] = requested
                current["stage_results"] = results
                current["authority_denied"] = authority_denied
            enriched.append(current)
        survivors = tuple(enriched)

        if len(survivors) > _NOTEBOOK_CLEANUP_IDENTITY_LIMIT:
            terminal = "incomplete(observation-overflow)"
            survivors = ()

        valid_observations = {
            "complete-empty",
            "complete-nonempty",
            "incomplete(authority-denied)",
            "incomplete(observer-unavailable)",
            "incomplete(observation-overflow)",
            "incomplete(malformed-observation)",
            "incomplete(stable-identity-mismatch)",
            "incomplete(cleanup-deadline)",
        }
        if (
            terminal not in valid_observations
            or (terminal == "complete-empty" and survivors)
            or (terminal == "complete-nonempty" and not survivors)
        ):
            terminal = "incomplete(malformed-observation)"
            survivors = ()
        if terminal.startswith("incomplete("):
            make_sticky(terminal)
        elif post_ack and terminal != "complete-empty":
            make_sticky(terminal)
        return True

    def request_for_all(stage: str) -> bool:
        nonlocal action_authority_failed, direct_wait_stage, survivors
        for expected in ordered_for_action(survivors):
            if monotonic() >= decision_deadline:
                action_authority_failed = True
                make_sticky("incomplete(cleanup-deadline)")
                return False
            try:
                observed = observe_identity(expected=expected)
            except PermissionError:
                action_authority_failed = True
                make_sticky("incomplete(authority-denied)")
                return False
            except (OSError, CandidateError):
                action_authority_failed = True
                make_sticky("incomplete(observer-unavailable)")
                return False
            except BaseException as error:
                action_authority_failed = True
                callback_failed(error)
                return False
            if observed is None:
                continue
            if not isinstance(observed, dict):
                action_authority_failed = True
                make_sticky("incomplete(observer-unavailable)")
                return False
            if observed.get("authority_denied") is True:
                action_authority_failed = True
                make_sticky("incomplete(authority-denied)")
                return False
            if not _notebook_owned_identity_matches(
                expected=expected,
                observed=observed,
            ):
                action_authority_failed = True
                make_sticky("incomplete(stable-identity-mismatch)")
                return False
            if not isinstance(observed.get("state"), str):
                action_authority_failed = True
                make_sticky("incomplete(observer-unavailable)")
                return False
            if observed["state"] == "Z":
                if expected.get("role") == "host":
                    direct_wait_stage = "reap"
                continue
            if monotonic() >= decision_deadline:
                action_authority_failed = True
                make_sticky("incomplete(cleanup-deadline)")
                return False
            if stage == "sigterm":
                direct_wait_stage = "graceful"
            try:
                result = request_stage(
                    stage=stage,
                    identity=expected,
                    deadline=decision_deadline,
                    monotonic=monotonic,
                )
            except BaseException as error:
                action_authority_failed = True
                callback_failed(error)
                return False
            suffixes = {
                "sent",
                "pending-reap",
                "not-found",
                "stable-identity-mismatch",
                "authority-denied",
                "observer-unavailable",
                "action-handle-unavailable",
                "cleanup-deadline",
            }
            valid_results = {f"{stage}={suffix}" for suffix in suffixes}
            if not isinstance(result, str) or result not in valid_results:
                action_authority_failed = True
                make_sticky("incomplete(malformed-action-result)")
                return False
            key = _notebook_identity_key(expected)
            previous = action_history.get(
                key,
                ((), (), bool(expected.get("authority_denied"))),
            )
            requested = previous[0]
            if not result.endswith("=pending-reap"):
                requested += (stage,)
            action_history[key] = (
                requested,
                previous[1] + (result,),
                previous[2] or result.endswith("=authority-denied"),
            )
            expected["requested_stages"] = action_history[key][0]
            expected["stage_results"] = action_history[key][1]
            expected["authority_denied"] = action_history[key][2]
            suffix = result.removeprefix(f"{stage}=")
            if stage == "sigterm" and expected.get("role") == "host":
                if suffix in ("pending-reap", "not-found"):
                    direct_wait_stage = "reap"
            sticky_action_results = {
                "stable-identity-mismatch": "incomplete(stable-identity-mismatch)",
                "authority-denied": "incomplete(authority-denied)",
                "observer-unavailable": "incomplete(observer-unavailable)",
                "action-handle-unavailable": "incomplete(action-handle-unavailable)",
                "cleanup-deadline": "incomplete(cleanup-deadline)",
            }
            if suffix in sticky_action_results:
                action_authority_failed = True
                make_sticky(sticky_action_results[suffix])
                return False
        return True

    def wait_once(*, stage: str, timeout: float) -> bool:
        nonlocal terminal, wait_acknowledged
        now = monotonic()
        if now >= decision_deadline:
            make_sticky("incomplete(cleanup-deadline)")
            return False
        bounded_timeout = min(max(0.0, timeout), decision_deadline - now)
        stage_stop = min(decision_deadline, now + bounded_timeout)
        try:
            result = wait(
                stage=stage,
                deadline=decision_deadline,
                timeout=bounded_timeout,
            )
        except BaseException as error:
            callback_failed(error)
            return False

        malformed = False
        if not isinstance(result, tuple) or len(result) != 2:
            malformed = True
            tag: object = None
            payload: object = None
        else:
            tag, payload = result
        allowed_incomplete = {
            "incomplete(authority-denied)",
            "incomplete(observer-unavailable)",
            "incomplete(observation-overflow)",
            "incomplete(malformed-observation)",
            "incomplete(stable-identity-mismatch)",
            "incomplete(cleanup-deadline)",
        }
        if not malformed and tag == "reaped-complete-empty":
            malformed = type(payload) is not int or payload not in (0, -15)
            if not malformed:
                wait_acknowledged = True
                terminal = "incomplete(post-reap-observation-missing)"
                observe_now(
                    stage=stage,
                    timeout=max(0.0, stage_stop - monotonic()),
                    post_ack=True,
                )
                return True
        elif not malformed and tag == "invalid-status":
            if type(payload) is int and payload != 0:
                make_sticky(f"incomplete(wait-invalid-status:{payload})")
                return False
            malformed = True
        elif not malformed and tag == "status-unavailable" and payload is None:
            make_sticky("incomplete(wait-status-unavailable)")
            return False
        elif not malformed and tag == "deadline-exhausted" and payload is None:
            make_sticky("incomplete(cleanup-deadline)")
            return False
        elif not malformed and tag == "host-still-running" and payload is None:
            make_sticky("incomplete(wait-host-still-running)")
            return False
        elif not malformed and tag == "owned-survivors" and payload is None:
            make_sticky("incomplete(wait-owned-survivors)")
            return False
        elif (
            not malformed
            and tag == "incomplete"
            and isinstance(payload, str)
            and payload in allowed_incomplete
        ):
            make_sticky(payload)
            return False
        else:
            malformed = True

        if malformed:
            make_sticky("incomplete(malformed-wait-result)")
        return False

    try:
        observe_now(
            stage="graceful",
            timeout=max(0.0, grace_deadline - monotonic()),
        )
        if terminal == "complete-nonempty" and survivors and sticky_terminal is None:
            request_for_all("sigterm")

        first_ack = wait_once(
            stage=direct_wait_stage,
            timeout=max(0.0, grace_deadline - monotonic()),
        )
        if not first_ack and monotonic() < decision_deadline:
            observe_now(
                stage="forced",
                timeout=max(0.0, decision_deadline - monotonic()),
            )

        live_survivors = tuple(
            survivor for survivor in survivors if survivor.get("state") != "Z"
        )
        if (
            terminal == "complete-nonempty"
            and live_survivors
            and not action_authority_failed
            and not wait_acknowledged
            and monotonic() < decision_deadline
        ):
            forced_escalation = True
            request_for_all("sigkill")
            wait_once(
                stage="forced",
                timeout=max(0.0, decision_deadline - monotonic()),
            )
        elif not wait_acknowledged and monotonic() < decision_deadline:
            observe_now(
                stage="final",
                timeout=max(0.0, decision_deadline - monotonic()),
            )

        if monotonic() >= decision_deadline and sticky_terminal is None:
            make_sticky("incomplete(cleanup-deadline)")
        if sticky_terminal is not None:
            terminal = sticky_terminal
        observed_at = monotonic()
    except BaseException as error:
        callback_failed(error)
        try:
            observed_at = monotonic()
        except BaseException:
            observed_at = decision_deadline

    diagnostic = None
    if cleanup_secondary is not None:
        diagnostic = _notebook_cleanup_diagnostic(
            scenario=scenario,
            primary_error=primary_error,
            forced_escalation=forced_escalation,
            observation=terminal,
            survivors=survivors,
            deadline_expired=observed_at >= decision_deadline,
            secondary=cleanup_secondary,
        )
    _notebook_cleanup_decision(
        scenario=scenario,
        primary_error=primary_error,
        forced_escalation=forced_escalation,
        observation=terminal,
        survivors=survivors,
        diagnostic=diagnostic,
        cleanup_started=cleanup_started,
        observed_at=observed_at,
    )


class _NotebookOwnedProcessObserver:
    """Linux observer for one causally launched, isolated host session."""

    def __init__(self, *, scenario: str, process: subprocess.Popen[str]) -> None:
        self.scenario = scenario
        self.process = process
        self.root_pid = process.pid
        self.initial_error: str | None = None
        try:
            root = self._read_process(self.root_pid)
        except PermissionError:
            root = None
            self.initial_error = "authority-denied"
        except CandidateError:
            root = None
            self.initial_error = "observer-unavailable"
        if root is None and self.initial_error is None:
            self.initial_error = "observer-unavailable"
        self.root_session = None if root is None else int(root["session"])
        self.isolated_session = (
            self.initial_error is None and self.root_session == self.root_pid
        )
        self.owned_sessions: dict[int, int] = {}
        if self.isolated_session and root is not None:
            self.owned_sessions[self.root_pid] = int(root["start_time"])
        self.known: dict[tuple[int, int], dict[str, object]] = {}
        self.last_survivors: tuple[dict[str, object], ...] = ()
        if root is not None:
            self._admit(root, role="host")

    def add_isolated_session(self, *, pid: int, start_time: int) -> None:
        """Admit a directly launched helper session by stable leader identity."""

        if pid <= 0 or start_time <= 0:
            self.initial_error = "observer-unavailable"
            return
        self.owned_sessions[pid] = start_time

    def mark_incomplete(self, reason: str) -> None:
        """Make a malformed direct-launch admission a fail-closed terminal."""

        self.initial_error = reason

    @staticmethod
    def _read_process(pid: int) -> dict[str, object] | None:
        try:
            record = (Path("/proc") / str(pid) / "stat").read_text(encoding="ascii")
        except (FileNotFoundError, ProcessLookupError):
            return None
        comm_end = record.rfind(")")
        if comm_end < 0:
            raise CandidateError(f"malformed Linux process identity for PID {pid}")
        fields = record[comm_end + 2 :].split()
        if len(fields) <= 19:
            raise CandidateError(f"incomplete Linux process identity for PID {pid}")
        try:
            return {
                "pid": pid,
                "state": fields[0],
                "ppid": int(fields[1]),
                "pgrp": int(fields[2]),
                "session": int(fields[3]),
                "start_time": int(fields[19]),
            }
        except ValueError as error:
            raise CandidateError(
                f"malformed Linux process identity for PID {pid}"
            ) from error

    def _admit(self, process: dict[str, object], *, role: str) -> None:
        key = (int(process["pid"]), int(process["start_time"]))
        existing = self.known.get(key)
        if existing is None:
            self.known[key] = {
                "scenario": self.scenario,
                "role": role,
                "pid": key[0],
                "start_time": key[1],
                "state": process["state"],
                "requested_stages": (),
                "stage_results": (),
                "authority_denied": False,
            }
        else:
            existing["state"] = process["state"]

    def _process_table(
        self, *, deadline: float | None
    ) -> tuple[dict[str, object], ...]:
        records: list[dict[str, object]] = []
        try:
            members = tuple(Path("/proc").iterdir())
        except PermissionError as error:
            raise PermissionError("Linux ownership observer is unavailable") from error
        for member in members:
            if deadline is not None and time.monotonic() >= deadline:
                raise TimeoutError("owned-process observation deadline exhausted")
            if not member.name.isdigit():
                continue
            try:
                record = self._read_process(int(member.name))
            except PermissionError as error:
                raise PermissionError(
                    f"Linux ownership observer lacks authority for PID {member.name}"
                ) from error
            if record is not None:
                records.append(record)
        return tuple(records)

    def observe(
        self,
        *,
        deadline: float | None = None,
    ) -> tuple[str, tuple[dict[str, object], ...]]:
        if self.initial_error is not None:
            return f"incomplete({self.initial_error})", self.last_survivors
        try:
            table = self._process_table(deadline=deadline)
        except TimeoutError:
            return "incomplete(cleanup-deadline)", self.last_survivors
        except PermissionError:
            survivors = tuple(self.known.values())
            self.last_survivors = survivors
            return "incomplete(authority-denied)", survivors
        except CandidateError:
            survivors = tuple(self.known.values())
            self.last_survivors = survivors
            return "incomplete(observer-unavailable)", survivors

        if self.isolated_session:
            by_pid = {int(process["pid"]): process for process in table}
            active_sessions: set[int] = set()
            for session, start_time in self.owned_sessions.items():
                leader = by_pid.get(session)
                if leader is not None and int(leader["start_time"]) != start_time:
                    continue
                active_sessions.add(session)
            for process in table:
                process_session = int(process["session"])
                if process_session in active_sessions:
                    self._admit(
                        process,
                        role=(
                            "host"
                            if int(process["pid"]) == self.root_pid
                            else (
                                "browser-helper"
                                if process_session != self.root_session
                                else "profile-helper"
                            )
                        ),
                    )
        else:
            descendants = {self.root_pid}
            changed = True
            while changed:
                changed = False
                for process in table:
                    pid = int(process["pid"])
                    if pid not in descendants and int(process["ppid"]) in descendants:
                        descendants.add(pid)
                        changed = True
            for process in table:
                if int(process["pid"]) in descendants:
                    self._admit(
                        process,
                        role=(
                            "host"
                            if int(process["pid"]) == self.root_pid
                            else "profile-helper"
                        ),
                    )

        live: list[dict[str, object]] = []
        authority_incomplete = False
        for key, identity in self.known.items():
            if deadline is not None and time.monotonic() >= deadline:
                return "incomplete(cleanup-deadline)", self.last_survivors
            try:
                current = self._read_process(key[0])
            except PermissionError:
                identity["state"] = "inaccessible"
                identity["authority_denied"] = True
                live.append(dict(identity))
                authority_incomplete = True
                continue
            if current is None or int(current["start_time"]) != key[1]:
                continue
            identity["state"] = current["state"]
            live.append(dict(identity))

        live.sort(key=_notebook_identity_key)
        self.last_survivors = tuple(live)
        if len(live) > _NOTEBOOK_CLEANUP_IDENTITY_LIMIT:
            return "incomplete(observation-overflow)", self.last_survivors
        if authority_incomplete:
            return "incomplete(authority-denied)", self.last_survivors
        if live:
            return "complete-nonempty", self.last_survivors
        return "complete-empty", ()

    def observe_identity(
        self,
        *,
        expected: dict[str, object],
    ) -> dict[str, object] | None:
        try:
            current = self._read_process(int(expected["pid"]))
        except PermissionError:
            observed = dict(expected)
            observed["start_time"] = None
            observed["state"] = "inaccessible"
            observed["authority_denied"] = True
            return observed
        if current is None:
            return None
        observed = dict(expected)
        observed["start_time"] = current["start_time"]
        observed["state"] = current["state"]
        return observed

    def request_stage(
        self,
        *,
        stage: str,
        identity: dict[str, object],
        deadline: float,
        monotonic: Callable[[], float],
    ) -> tuple[str, bool]:
        """Signal one exact known process through one action-local pidfd."""

        if stage not in ("sigterm", "sigkill"):
            raise CandidateError(f"unknown Notebook cleanup stage: {stage}")

        pid = identity.get("pid")
        start_time = identity.get("start_time")
        record = (
            self.known.get((pid, start_time))
            if type(pid) is int and type(start_time) is int
            else None
        )

        def finish(result: str, signal_accepted: bool) -> tuple[str, bool]:
            if record is not None:
                self.record_stage(
                    identity=record,
                    stage=stage,
                    result=result,
                    authority_denied=result.endswith("=authority-denied"),
                )
            return result, signal_accepted

        if record is None or any(
            type(identity.get(field)) is not expected_type
            or identity.get(field) != record.get(field)
            for field, expected_type in (
                ("scenario", str),
                ("role", str),
                ("pid", int),
                ("start_time", int),
            )
        ):
            return finish(f"{stage}=stable-identity-mismatch", False)
        if monotonic() >= deadline:
            return finish(f"{stage}=cleanup-deadline", False)

        pidfd_open = getattr(os, "pidfd_open", None)
        pidfd_send_signal = getattr(signal, "pidfd_send_signal", None)
        if not callable(pidfd_open) or not callable(pidfd_send_signal):
            return finish(f"{stage}=action-handle-unavailable", False)

        try:
            pidfd = pidfd_open(pid, 0)
        except ProcessLookupError:
            return finish(f"{stage}=not-found", False)
        except OSError:
            return finish(f"{stage}=action-handle-unavailable", False)

        result = f"{stage}=action-handle-unavailable"
        signal_accepted = False
        try:
            if monotonic() >= deadline:
                result = f"{stage}=cleanup-deadline"
            else:
                try:
                    observed = self.observe_identity(expected=dict(record))
                except PermissionError:
                    result = f"{stage}=authority-denied"
                except (OSError, CandidateError):
                    result = f"{stage}=observer-unavailable"
                else:
                    if observed is None:
                        result = f"{stage}=not-found"
                    elif not isinstance(observed, dict) or not isinstance(
                        observed.get("state"), str
                    ):
                        result = f"{stage}=observer-unavailable"
                    elif observed.get("authority_denied") is True:
                        result = f"{stage}=authority-denied"
                    elif any(
                        type(observed.get(field)) is not expected_type
                        for field, expected_type in (
                            ("scenario", str),
                            ("role", str),
                            ("pid", int),
                            ("start_time", int),
                        )
                    ):
                        result = f"{stage}=observer-unavailable"
                    elif not _notebook_owned_identity_matches(
                        expected=record,
                        observed=observed,
                    ):
                        result = f"{stage}=stable-identity-mismatch"
                    elif observed["state"] == "Z":
                        result = f"{stage}=pending-reap"
                    elif monotonic() >= deadline:
                        result = f"{stage}=cleanup-deadline"
                    else:
                        signum = (
                            signal.SIGTERM if stage == "sigterm" else signal.SIGKILL
                        )
                        try:
                            pidfd_send_signal(pidfd, signum, None, 0)
                        except ProcessLookupError:
                            result = f"{stage}=not-found"
                        except PermissionError:
                            result = f"{stage}=authority-denied"
                        except OSError:
                            result = f"{stage}=action-handle-unavailable"
                        else:
                            signal_accepted = True
                            result = f"{stage}=sent"
        finally:
            try:
                os.close(pidfd)
            except BaseException:
                result = f"{stage}=action-handle-unavailable"
        return finish(result, signal_accepted)

    def record_stage(
        self,
        *,
        identity: dict[str, object],
        stage: str,
        result: str,
        authority_denied: bool = False,
    ) -> None:
        key = (int(identity["pid"]), int(identity["start_time"]))
        record = self.known.get(key)
        if record is None:
            return
        if not result.endswith("=pending-reap"):
            record["requested_stages"] = tuple(record["requested_stages"]) + (stage,)
        record["stage_results"] = tuple(record["stage_results"]) + (result,)
        if authority_denied:
            record["authority_denied"] = True


def run_notebook_profile(
    *,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    workspace: candidate_profiles.ProfileWorkspace,
    config: DistributionConfig,
    receipt: dict[str, Any] | None = None,
    frontend: dict[str, Any] | None = None,
) -> list[str]:
    if receipt is None or frontend is None:
        raise CandidateError("Notebook profile requires its validated H2 observation")

    state: dict[str, Any] = {}

    def require_frontend_binding(name: str) -> None:
        if frontend["h2_receipt_sha256"] != hashlib.sha256(
            _h2_executor().canonical_json_bytes(receipt)
        ).hexdigest():
            raise CandidateError("Notebook frontend binding changed after H2")
        state[name] = True

    def install_notebook() -> None:
        python = candidate_profiles.install_environment(
            uv=uv,
            interpreter=interpreter,
            environment=workspace.environment,
            requirements=[
                f"{wheel}[gmsh,matplotlib,viewer]",
                config.pytest,
                "marimo==0.23.16",
            ],
            run=checked_run,
        )
        workspace.consumer.mkdir(parents=True)
        state["python"] = python

    def served_source_tree() -> Path:
        """Serve a proven copy so no host writes into the retained inputs."""

        served = state.get("served-source")
        if isinstance(served, Path):
            return served
        served = workspace.root / "served-source"
        shutil.copytree(extracted, served, symlinks=False)
        if source_tree_sha256(served) != source_tree_sha256(extracted):
            raise CandidateError("Notebook host tree is not an exact candidate copy")
        state["served-source"] = served
        return served

    def stage_single_file(source: Path, root: Path) -> Path:
        """Copy one reviewed input into an otherwise empty consumer."""

        if root.exists():
            raise CandidateError(f"Notebook consumer already exists: {root}")
        root.mkdir(parents=True)
        target = root / source.name
        shutil.copy2(source, target)
        members = tuple(root.iterdir())
        if (
            members != (target,)
            or target.is_symlink()
            or not target.is_file()
            or target.read_bytes() != source.read_bytes()
        ):
            raise CandidateError(f"Notebook consumer is not one exact file: {root}")
        return target

    def run_host(
        project: str,
        fixture: str,
        *,
        source_root: Path | None = None,
        test_spec: str | None = None,
        extra_environment: dict[str, str] | None = None,
    ) -> None:
        python = state.get("python")
        if not isinstance(python, Path):
            raise CandidateError("Notebook host ran before the exact Python environment")
        frontend_root = workspace.root / "frontend-host"
        if "host-environment" not in state:
            executor = _h2_executor()
            build_root = workspace.root / "host-build"
            build = executor.H2Workspace(
                root=build_root,
                home=build_root / "home",
                npm_cache=build_root / "npm-cache",
                temporary=build_root / "tmp",
                frontend=frontend_root,
                installation=frontend_root / "node_modules",
                output=frontend_root / "dist",
                browser_cache=build_root / "browser-cache",
            )
            for path in (build.root, build.home, build.npm_cache, build.temporary, build.browser_cache):
                path.mkdir(parents=True, exist_ok=False)
            executor.stage_frontend(extracted, build)
            acquired = executor.acquire_inputs(build)
            browser = receipt["browser"]
            if (
                acquired.browser_archive_sha256 != browser["downloaded_archive_sha256"]
                or acquired.browser_executable_sha256 != browser["executable_sha256"]
                or acquired.browser_platform != browser["platform"]
                or executor.structured_sha256(acquired.python_wheels)
                != receipt["python_host"]["resolved_environment_sha256"]
            ):
                raise CandidateError("Notebook host inputs differ from the accepted H2 receipt")
            environment = {
                "HOME": str(build.home),
                "LANG": "C.UTF-8",
                "LC_ALL": "C.UTF-8",
                "PATH": os.pathsep.join((str(acquired.npm.parent), str(acquired.node.parent), os.environ.get("PATH", ""))),
                "TMPDIR": str(build.temporary),
                "TZ": "UTC",
                "npm_config_cache": str(build.npm_cache),
                "PLAYWRIGHT_BROWSERS_PATH": str(build.browser_cache),
            }
            checked_run(
                [str(acquired.npm), "ci", "--ignore-scripts"],
                cwd=frontend_root,
                extra_environment=environment,
            )
            state["host-environment"] = environment
            state["browser-executable"] = acquired.browser_executable
            state["npm-executable"] = acquired.npm
        environment = dict(state["host-environment"])
        host_environment = os.environ.copy()
        host_environment.update(environment)
        host_environment["EQIORA_GMSH"] = str(
            python.parent / ("gmsh.exe" if os.name == "nt" else "gmsh")
        )
        host_environment["PATH"] = os.pathsep.join(
            (str(python.parent), environment["PATH"])
        )
        served = served_source_tree() if source_root is None else source_root
        port = _reserve_loopback_port()
        if project != "marimo-0.23.16":
            raise CandidateError(f"unknown Notebook host project: {project}")
        argv = [
            str(python), "-I", "-m", "marimo", "run", str(served / fixture),
            "--host", "127.0.0.1", "--port", str(port), "--headless",
        ]
        url_variable = "EQIORA_MARIMO_URL"
        url_value = f"http://127.0.0.1:{port}/"
        process = subprocess.Popen(
            argv,
            cwd=served,
            env=host_environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        observer = _NotebookOwnedProcessObserver(
            scenario=project,
            process=process,
        )
        sent_sigterm = False
        primary_error: BaseException | None = None

        def observe_owned(
            *,
            stage: str,
            deadline: float,
            timeout: float,
        ) -> tuple[str, tuple[dict[str, object], ...]]:
            del stage
            if timeout <= 0.0 and time.monotonic() >= deadline:
                return "incomplete(cleanup-deadline)", observer.last_survivors
            return observer.observe(deadline=deadline)

        def observe_owned_identity(
            *,
            expected: dict[str, object],
        ) -> dict[str, object] | None:
            return observer.observe_identity(expected=expected)

        def request_owned_stage(
            *,
            stage: str,
            identity: dict[str, object],
            deadline: float,
            monotonic: Callable[[], float],
        ) -> str:
            nonlocal sent_sigterm
            action = observer.request_stage(
                stage=stage,
                identity=identity,
                deadline=deadline,
                monotonic=monotonic,
            )
            if (
                not isinstance(action, tuple)
                or len(action) != 2
                or not isinstance(action[0], str)
                or type(action[1]) is not bool
            ):
                return "malformed-action-result"
            result, signal_accepted = action
            pid = identity.get("pid")
            start_time = identity.get("start_time")
            direct_record = (
                observer.known.get((pid, start_time))
                if type(pid) is int and type(start_time) is int
                else None
            )
            if (
                stage == "sigterm"
                and signal_accepted is True
                and pid == process.pid
                and direct_record is not None
                and direct_record.get("role") == "host"
                and _notebook_owned_identity_matches(
                    expected=direct_record,
                    observed=identity,
                )
            ):
                sent_sigterm = True
            return result

        def wait_owned(
            *,
            stage: str,
            deadline: float,
            timeout: float,
        ) -> tuple[str, int | str | None]:
            if stage not in ("reap", "graceful", "forced"):
                raise CandidateError(f"unknown Notebook wait stage: {stage}")
            wait_started = time.monotonic()
            if wait_started >= deadline:
                return "deadline-exhausted", None
            stop = min(deadline, wait_started + max(0.0, timeout))
            remaining = max(0.0, stop - wait_started)
            try:
                status = process.wait(timeout=remaining)
            except subprocess.TimeoutExpired:
                if time.monotonic() >= deadline:
                    return "deadline-exhausted", None
                return "host-still-running", None
            except OSError:
                return "status-unavailable", None
            if type(status) is not int:
                return "status-unavailable", None
            if status != 0 and not (sent_sigterm and status == -signal.SIGTERM):
                return "invalid-status", status
            while True:
                now = time.monotonic()
                if now >= deadline:
                    return "deadline-exhausted", None
                if now >= stop:
                    return "owned-survivors", None
                terminal, survivors = observer.observe(deadline=deadline)
                if time.monotonic() >= deadline:
                    return "deadline-exhausted", None
                if not isinstance(terminal, str) or not isinstance(survivors, tuple):
                    return "incomplete", "incomplete(malformed-observation)"
                if terminal == "complete-empty" and not survivors:
                    return "reaped-complete-empty", status
                if terminal.startswith("incomplete("):
                    return "incomplete", terminal
                if terminal != "complete-nonempty" or not survivors:
                    return "incomplete", "incomplete(malformed-observation)"
                now = time.monotonic()
                if now >= deadline:
                    return "deadline-exhausted", None
                sleep_for = min(0.01, max(0.0, stop - now))
                if sleep_for <= 0.0:
                    return "owned-survivors", None
                time.sleep(sleep_for)

        try:
            for _ in range(120):
                readiness_status = process.poll()
                if readiness_status is not None:
                    raise CandidateError(
                        "Notebook host exited before readiness: "
                        f"status={readiness_status}"
                    )
                try:
                    with socket.create_connection(("127.0.0.1", port), timeout=0.25):
                        break
                except OSError:
                    time.sleep(0.25)
            else:
                raise CandidateError("Notebook host did not become ready")
            environment[url_variable] = url_value
            if extra_environment is not None:
                environment.update(extra_environment)
            host_command = [
                "npm",
                "run",
                "test:hosts",
                "--",
                f"--project={project}",
            ]
            if test_spec is not None:
                host_command.append(test_spec)
            launch_number = int(state.get("host-launch-number", 0)) + 1
            state["host-launch-number"] = launch_number
            launcher_root = workspace.root / f"host-launch-{launch_number}"
            launcher_root.mkdir()
            launcher = launcher_root / "npm"
            launcher_identity = launcher_root / "session.identity"
            npm_executable = Path(state["npm-executable"])
            launcher.write_text(
                "#!/usr/bin/env python3\n"
                "import os\n"
                "import sys\n"
                "\n"
                "os.setsid()\n"
                "record = open(f'/proc/{os.getpid()}/stat', "
                "encoding='ascii').read()\n"
                "fields = record[record.rfind(')') + 2:].split()\n"
                f"identity = {str(launcher_identity)!r}\n"
                "with open(identity, 'x', encoding='ascii') as stream:\n"
                "    stream.write(f'{os.getpid()} {int(fields[19])}\\n')\n"
                f"os.environ['PATH'] = {environment['PATH']!r}\n"
                f"executable = {str(npm_executable)!r}\n"
                "os.execv(executable, [executable, *sys.argv[1:]])\n",
                encoding="utf-8",
            )
            launcher.chmod(0o700)
            host_test_environment = dict(environment)
            host_test_environment["PATH"] = os.pathsep.join(
                (str(launcher_root), environment["PATH"])
            )
            try:
                checked_run(
                    host_command,
                    cwd=frontend_root,
                    extra_environment=host_test_environment,
                )
            finally:
                if launcher_identity.is_file():
                    try:
                        values = launcher_identity.read_text(
                            encoding="ascii"
                        ).split()
                        if len(values) != 2:
                            raise ValueError("expected PID and Linux start time")
                        observer.add_isolated_session(
                            pid=int(values[0]),
                            start_time=int(values[1]),
                        )
                    except (OSError, ValueError):
                        observer.mark_incomplete("observer-unavailable")
        except BaseException as error:
            primary_error = error

        status_before_cleanup = process.poll()
        if (
            primary_error is None
            and status_before_cleanup is not None
            and status_before_cleanup != 0
        ):
            primary_error = CandidateError(
                "Notebook host shutdown was not clean: "
                f"unsolicited status={status_before_cleanup}"
            )
        try:
            _notebook_cleanup_lifecycle(
                scenario=project,
                primary_error=primary_error,
                observe=observe_owned,
                observe_identity=observe_owned_identity,
                request_stage=request_owned_stage,
                wait=wait_owned,
                monotonic=time.monotonic,
            )
        finally:
            if process.stdout is not None:
                process.stdout.close()
        state[project] = True

    def run_exact_cylinder_stokes_marimo() -> None:
        python = state.get("python")
        if not isinstance(python, Path):
            raise CandidateError(
                "Exact-cylinder Stokes Marimo app ran before the Python environment"
            )

        positive_app = stage_single_file(
            extracted / EXACT_CYLINDER_STOKES_MARIMO_APP,
            workspace.root / "exact-cylinder-stokes-marimo-positive",
        )
        run_host(
            "marimo-0.23.16",
            positive_app.name,
            source_root=positive_app.parent,
            test_spec="tests/exact-cylinder-stokes-marimo.spec.ts",
            extra_environment={EXACT_CYLINDER_STOKES_MARIMO_ORACLE_FLAG: "1"},
        )

        negative_app = stage_single_file(
            extracted / EXACT_CYLINDER_STOKES_MARIMO_MUTANT,
            workspace.root / "exact-cylinder-stokes-marimo-negative",
        )
        try:
            output = checked_run(
                [str(python), "-I", str(negative_app)],
                cwd=negative_app.parent,
                capture=True,
            )
        except subprocess.CalledProcessError as error:
            output = str(error.output or "")
            if EXACT_CYLINDER_STOKES_MARIMO_MUTANT_FAILURE not in output:
                raise CandidateError(
                    "Exact-cylinder Stokes repository-helper mutant failed at "
                    "an unrelated boundary"
                ) from error
        else:
            raise CandidateError(
                "Exact-cylinder Stokes repository helper unexpectedly resolved: "
                f"{output}"
            )

    def require_host_observation(name: str) -> None:
        if not state.get("marimo-0.23.16"):
            raise CandidateError(f"Notebook host observation is incomplete: {name}")
        if name == "browser" and not Path(state["browser-executable"]).is_file():
            raise CandidateError("accepted managed Chromium executable is missing")

    def run_shared_semantic_viewer_marimo() -> None:
        python = state.get("python")
        if not isinstance(python, Path):
            raise CandidateError(
                "Shared semantic viewer Marimo app ran before the Python environment"
            )
        app = stage_single_file(
            extracted / SHARED_SEMANTIC_VIEWER_MARIMO_APP,
            workspace.root / "shared-semantic-viewer-marimo-positive",
        )
        run_host(
            "marimo-0.23.16",
            app.name,
            source_root=app.parent,
            test_spec="tests/shared-semantic-viewer-marimo.spec.ts",
            extra_environment={SHARED_SEMANTIC_VIEWER_MARIMO_ORACLE_FLAG: "1"},
        )

    def run_exact_cylinder_profile() -> None:
        install_notebook()
        run_exact_cylinder_stokes_marimo()

    observations = (
        ("frontend:lock-integrity", lambda: require_frontend_binding("lock")),
        ("frontend:dependency-inventory", lambda: require_frontend_binding("dependencies")),
        (EXACT_CYLINDER_STOKES_MARIMO_CHECK, run_exact_cylinder_profile),
        (SHARED_SEMANTIC_VIEWER_MARIMO_CHECK, run_shared_semantic_viewer_marimo),
        ("cp313:notebook-managed-chromium-r1234", lambda: require_host_observation("browser")),
        ("cp313:notebook-no-external-network", lambda: require_host_observation("network")),
        ("cp313:notebook-cleanup-and-mutation", lambda: require_host_observation("cleanup")),
    )
    emitted: list[str] = []
    return list(candidate_profiles.run_notebook_profile(observations, emit=emitted.append))


def execute_profile(
    workspace: candidate_profiles.ProfileWorkspace,
    *,
    uv: str,
    config: DistributionConfig,
    wheels: dict[str, Path],
    extracted: Path,
    interpreters: dict[str, str],
    receipt: dict[str, Any] | None = None,
    frontend: dict[str, Any] | None = None,
) -> candidate_profiles.ProfileReceipt:
    """Execute one profile solely within its pre-declared writable root."""

    workspace.root.mkdir(parents=True)
    workspace.temporary.mkdir()
    if workspace.matplotlib_config is not None:
        workspace.matplotlib_config.mkdir()

    name = workspace.name
    checks: list[str]
    dependency_profiles: tuple[tuple[str, tuple[tuple[str, str], ...]], ...] = ()
    with command_context(
        environment=dict(workspace.environment_variables),
        log=workspace.log,
    ):
        if name.startswith("base-"):
            python_version = name.removeprefix("base-")
            checks = run_base_profile(
                uv=uv,
                interpreter=interpreters[python_version],
                python_version=python_version,
                wheel=wheels[python_version],
                extracted=extracted,
                workspace=workspace,
                config=config,
            )
        elif name == "numpy-floor-3.12":
            checks, numpy_floor = run_numpy_floor_profile(
                uv=uv,
                interpreter=interpreters[config.numpy_floor_interpreter],
                wheel=wheels[config.numpy_floor_interpreter],
                extracted=extracted,
                workspace=workspace,
                config=config,
            )
            dependency_profiles = (("numpy_floor", tuple(sorted(numpy_floor.items()))),)
        elif name == "generated-public-api":
            checked_run(
                [
                    interpreters[config.extras_interpreter],
                    "-I",
                    str(extracted / "tools/docs/generate_python_api.py"),
                    "--check",
                ],
                cwd=extracted,
            )
            checks = ["check:generated-public-api"]
        elif name == "notebook-3.13":
            checks = run_notebook_profile(
                uv=uv,
                interpreter=interpreters[config.extras_interpreter],
                wheel=wheels[config.extras_interpreter],
                extracted=extracted,
                workspace=workspace,
                config=config,
                receipt=receipt,
                frontend=frontend,
            )
        elif name in {"torch-3.13", "jax-3.13", "matplotlib-3.13"}:
            profile = name.removesuffix("-3.13")
            checks = run_optional_profile(
                name=profile,
                uv=uv,
                interpreter=interpreters[config.extras_interpreter],
                wheel=wheels[config.extras_interpreter],
                extracted=extracted,
                workspace=workspace,
                config=config,
            )
        elif name == "typing-3.13":
            checks = [
                run_full_typing_profile(
                    uv=uv,
                    interpreter=interpreters[config.extras_interpreter],
                    wheel=wheels[config.extras_interpreter],
                    extracted=extracted,
                    workspace=workspace,
                    config=config,
                )
            ]
        else:  # pragma: no cover - plan construction owns the closed set
            raise CandidateError(f"unknown candidate profile: {name}")

    log = workspace.log.read_text(encoding="utf-8") if workspace.log.is_file() else ""
    return candidate_profiles.ProfileReceipt(
        name=name,
        checks=tuple(checks),
        dependency_profiles=dependency_profiles,
        diagnostics=(),
        log=log,
    )


def replay_profile_logs(
    plan: tuple[candidate_profiles.ProfileWorkspace, ...],
) -> None:
    """Replay joined profile logs in frozen logical order."""

    for workspace in plan:
        if not workspace.log.is_file():
            continue
        print(f"=== Python candidate profile: {workspace.name} ===", flush=True)
        payload = workspace.log.read_text(encoding="utf-8")
        if payload:
            print(payload, end="" if payload.endswith("\n") else "\n", flush=True)


def tool_version(argv: list[str]) -> str:
    """Read one single-line tool version."""

    return checked_run(argv, capture=True).splitlines()[0]


def write_manifest(
    *,
    output: Path,
    source: SourceIdentity,
    sdist: Path,
    version: str,
    wheel_records: list[dict[str, Any]],
    checks: list[str],
    config: DistributionConfig,
    uv: str,
    complete_profiles: bool,
    dependency_profiles: dict[str, dict[str, str]],
    frontend: dict[str, Any] | None = None,
) -> Path:
    """Write deterministic provenance for the accepted artifact set."""

    manifest_checks = [
        "generated-public-api" if check == "check:generated-public-api" else check
        for check in checks
    ]
    artifacts = [
        {
            "filename": sdist.name,
            "kind": "sdist",
            "size": sdist.stat().st_size,
            "sha256": sha256(sdist),
        },
        *sorted(wheel_records, key=lambda record: record["python"]),
    ]
    manifest = {
        "format": MANIFEST_FORMAT,
        "project": "eqiora",
        "version": version,
        "acceptance": "complete" if complete_profiles else "development",
        "source": {
            "commit": source.commit,
            "expected_tag": config.expected_tag,
            "tags": list(source.tags),
            "tree": "clean",
        },
        "build": {
            "sdist_rebuilt": True,
            "wheel_family": {
                "implementation": "CPython",
                "ordinary_gil": True,
                "versions": list(config.interpreters),
                "platform": config.wheel_platform,
                "abi3": False,
            },
            "tools": {
                "maturin": config.maturin,
                "uv": tool_version([uv, "--version"]),
                "rustc": tool_version(["rustc", f"+{config.rust}", "--version"]),
                "cargo": tool_version(["cargo", f"+{config.rust}", "--version"]),
                "pytest": config.pytest,
                "mypy": config.mypy,
                "twine": config.twine,
            },
            "dependency_profiles": dependency_profiles,
        },
        "artifacts": artifacts,
        "checks": sorted(manifest_checks),
        "nonclaims": [
            "reproducible-build-certification",
            "artifact-signature",
            "macos-or-windows",
            "abi3",
            "free-threaded-cpython",
            "production-pypi-publication",
        ],
    }
    if frontend is not None:
        manifest["build"]["frontend"] = frontend
    path = output / f"eqiora-{version}-python-candidate.json"
    path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return path


def _h2_executor() -> Any:
    """Import the independently owned executor lazily to avoid its back-reference."""

    import python_candidate_h2

    return python_candidate_h2


def family_inventory(directory: Path) -> tuple[dict[str, object], ...]:
    return _h2_executor().family_inventory(directory)


def admit_candidate_family(directory: Path) -> Any:
    return _h2_executor().admit_candidate_family(directory)


def _admit_producer_return(
    output: Path,
    *,
    sdist: Path,
    wheels: dict[str, Path],
    config: DistributionConfig,
) -> tuple[dict[str, object], ...]:
    family = admit_candidate_family(output)
    expected_wheels = tuple(wheels[version] for version in config.interpreters)
    if family.sdist != sdist or family.wheels != expected_wheels:
        raise CandidateError("producer return differs from the admitted exact family")
    return family.inventory


def validate_h2_receipt(
    path: Path,
    *,
    expected_commit: str,
    family: Any,
) -> dict[str, Any]:
    """Validate canonical H2 bytes and their direct source/family binding."""

    try:
        raw = path.read_bytes()
        receipt = json.loads(raw)
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise CandidateError(f"cannot read detached H2 receipt: {error}") from error
    executor = _h2_executor()
    if raw != executor.canonical_json_bytes(receipt):
        raise CandidateError("detached H2 receipt is not canonical JSON")
    executor.validate_h2_receipt(receipt)
    candidate = receipt["candidate"]
    probe = receipt["probe"]
    if (
        candidate["source_commit"] != expected_commit
        or probe["writer_revision"] != expected_commit
        or candidate["version"] != family.version
        or tuple(candidate["artifacts"]) != family.inventory
    ):
        raise CandidateError("detached H2 receipt belongs to another source/family")
    expected_name = f"eqiora-{family.version}-python-candidate-h2.json"
    if path.name != expected_name:
        raise CandidateError("detached H2 receipt filename differs from its family")
    return receipt


def derive_frontend_manifest(
    *,
    family: Any,
    h2_receipt: Path,
    receipt: dict[str, Any],
) -> dict[str, Any]:
    """Derive the closed v3 frontend record from retained bytes and H2 observations."""

    executor = _h2_executor()
    scratch_parent = home_scratch_parent("python-candidate-finalize-frontend")
    with tempfile.TemporaryDirectory(
        prefix="eqiora-finalize-frontend-", dir=scratch_parent
    ) as temporary:
        extracted = safe_extract_sdist(family.sdist, Path(temporary) / "source")
        frontend_root = extracted / "bindings/python/frontend"
        package = frontend_root / "package.json"
        lock = frontend_root / "package-lock.json"
        if not package.is_file() or not lock.is_file():
            raise CandidateError("retained sdist omits the frozen frontend package/lock")
        package_sha256 = sha256(package)
        lock_sha256 = sha256(lock)

    inputs = receipt["inputs"]
    scripts = [
        {
            "lock_path": item["lock_path"],
            "name": item["name"],
            "version": item["version"],
            "lifecycle_scripts": item["lifecycle_scripts"],
        }
        for item in inputs["locked_packages"]
    ]
    browser = receipt["browser"]
    python_host = receipt["python_host"]
    return {
        "node": "v24.18.1",
        "npm": "11.16.0",
        "h2_receipt_sha256": hashlib.sha256(h2_receipt.read_bytes()).hexdigest(),
        "package_json_sha256": package_sha256,
        "package_lock_sha256": lock_sha256,
        "source_inventory_sha256": executor.structured_sha256(
            inputs["source_root_inventory"]
        ),
        "config_inventory_sha256": executor.structured_sha256(
            inputs["config_inventory"]
        ),
        "locked_packages_sha256": executor.structured_sha256(
            inputs["locked_packages"]
        ),
        "install_script_inventory_sha256": executor.structured_sha256(scripts),
        "node_executable_sha256": NODE_EXECUTABLE_SHA256,
        "npm_package_integrity": NPM_PACKAGE_INTEGRITY,
        "runtime": {
            "python": "3.13",
            "marimo": "0.23.16",
            "resolved_environment_sha256": python_host["resolved_environment_sha256"],
        },
        "browser": {
            "playwright": "1.62.1",
            "chromium_revision": "1234",
            "browser_version": "151.0.7922.34",
            "browsers_json_sha256": BROWSERS_JSON_SHA256,
            "platform": browser["platform"],
            "downloaded_archive_sha256": browser["downloaded_archive_sha256"],
            "executable_sha256": browser["executable_sha256"],
        },
    }


def _require_candidate_host(output: Path) -> None:
    if platform.system() != "Linux" or platform.machine().lower() not in {
        "x86_64",
        "amd64",
    }:
        raise CandidateError("the first Python candidate builder requires Linux x86-64")
    if output == ROOT or output.is_relative_to(ROOT):
        raise CandidateError("candidate output must remain outside the source tree")


def _require_expected_source(expected_commit: str) -> SourceIdentity:
    if GIT_SHA.fullmatch(expected_commit) is None:
        raise CandidateError("expected commit must be one full lowercase revision")
    source = source_identity()
    if source.commit != expected_commit:
        raise CandidateError("clean source commit differs from the expected revision")
    return source


def prepare_candidate(
    *,
    expected_commit: str,
    out: Path,
    require_tag: bool,
) -> Path:
    """Build one immutable sdist/four-wheel family without acceptance metadata."""

    output = out.resolve()
    _require_candidate_host(output)
    source = _require_expected_source(expected_commit)
    config = load_config()
    if require_tag:
        require_annotated_expected_tag(source, config.expected_tag)
    if output.exists() and any(output.iterdir()):
        raise CandidateError("candidate family output directory must be empty")
    output.mkdir(parents=True, exist_ok=True)
    uv = ensure_exact_uv(config.uv)
    require_executable("git")
    require_executable("rustc")
    require_executable("cargo")
    rustup = require_executable("rustup")
    scratch_parent = home_scratch_parent("python-candidate-prepare")
    with tempfile.TemporaryDirectory(
        prefix="eqiora-python-prepare-", dir=scratch_parent
    ) as temporary:
        scratch = Path(temporary)
        candidate_temporary = scratch / "tmp"
        candidate_temporary.mkdir()
        with command_context(environment={"TMPDIR": str(candidate_temporary)}):
            checked_run(
                [rustup, "toolchain", "install", config.rust, "--profile", "minimal"]
            )
            interpreters = {
                version: uv_interpreter(uv, version) for version in config.interpreters
            }
            sdist, wheels, extracted = build_artifacts(
                output=output,
                scratch=scratch,
                config=config,
                uv=uv,
                interpreters=interpreters,
            )
            producer_inventory = _admit_producer_return(
                output,
                sdist=sdist,
                wheels=wheels,
                config=config,
            )
            checked_run(
                [
                    uv,
                    "tool",
                    "run",
                    "--from",
                    config.twine,
                    "twine",
                    "check",
                    "--strict",
                    str(sdist),
                    *(str(wheels[version]) for version in config.interpreters),
                ]
            )
            license_bytes = (extracted / "LICENSE").read_bytes()
            notice_bytes = (extracted / "NOTICE").read_bytes()
            versions = {
                inspect_wheel(
                    wheels[python_version],
                    python_version=python_version,
                    config=config,
                    license_bytes=license_bytes,
                    notice_bytes=notice_bytes,
                )[0]
                for python_version in config.interpreters
            }
            if versions != {config.python_version}:
                raise CandidateError("prepared wheel family version drifted")
    admitted = admit_candidate_family(output)
    if admitted.version != config.python_version:
        raise CandidateError("prepared family identity differs from Cargo")
    if admitted.inventory != producer_inventory:
        raise CandidateError("prepared family differs from the producer return")
    for path in output.iterdir():
        path.chmod(0o444)
    output.chmod(0o555)
    return output


def run_candidate_profiles(
    *,
    family: Any,
    receipt: dict[str, Any],
    frontend: dict[str, Any],
) -> CandidateProfileSummary:
    """Run every ordinary and Notebook profile against one retained family."""

    config = load_config()
    uv = ensure_exact_uv(config.uv)
    require_executable("rustc")
    require_executable("cargo")
    scratch_parent = home_scratch_parent("python-candidate-finalize-profiles")
    with tempfile.TemporaryDirectory(
        prefix="eqiora-python-finalize-", dir=scratch_parent
    ) as temporary:
        scratch = Path(temporary)
        extracted = safe_extract_sdist(family.sdist, scratch / "source")
        wheels: dict[str, Path] = {}
        for version in config.interpreters:
            compact = version.replace(".", "")
            expected_name = (
                f"eqiora-{family.version}-cp{compact}-cp{compact}-"
                "manylinux_2_17_x86_64.manylinux2014_x86_64.whl"
            )
            matches = [
                path
                for path in family.wheels
                if path.name == expected_name
            ]
            if len(matches) != 1:
                raise CandidateError(f"retained family omits exact CPython {version} wheel")
            wheels[version] = matches[0]
        interpreters = {
            version: uv_interpreter(uv, version) for version in config.interpreters
        }
        checked_run(
            [
                uv,
                "tool",
                "run",
                "--from",
                config.twine,
                "twine",
                "check",
                "--strict",
                str(family.sdist),
                *(str(wheels[version]) for version in config.interpreters),
            ]
        )
        license_bytes = (extracted / "LICENSE").read_bytes()
        notice_bytes = (extracted / "NOTICE").read_bytes()
        records: list[dict[str, Any]] = []
        versions: set[str] = set()
        for version in config.interpreters:
            wheel_version, record = inspect_wheel(
                wheels[version],
                python_version=version,
                config=config,
                license_bytes=license_bytes,
                notice_bytes=notice_bytes,
            )
            versions.add(wheel_version)
            records.append(record)
        if versions != {family.version} or family.version != config.python_version:
            raise CandidateError("retained wheel versions disagree with the family")
        initial_identity = candidate_payload_identity(family.sdist, wheels, extracted)
        plan = candidate_profiles.build_profile_plan(scratch, config, skip_extras=False)
        outcomes = candidate_profiles.run_profile_tasks(
            plan,
            lambda workspace: execute_profile(
                workspace,
                uv=uv,
                config=config,
                wheels=wheels,
                extracted=extracted,
                interpreters=interpreters,
                receipt=receipt,
                frontend=frontend,
            ),
        )
        replay_profile_logs(plan)
        by_name = {outcome.name: outcome for outcome in outcomes}
        failures = tuple(
            (workspace.name, by_name[workspace.name].error)
            for workspace in plan
            if by_name[workspace.name].error is not None
        )
        if failures:
            diagnostics = "\n".join(f"- {name}: {error}" for name, error in failures)
            raise CandidateError(f"candidate validation profiles failed:\n{diagnostics}")
        if candidate_payload_identity(family.sdist, wheels, extracted) != initial_identity:
            raise CandidateError("candidate profiles mutated retained artifact inputs")
        receipts = tuple(outcome.value for outcome in outcomes if outcome.value is not None)
        try:
            merged = candidate_profiles.merge_profile_receipts(
                candidate_profiles.COMPLETE_PROFILE_NAMES,
                receipts,
            )
        except ValueError as error:
            raise CandidateError(f"candidate profile receipts are invalid: {error}") from error
        dependency_profiles = {
            name: dict(values) for name, values in merged.dependency_profiles
        }
        return CandidateProfileSummary(
            config=config,
            uv=uv,
            wheel_records=tuple(records),
            checks=("twine-strict", "sdist-to-wheel-rebuild", *merged.checks),
            dependency_profiles=dependency_profiles,
        )


def _family_version_from_sdist(sdist: Path) -> str:
    prefix = "eqiora-"
    suffix = ".tar.gz"
    if not sdist.name.startswith(prefix) or not sdist.name.endswith(suffix):
        raise CandidateError("candidate sdist filename is malformed")
    version = sdist.name[len(prefix) : -len(suffix)]
    if not version:
        raise CandidateError("candidate sdist version is empty")
    return version


def _admit_candidate_profile_summary(
    profiles: object,
    *,
    artifact_root: Path,
    family: Any,
    entry_inventory: tuple[dict[str, object], ...],
) -> CandidateProfileSummary:
    """Admit one complete profile-owner result before publication side effects."""

    if not isinstance(profiles, CandidateProfileSummary):
        raise CandidateError(
            "candidate profile owner did not return CandidateProfileSummary"
        )

    try:
        current_inventory = family_inventory(artifact_root)
    except (CandidateError, OSError) as error:
        raise CandidateError("candidate family changed during finalization") from error
    if current_inventory != entry_inventory or family.inventory != entry_inventory:
        raise CandidateError("candidate family changed during finalization")

    try:
        sdist = family.sdist
        wheels = tuple(family.wheels)
        version = family.version
        family_paths = tuple(artifact_root.iterdir())
    except (AttributeError, OSError, TypeError) as error:
        raise CandidateError(
            "candidate family is not the exact one-sdist/four-wheel set"
        ) from error
    if (
        not isinstance(sdist, Path)
        or not isinstance(version, str)
        or not version
        or len(wheels) != 4
        or len(family_paths) != 5
        or set(family_paths) != {sdist, *wheels}
        or sdist.name != f"eqiora-{version}.tar.gz"
        or any(
            not isinstance(wheel, Path) or wheel.suffix != ".whl" for wheel in wheels
        )
    ):
        raise CandidateError(
            "candidate family is not the exact one-sdist/four-wheel set"
        )

    try:
        config = profiles.config
        uv = profiles.uv
        reviewed = load_config()
    except (AttributeError, CandidateError, OSError, TypeError, ValueError) as error:
        raise CandidateError(
            "candidate profile summary has invalid configuration"
        ) from error
    if (
        not isinstance(config, DistributionConfig)
        or config != reviewed
        or config.python_version != version
        or config.interpreters != ("3.11", "3.12", "3.13", "3.14")
        or config.wheel_platform != "manylinux_2_17_x86_64"
        or not isinstance(uv, str)
        or not uv
    ):
        raise CandidateError("candidate profile summary has invalid configuration")

    record_members = {
        "filename",
        "kind",
        "python",
        "abi",
        "platform",
        "size",
        "sha256",
    }
    expected_by_python = dict(zip(config.interpreters, wheels, strict=True))
    seen_python: set[str] = set()
    try:
        records = profiles.wheel_records
        valid_records = isinstance(records, tuple) and len(records) == 4
        if valid_records:
            for record in records:
                if not isinstance(record, dict) or set(record) != record_members:
                    valid_records = False
                    break
                python = record.get("python")
                wheel = expected_by_python.get(python)
                compact = python.replace(".", "") if isinstance(python, str) else ""
                if (
                    wheel is None
                    or python in seen_python
                    or record.get("filename") != wheel.name
                    or record.get("kind") != "wheel"
                    or record.get("abi") != f"cp{compact}"
                    or record.get("platform") != config.wheel_platform
                    or not isinstance(record.get("size"), int)
                    or isinstance(record.get("size"), bool)
                    or record.get("size") != wheel.stat().st_size
                    or record.get("size", 0) <= 0
                    or record.get("sha256") != sha256(wheel)
                ):
                    valid_records = False
                    break
                seen_python.add(python)
        if seen_python != set(config.interpreters):
            valid_records = False
    except (AttributeError, OSError, TypeError, ValueError) as error:
        raise CandidateError(
            "candidate profile summary does not bind the exact four-wheel family"
        ) from error
    if not valid_records:
        raise CandidateError(
            "candidate profile summary does not bind the exact four-wheel family"
        )

    try:
        checks = profiles.checks
        if (
            not isinstance(checks, tuple)
            or any(not isinstance(check, str) or not check for check in checks)
            or len(set(checks)) != len(checks)
        ):
            raise ValueError
        normalized_checks = tuple(
            "generated-public-api" if check == "check:generated-public-api" else check
            for check in checks
        )
        if len(set(normalized_checks)) != len(normalized_checks):
            raise ValueError
        required_checks = set().union(
            *(PROFILE_CHECKS[profile] for profile in REQUIRED_PROFILES)
        )
        producer_checks = required_checks | {
            f"cp{python.replace('.', '')}:packaged-exact-cylinder-model-demo"
            for python in config.interpreters
        }
        if set(normalized_checks) != producer_checks or not NOTEBOOK_CHECKS.issubset(
            normalized_checks
        ):
            raise ValueError
    except (AttributeError, KeyError, TypeError, ValueError) as error:
        raise CandidateError(
            "candidate profile summary does not contain the exact complete check set"
        ) from error

    numpy_version = config.numpy_floor.split("==", maxsplit=1)[1]
    expected_dependency_profiles = {
        "numpy_floor": {
            "python": config.numpy_floor_interpreter,
            "requirement": config.numpy_floor,
            "observed": numpy_version,
            "profile": (
                f"cp{config.numpy_floor_interpreter.replace('.', '')}:"
                f"numpy-{numpy_version}-floor"
            ),
        }
    }
    if profiles.dependency_profiles != expected_dependency_profiles:
        raise CandidateError(
            "candidate profile summary has invalid dependency profiles"
        )
    return profiles


def finalize_candidate(
    *,
    expected_commit: str,
    artifacts: Path,
    h2_receipt: Path,
    manifest_out: Path,
) -> Path:
    """Validate external H2 evidence, run profiles, and retain v3 metadata."""

    artifact_root = artifacts
    receipt_path = h2_receipt
    metadata_root = manifest_out
    _require_candidate_host(artifact_root.resolve())
    _require_candidate_host(metadata_root.resolve())
    roots = tuple(path.resolve() for path in (artifact_root, receipt_path, metadata_root))
    for index, left in enumerate(roots):
        for right in roots[index + 1 :]:
            if left == right or left.is_relative_to(right) or right.is_relative_to(left):
                raise CandidateError("family, H2, and metadata paths must be disjoint")
    source = _require_expected_source(expected_commit)
    family = admit_candidate_family(artifact_root)
    entry_inventory = family_inventory(artifact_root)
    if metadata_root.exists() and any(metadata_root.iterdir()):
        raise CandidateError("candidate metadata output directory must be empty")
    metadata_root.mkdir(parents=True, exist_ok=True)
    try:
        receipt = validate_h2_receipt(
            receipt_path,
            expected_commit=expected_commit,
            family=family,
        )
        frontend = derive_frontend_manifest(
            family=family,
            h2_receipt=receipt_path,
            receipt=receipt,
        )
        profiles = run_candidate_profiles(
            family=family,
            receipt=receipt,
            frontend=frontend,
        )
        profiles = _admit_candidate_profile_summary(
            profiles,
            artifact_root=artifact_root,
            family=family,
            entry_inventory=entry_inventory,
        )
        sdist = family.sdist
        manifest = write_manifest(
            output=metadata_root,
            source=source,
            sdist=sdist,
            version=_family_version_from_sdist(sdist),
            wheel_records=list(profiles.wheel_records),
            checks=list(profiles.checks),
            config=profiles.config,
            uv=profiles.uv,
            complete_profiles=True,
            dependency_profiles=profiles.dependency_profiles,
            frontend=frontend if isinstance(frontend, dict) else None,
        )
        retained_receipt = metadata_root / receipt_path.name
        if retained_receipt.exists():
            raise CandidateError("candidate finalization would replace retained metadata")
        retained_receipt.write_bytes(receipt_path.read_bytes())
        if retained_receipt.read_bytes() != receipt_path.read_bytes():
            raise CandidateError("retained H2 receipt bytes changed")
        if family_inventory(artifact_root) != entry_inventory:
            raise CandidateError("candidate family inventory changed after H2")
        candidate = load_candidate_family(
            manifest,
            artifact_root,
            requested_profiles=REQUIRED_PROFILES,
            h2_receipt=retained_receipt,
        )
        verify_artifacts(candidate, artifact_root)
        if {path.name for path in metadata_root.iterdir()} != {
            manifest.name,
            retained_receipt.name,
        }:
            raise CandidateError("candidate metadata output is not the exact closed pair")
        return manifest
    except Exception:
        for path in metadata_root.iterdir():
            if path.is_file() and not path.is_symlink():
                path.unlink()
        raise


def build_candidate(
    output: Path,
    *,
    require_tag: bool,
    skip_extras: bool,
) -> Path:
    """Build and accept one complete artifact family."""

    if require_tag and skip_extras:
        raise CandidateError("a tagged publication candidate cannot skip extras")
    if platform.system() != "Linux" or platform.machine().lower() not in {
        "x86_64",
        "amd64",
    }:
        raise CandidateError("the first Python candidate builder requires Linux x86-64")
    if output == ROOT or output.is_relative_to(ROOT):
        raise CandidateError(
            "candidate artifacts must be written outside the source tree"
        )
    if output.exists() and any(output.iterdir()):
        raise CandidateError("candidate output directory must be empty")
    output.mkdir(parents=True, exist_ok=True)

    config = load_config()
    source = source_identity()
    if require_tag:
        require_annotated_expected_tag(source, config.expected_tag)
    uv = ensure_exact_uv(config.uv)
    require_executable("git")
    require_executable("rustc")
    require_executable("cargo")
    rustup = require_executable("rustup")

    scratch_parent = home_scratch_parent("python-candidate")
    with tempfile.TemporaryDirectory(
        prefix="eqiora-python-candidate-",
        dir=scratch_parent,
    ) as temporary:
        scratch = Path(temporary)
        candidate_temporary = scratch / "tmp"
        candidate_temporary.mkdir()
        with command_context(environment={"TMPDIR": str(candidate_temporary)}):
            checked_run(
                [
                    rustup,
                    "toolchain",
                    "install",
                    config.rust,
                    "--profile",
                    "minimal",
                ]
            )

            interpreters = {
                version: uv_interpreter(uv, version) for version in config.interpreters
            }
            sdist, wheels, extracted = build_artifacts(
                output=output,
                scratch=scratch,
                config=config,
                uv=uv,
                interpreters=interpreters,
            )
            checked_run(
                [
                    uv,
                    "tool",
                    "run",
                    "--from",
                    config.twine,
                    "twine",
                    "check",
                    "--strict",
                    str(sdist),
                    *(str(wheels[version]) for version in config.interpreters),
                ]
            )

            versions: set[str] = set()
            records: list[dict[str, Any]] = []
            license_bytes = (extracted / "LICENSE").read_bytes()
            notice_bytes = (extracted / "NOTICE").read_bytes()
            for python_version in config.interpreters:
                wheel_version, record = inspect_wheel(
                    wheels[python_version],
                    python_version=python_version,
                    config=config,
                    license_bytes=license_bytes,
                    notice_bytes=notice_bytes,
                )
                versions.add(wheel_version)
                records.append(record)
            if len(versions) != 1:
                raise CandidateError("candidate wheels disagree on the package version")
            version = versions.pop()
            if version != config.python_version:
                raise CandidateError(
                    "candidate artifact version differs from the authored Cargo version"
                )

            initial_identity = candidate_payload_identity(
                sdist,
                wheels,
                extracted,
            )
            plan = candidate_profiles.build_profile_plan(
                scratch,
                config,
                skip_extras=skip_extras,
            )
            outcomes = candidate_profiles.run_profile_tasks(
                plan,
                lambda workspace: execute_profile(
                    workspace,
                    uv=uv,
                    config=config,
                    wheels=wheels,
                    extracted=extracted,
                    interpreters=interpreters,
                ),
            )
            replay_profile_logs(plan)

            outcomes_by_name = {outcome.name: outcome for outcome in outcomes}
            failures = tuple(
                (workspace.name, outcomes_by_name[workspace.name].error)
                for workspace in plan
                if outcomes_by_name[workspace.name].error is not None
            )
            if failures:
                diagnostics = "\n".join(
                    f"- {name}: {error}" for name, error in failures
                )
                raise CandidateError(
                    f"candidate validation profiles failed:\n{diagnostics}"
                )

            final_identity = candidate_payload_identity(
                sdist,
                wheels,
                extracted,
            )
            if final_identity != initial_identity:
                raise CandidateError(
                    "candidate validation profile mutated shared artifact inputs"
                )

            receipts = tuple(
                outcome.value for outcome in outcomes if outcome.value is not None
            )
            expected_names = (
                candidate_profiles.DEVELOPMENT_PROFILE_NAMES
                if skip_extras
                else candidate_profiles.COMPLETE_PROFILE_NAMES
            )
            try:
                merged = candidate_profiles.merge_profile_receipts(
                    expected_names,
                    receipts,
                )
            except ValueError as error:
                raise CandidateError(
                    f"candidate profile receipts are invalid: {error}"
                ) from error
            checks = [
                "twine-strict",
                "sdist-to-wheel-rebuild",
                *merged.checks,
            ]
            dependency_profiles = {
                name: dict(values) for name, values in merged.dependency_profiles
            }
            return write_manifest(
                output=output,
                source=source,
                sdist=sdist,
                version=version,
                wheel_records=records,
                checks=checks,
                config=config,
                uv=uv,
                complete_profiles=not skip_extras,
                dependency_profiles=dependency_profiles,
            )


def parse_args() -> argparse.Namespace:
    """Parse the closed prepare/finalize release handoff."""

    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare")
    prepare.add_argument("--expected-commit", required=True)
    prepare.add_argument("--out", type=Path, required=True)
    prepare.add_argument("--require-tag", action="store_true")
    finalize = commands.add_parser("finalize")
    finalize.add_argument("--expected-commit", required=True)
    finalize.add_argument("--artifacts", type=Path, required=True)
    finalize.add_argument("--h2-receipt", type=Path, required=True)
    finalize.add_argument("--manifest-out", type=Path, required=True)
    return parser.parse_args()


def main() -> int:
    """CLI entry point."""

    arguments = parse_args()
    try:
        if arguments.command == "prepare":
            result = prepare_candidate(
                expected_commit=arguments.expected_commit,
                out=arguments.out,
                require_tag=arguments.require_tag,
            )
        else:
            result = finalize_candidate(
                expected_commit=arguments.expected_commit,
                artifacts=arguments.artifacts,
                h2_receipt=arguments.h2_receipt,
                manifest_out=arguments.manifest_out,
            )
    except (CandidateError, OSError, subprocess.CalledProcessError) as error:
        print(f"Python candidate failed: {error}", file=sys.stderr)
        return 2
    print(result)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
