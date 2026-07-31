#!/usr/bin/env python3
"""Build and verify one commit-bound Eqiora Python distribution candidate."""

from __future__ import annotations

import argparse
import email.parser
import hashlib
import json
import os
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


ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT
PYPROJECT = ROOT / "pyproject.toml"
MANIFEST_FORMAT = "eqiora.python-distribution-candidate/v2"
EXACT_CYLINDER_DEMO = Path("examples/python/exact_cylinder_stokes.py")
EXACT_CYLINDER_REPOSITORY_MODEL = Path(
    "examples/steady-flow-past-cylinder.model-v7.json"
)
PYTHON_TEST_FIXTURES = (
    Path("verify/interfaces/control-plane-compile-check"),
    Path("verify/interfaces/current-authoring-profile"),
    # The packaged-Poisson consumer test compiles the shipped package source
    # itself, so reading it is the claim rather than an implementation detail.
    Path("packages/org.example.poisson"),
)


class CandidateError(RuntimeError):
    """The requested artifact set is not an acceptable release candidate."""


def python_distribution_version(cargo_version: str) -> str:
    """Map admitted Cargo SemVer release forms to normalized Python versions."""

    if "+" in cargo_version:
        raise CandidateError(
            "Cargo release versions with build metadata are unsupported"
        )
    release, separator, prerelease = cargo_version.partition("-")
    release_components = release.split(".")
    if len(release_components) != 3 or any(
        not component or not component.isascii() or not component.isdecimal()
        for component in release_components
    ):
        raise CandidateError(f"invalid Cargo release version: {cargo_version}")
    if not separator:
        return release
    prerelease_components = prerelease.split(".")
    if len(prerelease_components) != 2:
        raise CandidateError(f"unsupported Cargo prerelease identity: {cargo_version}")
    label, serial = prerelease_components
    markers = {"alpha": "a", "beta": "b", "rc": "rc"}
    if (
        label not in markers
        or not serial
        or not serial.isascii()
        or not serial.isdecimal()
        or str(int(serial)) != serial
    ):
        raise CandidateError(f"unsupported Cargo prerelease identity: {cargo_version}")
    return f"{release}{markers[label]}{serial}"


@dataclass(frozen=True)
class DistributionConfig:
    """Reviewed Python distribution inputs from ``pyproject.toml``."""

    cargo_version: str
    interpreters: tuple[str, ...]
    wheel_platform: str
    extras_interpreter: str
    numpy_floor_interpreter: str
    numpy_floor: str
    uv: str
    maturin: str
    pytest: str
    mypy: str
    twine: str
    torch: str
    jax: tuple[str, ...]
    matplotlib: str
    rust: str

    @property
    def python_version(self) -> str:
        """Normalized distribution version derived from Cargo."""

        return python_distribution_version(self.cargo_version)

    @property
    def expected_tag(self) -> str:
        """Only release tag admitted for this authored version."""

        return f"v{self.python_version}"


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


def checked_run(
    argv: list[str],
    *,
    cwd: Path = ROOT,
    capture: bool = False,
    extra_environment: dict[str, str] | None = None,
) -> str:
    """Run one shell-free command under a source-isolated environment."""

    environment = os.environ.copy()
    environment.pop("DISPLAY", None)
    environment.pop("MATPLOTLIBRC", None)
    environment.pop("MPLCONFIGDIR", None)
    environment.pop("PYTHONPATH", None)
    environment.update(
        {
            "PIP_DISABLE_PIP_VERSION_CHECK": "1",
            "PYTHONDONTWRITEBYTECODE": "1",
            "PYTHONNOUSERSITE": "1",
        }
    )
    if extra_environment is not None:
        environment.update(extra_environment)
    completed = subprocess.run(
        argv,
        cwd=cwd,
        env=environment,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.STDOUT if capture else None,
    )
    return completed.stdout.strip() if capture else ""


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
        interpreter = uv_interpreter(uv, version)
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
                interpreter,
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
            "eqiora/compatibility.pyi",
            "eqiora/diff.pyi",
            "eqiora/jax.pyi",
            "eqiora/matplotlib.pyi",
            "eqiora/torch.pyi",
            "eqiora/py.typed",
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


