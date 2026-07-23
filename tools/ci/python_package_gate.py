#!/usr/bin/env python3
"""Build, install, and test the Python package in one ephemeral interpreter."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT
TESTS = ROOT / "bindings/python/tests"
_PROJECT = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
_DISTRIBUTION = _PROJECT["tool"]["eqiora-distribution"]
BUILD_TOOLS = (
    _PROJECT["build-system"]["requires"][0],
    _DISTRIBUTION["pytest"],
)


def venv_python(environment: Path) -> Path:
    """Return the platform-native interpreter path for one virtual environment."""
    if os.name == "nt":
        return environment / "Scripts/python.exe"
    return environment / "bin/python"


def run(argv: list[str], *, cwd: Path = ROOT) -> None:
    environment = os.environ.copy()
    environment.update(
        {
            "PIP_DISABLE_PIP_VERSION_CHECK": "1",
            "PYTHONNOUSERSITE": "1",
        }
    )
    subprocess.run(argv, cwd=cwd, env=environment, check=True)


def uv_gate_command(uv: str, python: str) -> list[str]:
    """Build the current source tree even when its package version is unchanged."""
    return [
        uv,
        "run",
        "--directory",
        str(PACKAGE),
        "--isolated",
        "--no-editable",
        "--reinstall-package",
        "eqiora",
        "--with",
        BUILD_TOOLS[1],
        "--python",
        python,
        "python",
        "-m",
        "pytest",
        "-q",
        str(TESTS),
    ]


def main() -> int:
    try:
        if uv := shutil.which("uv"):
            run(uv_gate_command(uv, sys.executable))
            return 0
        with tempfile.TemporaryDirectory(prefix="eqiora-python-gate-") as directory:
            environment = Path(directory)
            run([sys.executable, "-m", "venv", str(environment)])
            python = str(venv_python(environment))
            run([python, "-m", "pip", "install", *BUILD_TOOLS])
            run(
                [python, "-m", "pip", "install", "--no-build-isolation", "."],
                cwd=PACKAGE,
            )
            run([python, "-m", "pytest", "-q", str(TESTS)], cwd=PACKAGE)
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"Python package gate failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
