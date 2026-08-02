"""Solve and optionally plot the accepted two-step fixed-reference FSI case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def solve() -> eqiora.fsi.FixedReferenceFsiResult:
    source = (
        files(eqiora)
        .joinpath("examples", "fixed-reference-fsi.eqi")
        .read_text(encoding="utf-8")
    )
    model = eqiora.compile(
        source,
        filename="fixed-reference-fsi.eqi",
    )
    return eqiora.fsi.solve_fixed_reference_fsi(model)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fsi-png",
        type=Path,
        help="save one accepted FSI step still (requires eqiora[matplotlib])",
    )
    parser.add_argument(
        "--step",
        type=int,
        choices=(1, 2),
        default=2,
        help="accepted step ordinal used only by the optional still",
    )
    parser.add_argument(
        "--displacement-scale",
        type=float,
        default=12.0,
        help="visible solid displacement scale used only by the optional still",
    )
    arguments = parser.parse_args()

    result = solve()
    print(result.trajectory.run_digest)
    print(result.trajectory.digest)
    for step in result.steps:
        print(
            f"step {step.ordinal} at {step.time_s:g} s",
            step.solve,
            f"energy defect {step.energy_defect_j_per_m:.6e} J/m",
        )
    if arguments.fsi_png is not None:
        import eqiora.matplotlib as eqplot

        figure = eqplot.plot_fixed_reference_fsi(
            result,
            step=arguments.step,
            displacement_scale=arguments.displacement_scale,
        )
        figure.savefig(arguments.fsi_png, dpi=160)
        print("FSI still", arguments.fsi_png)


if __name__ == "__main__":
    main()
