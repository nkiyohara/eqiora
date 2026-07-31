#!/usr/bin/env python3
"""Build an installed wheel environment and verify the Matplotlib adapter."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TEST = ROOT / "bindings/python/tests/test_matplotlib.py"
_PROJECT = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
_DISTRIBUTION = _PROJECT["tool"]["eqiora-distribution"]
MATURIN = _PROJECT["build-system"]["requires"][0]
PYTEST = _DISTRIBUTION["pytest"]
MATPLOTLIB = _DISTRIBUTION["tested-matplotlib"]
PYTHON = _DISTRIBUTION["extras-python"]


def venv_python(environment: Path) -> Path:
    """Return the platform-native interpreter path for one virtual environment."""

    if os.name == "nt":
        return environment / "Scripts/python.exe"
    return environment / "bin/python"


def run(argv: list[str], *, cwd: Path = ROOT) -> None:
    environment = os.environ.copy()
    environment.pop("DISPLAY", None)
    environment.pop("PYTHONPATH", None)
    environment.update(
        {
            "EQIORA_TEST_MATPLOTLIB_VERSION": MATPLOTLIB.removeprefix("matplotlib=="),
            "MPLBACKEND": "Agg",
            "PIP_DISABLE_PIP_VERSION_CHECK": "1",
            "PYTHONNOUSERSITE": "1",
        }
    )
    subprocess.run(argv, cwd=cwd, env=environment, check=True)


def uv_gate_command(uv: str) -> list[str]:
    """Install the optional extra and exact verified renderer release."""

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
        "-m",
        "pytest",
        "-q",
        str(TEST),
    ]


def main() -> int:
    try:
        if uv := shutil.which("uv"):
            run(uv_gate_command(uv))
            return 0
        interpreter = shutil.which("python3.13")
        if interpreter is None:
            raise RuntimeError(
                "the Matplotlib evidence gate requires CPython 3.13 or uv"
            )
        with tempfile.TemporaryDirectory(prefix="eqiora-matplotlib-gate-") as directory:
            environment = Path(directory)
            run([interpreter, "-m", "venv", str(environment)])
            python = str(venv_python(environment))
            run(
                [
                    python,
                    "-m",
                    "pip",
                    "install",
                    MATURIN,
                    PYTEST,
                    MATPLOTLIB,
                ]
            )
            run(
                [
                    python,
                    "-m",
                    "pip",
                    "install",
                    "--no-build-isolation",
                    ".[matplotlib]",
                ],
                cwd=ROOT,
            )
            run([python, "-m", "pytest", "-q", str(TEST)], cwd=ROOT)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Python Matplotlib gate failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