def sha256(path: Path) -> str:
    """Return a lowercase SHA-256 artifact identity."""

    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def venv_python(environment: Path) -> Path:
    """Return the interpreter inside a uv-created virtual environment."""

    return environment / ("Scripts/python.exe" if os.name == "nt" else "bin/python")


def install_environment(
    *,
    uv: str,
    interpreter: str,
    environment: Path,
    requirements: list[str],
) -> Path:
    """Create one isolated environment and install exact artifact inputs."""

    checked_run([uv, "venv", "--python", interpreter, str(environment)])
    python = venv_python(environment)
    checked_run(
        [uv, "pip", "install", "--python", str(python), *requirements],
        cwd=environment.parent,
    )
    return python


def assert_installed_origin(
    python: Path,
    wheel: Path,
    run_root: Path,
    expected_version: str,
) -> None:
    """Prove that imports and metadata come from the installed wheel."""

    script = """
import importlib.metadata as metadata
import os
from pathlib import Path
import eqiora

root = Path.cwd().resolve()
module = Path(eqiora.__file__).resolve()
assert root not in module.parents, (root, module)
distribution = metadata.distribution("eqiora")
expected_version = os.environ["EQIORA_EXPECTED_VERSION"]
assert distribution.version == expected_version
assert eqiora.__version__ == expected_version
files = {str(path) for path in distribution.files or ()}
assert "eqiora/py.typed" in files
assert "eqiora/__init__.pyi" in files
assert not any(
    name in __import__("sys").modules
    for name in ("torch", "jax", "jaxlib", "matplotlib")
)
"""
    checked_run(
        [str(python), "-I", "-c", script],
        cwd=run_root,
        extra_environment={
            "EQIORA_EXPECTED_VERSION": expected_version,
            "EQIORA_EXPECTED_WHEEL": wheel.name,
        },
    )


def prepare_base_consumer_tree(extracted: Path, run_root: Path) -> tuple[Path, Path]:
    """Copy tests, typing fixtures, and their exact data without package sources."""

    tests = run_root / "bindings/python/tests"
    typecheck = run_root / "bindings/python/typecheck"
    shutil.copytree(extracted / "bindings/python/tests", tests)
    shutil.copytree(extracted / "bindings/python/typecheck", typecheck)
    for relative in PYTHON_TEST_FIXTURES:
        shutil.copytree(extracted / relative, run_root / relative)
    return tests, typecheck


def prepare_exact_cylinder_demo_consumer(
    extracted: Path,
    run_root: Path,
) -> Path:
    """Copy one checked-in demo without its repository Model dependency."""

    source = extracted / EXACT_CYLINDER_DEMO
    if not source.is_file():
        raise CandidateError(
            f"source distribution omits checked-in demo {EXACT_CYLINDER_DEMO}"
        )
    repository_model = run_root / EXACT_CYLINDER_REPOSITORY_MODEL
    if repository_model.exists():
        raise CandidateError(
            "exact-cylinder consumer tree unexpectedly carries the repository Model"
        )
    destination = run_root / EXACT_CYLINDER_DEMO
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)
    if repository_model.exists():  # pragma: no cover - copy2 cannot create it
        raise CandidateError(
            "exact-cylinder demo copy introduced the repository Model"
        )
    return destination


def assert_matplotlib_is_optional(python: Path, run_root: Path) -> None:
    """Require the base wheel to explain its absent Matplotlib adapter."""

    script = """
import importlib.util
import sys

assert importlib.util.find_spec("matplotlib") is None
import eqiora
assert "matplotlib" not in sys.modules
try:
    import eqiora.matplotlib
except ImportError as error:
    assert str(error) == (
        "eqiora.matplotlib requires the optional 'matplotlib' dependency; "
        "install eqiora[matplotlib]"
    )
else:
    raise AssertionError("the base environment unexpectedly imported Matplotlib")
"""
    checked_run([str(python), "-I", "-c", script], cwd=run_root)


