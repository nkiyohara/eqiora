#!/usr/bin/env python3
"""Render the current fine-mesh steady-cylinder pressure presentation."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[2]
EXAMPLES = ROOT / "examples" / "python"


def produce(output: Path) -> None:
    sys.path.insert(0, str(EXAMPLES))
    try:
        from exact_cylinder_stokes import solve

        result, pressure_field, _ = solve()
    finally:
        sys.path.pop(0)

    import eqiora.matplotlib as eqplot

    figure = eqplot.plot_scalar_field(result, field=pressure_field)
    figure.axes[0].set_title("Steady Stokes pressure — 0.025 m presentation mesh")
    output.parent.mkdir(parents=True, exist_ok=True)
    figure.savefig(
        output,
        format="png",
        dpi=180,
        metadata={"Software": "Eqiora fine-mesh steady-cylinder presentation v1"},
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    produce(arguments.output)


if __name__ == "__main__":
    main()
