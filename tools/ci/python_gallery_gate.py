#!/usr/bin/env python3
"""Build an installed wheel and verify the private gallery-media bundle."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
GALLERY = ROOT / "tools" / "gallery"
BUILD_SCRIPT = GALLERY / "build_fixed_reference_fsi.py"
TESTS = GALLERY / "tests"
_PROJECT = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
_DISTRIBUTION = _PROJECT["tool"]["eqiora-distribution"]
MATURIN = _PROJECT["build-system"]["requires"][0]
PYTEST = _DISTRIBUTION["pytest"]
MATPLOTLIB = _DISTRIBUTION["tested-matplotlib"]
PYTHON = _DISTRIBUTION["extras-python"]
INNER = "EQIORA_GALLERY_GATE_INNER"
PROFILE = {
    "SOURCE_DATE_EPOCH": "0",
    "TZ": "UTC",
    "LC_ALL": "C",
    "PYTHONHASHSEED": "0",
    "MPLBACKEND": "Agg",
}


def venv_python(environment: Path) -> Path:
    """Return the platform-native interpreter path for one virtual environment."""

    if os.name == "nt":
        return environment / "Scripts" / "python.exe"
    return environment / "bin" / "python"


def child_environment(
    *, mplconfig: Path, inner: bool = False, uv_cache: Path | None = None
) -> dict[str, str]:
    """Return the exact headless producer environment."""

    environment = os.environ.copy()
    for key in ("DISPLAY", "MATPLOTLIBRC", "PYTHONPATH"):
        environment.pop(key, None)
    environment.update(PROFILE)
    environment["MPLCONFIGDIR"] = str(mplconfig)
    environment["PIP_DISABLE_PIP_VERSION_CHECK"] = "1"
    environment["PYTHONNOUSERSITE"] = "1"
    if uv_cache is not None:
        environment["UV_CACHE_DIR"] = str(uv_cache)
    if inner:
        environment[INNER] = "1"
    else:
        environment.pop(INNER, None)
    return environment


def run(
    argv: list[str],
    *,
    cwd: Path = ROOT,
    environment: dict[str, str],
) -> subprocess.CompletedProcess[str]:
    """Run one checked gate command without shell interpretation."""

    return subprocess.run(
        argv,
        cwd=cwd,
        env=environment,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=None,
    )


def uv_gate_command(uv: str) -> list[str]:
    """Build a non-editable installed wheel before entering the evidence process."""

    return [
        uv,
        "run",
        "--directory",
        str(ROOT),
        "--isolated",
        "--no-editable",
        "--reinstall-package",
        "eqiora",
        "--extra",
        "matplotlib",
        "--with",
        PYTEST,
        "--with",
        MATPLOTLIB,
        "--python",
        PYTHON,
        "python",
        "-E",
        "-s",
        str(Path("tools/ci/python_gallery_gate.py")),
    ]


def inner_gate(*, mplconfig: Path) -> None:
    """Run pure mutants and the complete installed-result media build."""

    import matplotlib

    expected_matplotlib = MATPLOTLIB.removeprefix("matplotlib==")
    if matplotlib.__version__ != expected_matplotlib:
        raise RuntimeError(
            "gallery gate Matplotlib drifted: "
            f"expected {expected_matplotlib}, received {matplotlib.__version__}"
        )
    environment = child_environment(mplconfig=mplconfig, inner=True)
    run(
        [
            sys.executable,
            "-E",
            "-s",
            "-m",
            "pytest",
            "-q",
            str(TESTS),
        ],
        cwd=GALLERY,
        environment=environment,
    )
    with tempfile.TemporaryDirectory(prefix="eqiora-gallery-evidence-") as directory:
        output = Path(directory) / "bundle"
        completed = run(
            [
                sys.executable,
                "-E",
                "-s",
                str(BUILD_SCRIPT),
                "--output-dir",
                str(output),
                "--verify-determinism",
            ],
            environment=environment,
        )
        digest = completed.stdout.strip()
        if len(digest) != 64 or any(
            character not in "0123456789abcdef" for character in digest
        ):
            raise RuntimeError("gallery build did not print one SHA-256 record identity")
        record_path = output / "dev-build-record.json"
        identity_path = output / "dev-build-record.sha256"
        value = json.loads(record_path.read_text(encoding="utf-8"))
        if value["publication_status"] != "development-preview":
            raise RuntimeError("gallery evidence emitted a publishable record")
        if identity_path.read_text(encoding="ascii") != (
            f"{digest}  {record_path.name}\n"
        ):
            raise RuntimeError("gallery record sidecar disagrees with stdout")
        expected = {
            "dev-poster.png",
            "dev-film.webm",
            "dev-film.mp4",
            "dev-reduced-motion.png",
            "dev-text-alternative.txt",
            "dev-build-record.json",
            "dev-build-record.sha256",
        }
        if {path.name for path in output.iterdir()} != expected:
            raise RuntimeError("gallery build emitted an unexpected file inventory")


def main() -> int:
    """Provision the exact optional renderer and run the evidence."""

    try:
        with tempfile.TemporaryDirectory(
            prefix="eqiora-gallery-matplotlib-config-"
        ) as directory:
            mplconfig = Path(directory)
            if os.environ.get(INNER) == "1":
                inner_gate(mplconfig=mplconfig)
                return 0
            if uv := shutil.which("uv"):
                with tempfile.TemporaryDirectory(
                    prefix="eqiora-gallery-uv-cache-"
                ) as cache_directory:
                    run(
                        uv_gate_command(uv),
                        environment=child_environment(
                            mplconfig=mplconfig,
                            inner=True,
                            uv_cache=Path(cache_directory),
                        ),
                    )
                return 0
            with tempfile.TemporaryDirectory(
                prefix="eqiora-gallery-venv-"
            ) as environment_directory:
                virtual_environment = Path(environment_directory)
                subprocess.run(
                    [sys.executable, "-m", "venv", str(virtual_environment)],
                    check=True,
                    env=child_environment(mplconfig=mplconfig),
                )
                python = venv_python(virtual_environment)
                environment = child_environment(mplconfig=mplconfig, inner=True)
                scripts = python.parent
                environment["PATH"] = os.pathsep.join(
                    (str(scripts), environment.get("PATH", ""))
                )
                environment["VIRTUAL_ENV"] = str(virtual_environment)
                run(
                    [
                        str(python),
                        "-m",
                        "pip",
                        "install",
                        MATURIN,
                        PYTEST,
                        MATPLOTLIB,
                    ],
                    environment=environment,
                )
                run(
                    [
                        str(python),
                        "-m",
                        "pip",
                        "install",
                        "--no-build-isolation",
                        ".[matplotlib]",
                    ],
                    environment=environment,
                )
                run(
                    [str(python), "-E", "-s", str(Path(__file__).resolve())],
                    environment=environment,
                )
                return 0
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Python gallery gate failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