def run_public_smoke(
    *,
    python: Path,
    extracted: Path,
    run_root: Path,
    expected_version: str,
    profile: str,
) -> None:
    """Replay one published quick start against an installed wheel."""

    checked_run(
        [
            str(python),
            "-I",
            str(extracted / "tools/release/python_public_smoke.py"),
            "--expected-version",
            expected_version,
            "--profile",
            profile,
        ],
        cwd=run_root,
    )


def run_base_profile(
    *,
    uv: str,
    interpreter: str,
    python_version: str,
    wheel: Path,
    extracted: Path,
    scratch: Path,
    config: DistributionConfig,
) -> list[str]:
    """Run the complete framework-free suite and strict base consumer typing."""

    environment = scratch / f"base-{python_version}"
    python = install_environment(
        uv=uv,
        interpreter=interpreter,
        environment=environment,
        requirements=[str(wheel), config.pytest, config.mypy],
    )
    run_root = scratch / f"run-base-{python_version}"
    run_root.mkdir()
    tests, typecheck = prepare_base_consumer_tree(extracted, run_root)
    prepare_exact_cylinder_demo_consumer(extracted, run_root)
    assert_installed_origin(
        python,
        wheel,
        run_root,
        config.python_version,
    )
    assert_matplotlib_is_optional(python, run_root)
    checked_run(
        [str(python), "-I", "-m", "pytest", "-q", str(tests)],
        cwd=run_root,
    )
    checked_run(
        [
            str(python),
            "-I",
            "-m",
            "mypy",
            "--strict",
            str(typecheck / "base.py"),
        ],
        cwd=run_root,
    )
    run_public_smoke(
        python=python,
        extracted=extracted,
        run_root=run_root,
        expected_version=config.python_version,
        profile="base",
    )
    return [
        f"cp{python_version.replace('.', '')}:installed-wheel",
        f"cp{python_version.replace('.', '')}:base-and-numpy",
        f"cp{python_version.replace('.', '')}:packaged-exact-cylinder-model-demo",
        f"cp{python_version.replace('.', '')}:async-and-cancellation",
        f"cp{python_version.replace('.', '')}:strict-base-typing",
        f"cp{python_version.replace('.', '')}:public-smoke-base",
        f"cp{python_version.replace('.', '')}:matplotlib-free-base",
    ]


