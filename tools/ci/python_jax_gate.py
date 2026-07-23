#!/usr/bin/env python3
"""Build an installed wheel environment and verify the JAX typed-FFI adapter."""

from __future__ import annotations

import os
import platform
import shutil
import subprocess
import sys
import tempfile
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT
TEST = ROOT / "bindings/python/tests/test_jax.py"
_PROJECT = tomllib.loads((ROOT / "pyproject.toml").read_text(encoding="utf-8"))
_DISTRIBUTION = _PROJECT["tool"]["eqiora-distribution"]
MATURIN = _PROJECT["build-system"]["requires"][0]
PYTEST = _DISTRIBUTION["pytest"]
JAX, JAXLIB = _DISTRIBUTION["tested-jax"]
PYTHON = _DISTRIBUTION["extras-python"]


def venv_python(environment: Path) -> Path:
    """Return the platform-native interpreter path for one virtual environment."""
    if os.name == "nt":
        return environment / "Scripts/python.exe"
    return environment / "bin/python"


def run(argv: list[str], *, cwd: Path = ROOT) -> None:
    environment = os.environ.copy()
    environment.update(
        {
            "EQIORA_REQUIRE_JAX_ABI_PROBE": "1",
            "EQIORA_TEST_JAX_VERSION": JAX.removeprefix("jax=="),
            "EQIORA_TEST_PYTHON_VERSION": PYTHON,
            "JAX_ENABLE_X64": "1",
            "PIP_DISABLE_PIP_VERSION_CHECK": "1",
            "PYTHONNOUSERSITE": "1",
            "XLA_FLAGS": "--xla_force_host_platform_device_count=2",
        }
    )
    subprocess.run(argv, cwd=cwd, env=environment, check=True)


def uv_gate_command(uv: str) -> list[str]:
    """Install the optional extra and exact verified framework release."""
    return [
        uv,
        "run",
        "--directory",
        str(PACKAGE),
        "--isolated",
        "--no-editable",
        "--reinstall-package",
        "eqiora",
        "--extra",
        "jax",
        "--with",
        PYTEST,
        "--with",
        JAX,
        "--with",
        JAXLIB,
        "--python",
        PYTHON,
        "python",
        "-m",
        "pytest",
        "-q",
        str(TEST),
    ]


def require_evidence_platform() -> None:
    """Keep the registered claim on its exact tested platform."""
    machine = platform.machine().lower()
    if platform.system() != "Linux" or machine not in {"x86_64", "amd64"}:
        raise RuntimeError(
            "the JAX evidence gate requires Linux x86_64; "
            f"found {platform.system()} {platform.machine()}"
        )


def main() -> int:
    try:
        require_evidence_platform()
        if uv := shutil.which("uv"):
            run(uv_gate_command(uv))
            return 0
        interpreter = shutil.which("python3.13")
        if interpreter is None:
            raise RuntimeError(
                "the JAX evidence gate requires CPython 3.13 or uv"
            )
        with tempfile.TemporaryDirectory(prefix="eqiora-jax-gate-") as directory:
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
                    JAX,
                    JAXLIB,
                ]
            )
            run(
                [
                    python,
                    "-m",
                    "pip",
                    "install",
                    "--no-build-isolation",
                    ".[jax]",
                ],
                cwd=PACKAGE,
            )
            run(
                [python, "-m", "pytest", "-q", str(TEST)],
                cwd=PACKAGE,
            )
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"Python JAX gate failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
