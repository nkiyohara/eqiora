#!/usr/bin/env python3
"""Run both registered oracles for the offline Model Package Python projection."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[3]
INSTALLED_WHEEL_GATE = ROOT / "tools/ci/python_package_gate.py"


def run(argv: list[str]) -> None:
    """Run one exact child command from the repository root without a shell."""
    environment = os.environ.copy()
    environment.pop("PYTHONPATH", None)
    subprocess.run(argv, cwd=ROOT, env=environment, check=True)


def main() -> int:
    try:
        run([sys.executable, str(INSTALLED_WHEEL_GATE)])
        cargo = os.environ.get("CARGO", "cargo")
        run(
            [
                cargo,
                "test",
                "--locked",
                "-p",
                "eqiora-python",
                "--test",
                "python_offline_model_package",
            ]
        )
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"offline Model Package Python evidence failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
