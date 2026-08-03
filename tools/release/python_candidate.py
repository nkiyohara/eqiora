#!/usr/bin/env python3
"""Build and verify one commit-bound Eqiora Python distribution candidate."""

from __future__ import annotations

import argparse
import email.parser
import json
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import zipfile
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import python_candidate_profiles as candidate_profiles
from python_candidate_common import (
    CandidateError,
    DistributionConfig,
    candidate_payload_identity,
    checked_run,
    command_context,
    home_scratch_parent,
    python_distribution_version as python_distribution_version,
    sha256 as sha256,
)


ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT
PYPROJECT = ROOT / "pyproject.toml"
MANIFEST_FORMAT = "eqiora.python-distribution-candidate/v2"
PYTHON_TEST_FIXTURES = candidate_profiles.PYTHON_TEST_FIXTURES


@dataclass(frozen=True)
class SourceIdentity:
    """Exact source state from which a candidate is built."""

    commit: str
    tags: tuple[str, ...]


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
    interpreters = tuple(raw["ordinary-gil-cpython"])
    jax = tuple(raw["tested-jax"])
    config = DistributionConfig(
        cargo_version=cargo["workspace"]["package"]["version"],
        interpreters=interpreters,
        wheel_platform=raw["wheel-platform"],
        extras_interpreter=raw["extras-python"],
        numpy_floor_interpreter=raw["numpy-floor-python"],
        numpy_floor=raw["tested-numpy-floor"],
        uv=raw["uv"],
        maturin=maturin[0],
        pytest=raw["pytest"],
        mypy=raw["mypy"],
        twine=raw["twine"],
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
    if config.python_version != "0.1.0a1":
        raise CandidateError(
            "the first public alpha candidate must have Python version 0.1.0a1"
        )
    if config.uv != "uv==0.11.31":
        raise CandidateError("the first candidate must use exact uv 0.11.31")
    return config


def require_executable(name: str) -> str:
    """Resolve one conventional release tool or fail clearly."""

    executable = shutil.which(name)
    if executable is None:
        raise CandidateError(f"required executable is unavailable: {name}")
    return executable


def require_exact_uv(executable: str, requirement: str) -> None:
    """Require the declared release tool rather than an ambient compatible one."""

    name, separator, expected = requirement.partition("==")
    if name != "uv" or not separator or not expected:
        raise CandidateError("the uv build-tool requirement is malformed")
    observed = tool_version([executable, "--version"]).split()
    if len(observed) < 2 or observed[0] != "uv" or observed[1] != expected:
        raise CandidateError(
            f"candidate requires uv {expected}, observed {' '.join(observed)!r}"
        )


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


def build_artifacts(
    *,
    output: Path,
    scratch: Path,
    config: DistributionConfig,
    uv: str,
    interpreters: dict[str, str],
) -> tuple[Path, dict[str, Path], Path]:
    """Build one sdist, then every wheel solely from its extracted content."""

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
    sdists = sorted(output.glob("eqiora-*.tar.gz"))
    if len(sdists) != 1:
        raise CandidateError("maturin must produce exactly one source distribution")
    sdist = sdists[0]
    expected_sdist = f"eqiora-{config.python_version}.tar.gz"
    if sdist.name != expected_sdist:
        raise CandidateError(
            f"source distribution identity drifted: expected {expected_sdist}, "
            f"received {sdist.name}"
        )

    extracted = safe_extract_sdist(sdist, scratch / "source")
    if cargo_workspace_version(extracted) != config.cargo_version:
        raise CandidateError(
            "source distribution Cargo version differs from the candidate source"
        )
    target = scratch / "cargo-target"
    wheels: dict[str, Path] = {}
    wheel_tool = maturin_package(config, zig=True)
    for version in config.interpreters:
        before = set(output.glob("*.whl"))
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
                "manylinux2014",
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
        created = set(output.glob("*.whl")) - before
        if len(created) != 1:
            raise CandidateError(f"CPython {version} did not produce one wheel")
        wheels[version] = created.pop()
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
    expected_prefix = f"eqiora-{config.python_version}-"
    if not wheel.name.startswith(expected_prefix):
        raise CandidateError(f"wheel has the wrong distribution version: {wheel.name}")
    required_tag = f"-cp{compact}-cp{compact}-"
    if required_tag not in wheel.name:
        raise CandidateError(f"wheel has the wrong CPython tag: {wheel.name}")
    if config.wheel_platform not in wheel.name:
        raise CandidateError(f"wheel has the wrong platform tag: {wheel.name}")

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
            "eqiora/solid.pyi",
            "eqiora/torch.pyi",
            "eqiora/py.typed",
            "eqiora/examples/steady-flow-past-cylinder.model.json",
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
    normalized = [dependency.lower().replace(" ", "") for dependency in dependencies]
    numpy_requirements = [item for item in normalized if item.startswith("numpy")]
    if (
        len(numpy_requirements) != 1
        or ">=2.1" not in numpy_requirements[0]
        or "<3" not in numpy_requirements[0]
        or ";" in numpy_requirements[0]
    ):
        raise CandidateError("wheel must declare the reviewed NumPy range")
    for framework in ("torch", "jax", "jaxlib", "matplotlib"):
        declarations = [item for item in normalized if item.startswith(framework)]
        if not declarations or any("extra==" not in item for item in declarations):
            raise CandidateError(
                f"{framework} must remain an optional-extra dependency"
            )
    if sorted(metadata.get_all("Provides-Extra", [])) != [
        "jax",
        "matplotlib",
        "torch",
    ]:
        raise CandidateError(
            "wheel must expose exactly the jax, matplotlib, and torch extras"
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
prepare_fixed_reference_fsi_demo_consumer = (
    candidate_profiles.prepare_fixed_reference_fsi_demo_consumer
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


def execute_profile(
    workspace: candidate_profiles.ProfileWorkspace,
    *,
    uv: str,
    config: DistributionConfig,
    wheels: dict[str, Path],
    extracted: Path,
    interpreters: dict[str, str],
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
    path = output / f"eqiora-{version}-python-candidate.json"
    path.write_text(
        json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    return path


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
    uv = require_executable("uv")
    require_exact_uv(uv, config.uv)
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
            merged = candidate_profiles.merge_profile_receipts(
                expected_names,
                receipts,
            )
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
    """Parse the intentionally small release CLI."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="empty output directory for the accepted artifact set",
    )
    parser.add_argument(
        "--require-tag",
        action="store_true",
        help="reject an otherwise valid candidate whose source commit is untagged",
    )
    parser.add_argument(
        "--skip-extras",
        action="store_true",
        help=(
            "development-only: omit the exact PyTorch/JAX/Matplotlib "
            "and full stub profiles"
        ),
    )
    return parser.parse_args()


def main() -> int:
    """CLI entry point."""

    arguments = parse_args()
    try:
        manifest = build_candidate(
            arguments.out.resolve(),
            require_tag=arguments.require_tag,
            skip_extras=arguments.skip_extras,
        )
    except (CandidateError, OSError, subprocess.CalledProcessError) as error:
        print(f"Python candidate failed: {error}", file=sys.stderr)
        return 2
    print(manifest)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