def run_optional_profile(
    *,
    name: str,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    scratch: Path,
    config: DistributionConfig,
) -> list[str]:
    """Run one optional framework adapter from the exact same wheel."""

    if name == "torch":
        exact = [config.torch]
        test = "test_torch.py"
        environment_variables = {
            "EQIORA_TEST_TORCH_VERSION": config.torch.split("==", maxsplit=1)[1]
        }
    elif name == "jax":
        exact = list(config.jax)
        test = "test_jax.py"
        environment_variables = {
            "EQIORA_REQUIRE_JAX_ABI_PROBE": "1",
            "EQIORA_TEST_JAX_VERSION": config.jax[0].split("==", maxsplit=1)[1],
            "EQIORA_TEST_PYTHON_VERSION": config.extras_interpreter,
            "JAX_ENABLE_X64": "1",
            "XLA_FLAGS": "--xla_force_host_platform_device_count=2",
        }
    elif name == "matplotlib":
        exact = [config.matplotlib]
        test = "test_matplotlib.py"
        matplotlib_config = scratch / "matplotlib-config"
        matplotlib_config.mkdir()
        environment_variables = {
            "EQIORA_TEST_MATPLOTLIB_VERSION": config.matplotlib.split("==", maxsplit=1)[
                1
            ],
            "MPLBACKEND": "Agg",
            "MPLCONFIGDIR": str(matplotlib_config),
        }
    else:  # pragma: no cover - closed internal call set
        raise CandidateError(f"unknown optional profile: {name}")

    environment = scratch / name
    python = install_environment(
        uv=uv,
        interpreter=interpreter,
        environment=environment,
        requirements=[f"{wheel}[{name}]", config.pytest, *exact],
    )
    run_root = scratch / f"run-{name}"
    run_root.mkdir()
    test_path = run_root / test
    shutil.copy2(extracted / f"bindings/python/tests/{test}", test_path)
    demo = None
    if name == "matplotlib":
        demo = prepare_exact_cylinder_demo_consumer(extracted, run_root)
    checked_run(
        [str(python), "-I", "-m", "pytest", "-q", str(test_path)],
        cwd=run_root,
        extra_environment=environment_variables,
    )
    if name == "matplotlib":
        assert demo is not None
        destination = run_root / "exact-cylinder-pressure.png"
        checked_run(
            [
                str(python),
                "-I",
                str(demo),
                "--pressure-png",
                str(destination),
            ],
            cwd=run_root,
            extra_environment=environment_variables,
        )
        if not destination.is_file() or not destination.read_bytes().startswith(
            b"\x89PNG\r\n\x1a\n"
        ):
            raise CandidateError(
                "installed exact-cylinder Matplotlib demo did not write a PNG"
            )
    else:
        run_public_smoke(
            python=python,
            extracted=extracted,
            run_root=run_root,
            expected_version=config.python_version,
            profile=name,
        )
    compact = config.extras_interpreter.replace(".", "")
    verification = (
        "packaged-exact-cylinder-pressure-demo"
        if name == "matplotlib"
        else f"public-smoke-{name}"
    )
    return [
        f"cp{compact}:{name}",
        f"cp{compact}:{verification}",
    ]


def run_numpy_floor_profile(
    *,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    scratch: Path,
    config: DistributionConfig,
) -> tuple[list[str], dict[str, str]]:
    """Exercise the exact declared NumPy floor without replacing latest profiles."""

    python_version = config.numpy_floor_interpreter
    environment = scratch / f"numpy-floor-{python_version}"
    python = install_environment(
        uv=uv,
        interpreter=interpreter,
        environment=environment,
        requirements=[str(wheel), config.numpy_floor, config.pytest],
    )
    run_root = scratch / f"run-numpy-floor-{python_version}"
    run_root.mkdir()
    test_path = run_root / "test_array_transport.py"
    shutil.copy2(
        extracted / "bindings/python/tests/test_array_transport.py",
        test_path,
    )
    checked_run(
        [str(python), "-I", "-m", "pytest", "-q", str(test_path)],
        cwd=run_root,
    )
    run_public_smoke(
        python=python,
        extracted=extracted,
        run_root=run_root,
        expected_version=config.python_version,
        profile="base",
    )
    observed = checked_run(
        [
            str(python),
            "-I",
            "-c",
            "import importlib.metadata as m; print(m.version('numpy'))",
        ],
        cwd=run_root,
        capture=True,
    )
    expected = config.numpy_floor.split("==", maxsplit=1)[1]
    if observed != expected:
        raise CandidateError(
            f"NumPy floor profile expected {expected}, observed {observed!r}"
        )
    compact = python_version.replace(".", "")
    profile = f"cp{compact}:numpy-{observed}-floor"
    return [profile], {
        "python": python_version,
        "requirement": config.numpy_floor,
        "observed": observed,
        "profile": profile,
    }


def run_full_typing_profile(
    *,
    uv: str,
    interpreter: str,
    wheel: Path,
    extracted: Path,
    scratch: Path,
    config: DistributionConfig,
) -> str:
    """Check runtime/stub parity and strict optional-adapter consumers."""

    environment = scratch / "typing-all"
    python = install_environment(
        uv=uv,
        interpreter=interpreter,
        environment=environment,
        requirements=[
            f"{wheel}[torch,jax,matplotlib]",
            config.mypy,
            config.torch,
            *config.jax,
            config.matplotlib,
        ],
    )
    run_root = scratch / "run-typing-all"
    run_root.mkdir()
    typecheck = run_root / "typecheck"
    shutil.copytree(extracted / "bindings/python/typecheck", typecheck)
    checked_run(
        [
            str(python),
            "-I",
            "-m",
            "mypy.stubtest",
            "eqiora",
            "--concise",
            "--ignore-disjoint-bases",
        ],
        cwd=run_root,
    )
    checked_run(
        [
            str(python),
            "-I",
            "-m",
            "mypy",
            "--strict",
            str(typecheck / "diff.py"),
            str(typecheck / "torch_adapter.py"),
            str(typecheck / "jax_adapter.py"),
            str(typecheck / "matplotlib_adapter.py"),
        ],
        cwd=run_root,
    )
    return f"cp{config.extras_interpreter.replace('.', '')}:complete-public-typing"


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
        "checks": sorted(checks),
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

    with tempfile.TemporaryDirectory(prefix="eqiora-python-candidate-") as temporary:
        scratch = Path(temporary)
        sdist, wheels, extracted = build_artifacts(
            output=output,
            scratch=scratch,
            config=config,
            uv=uv,
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
        checks = ["twine-strict", "sdist-to-wheel-rebuild"]
        license_bytes = (extracted / "LICENSE").read_bytes()
        notice_bytes = (extracted / "NOTICE").read_bytes()
        interpreters: dict[str, str] = {}
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
            interpreter = uv_interpreter(uv, python_version)
            interpreters[python_version] = interpreter
            checks.extend(
                run_base_profile(
                    uv=uv,
                    interpreter=interpreter,
                    python_version=python_version,
                    wheel=wheels[python_version],
                    extracted=extracted,
                    scratch=scratch,
                    config=config,
                )
            )
        floor_checks, numpy_floor = run_numpy_floor_profile(
            uv=uv,
            interpreter=interpreters[config.numpy_floor_interpreter],
            wheel=wheels[config.numpy_floor_interpreter],
            extracted=extracted,
            scratch=scratch,
            config=config,
        )
        checks.extend(floor_checks)
        if len(versions) != 1:
            raise CandidateError("candidate wheels disagree on the package version")
        version = versions.pop()
        if version != config.python_version:
            raise CandidateError(
                "candidate artifact version differs from the authored Cargo version"
            )
        checked_run(
            [
                interpreters[config.extras_interpreter],
                "-I",
                str(extracted / "tools/docs/generate_python_api.py"),
                "--check",
            ],
            cwd=extracted,
        )
        checks.append("generated-public-api")

        if not skip_extras:
            extras_version = config.extras_interpreter
            extras_wheel = wheels[extras_version]
            extras_interpreter = interpreters[extras_version]
            checks.extend(
                run_optional_profile(
                    name="torch",
                    uv=uv,
                    interpreter=extras_interpreter,
                    wheel=extras_wheel,
                    extracted=extracted,
                    scratch=scratch,
                    config=config,
                )
            )
            checks.extend(
                run_optional_profile(
                    name="jax",
                    uv=uv,
                    interpreter=extras_interpreter,
                    wheel=extras_wheel,
                    extracted=extracted,
                    scratch=scratch,
                    config=config,
                )
            )
            checks.extend(
                run_optional_profile(
                    name="matplotlib",
                    uv=uv,
                    interpreter=extras_interpreter,
                    wheel=extras_wheel,
                    extracted=extracted,
                    scratch=scratch,
                    config=config,
                )
            )
            checks.append(
                run_full_typing_profile(
                    uv=uv,
                    interpreter=extras_interpreter,
                    wheel=extras_wheel,
                    extracted=extracted,
                    scratch=scratch,
                    config=config,
                )
            )

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
        dependency_profiles={"numpy_floor": numpy_floor},
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
