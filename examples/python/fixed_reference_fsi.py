"""Solve and optionally plot the accepted two-step fixed-reference FSI case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def compile_model() -> eqiora.Model:
    source = (
        files(eqiora)
        .joinpath("examples", "fixed-reference-fsi.eqi")
        .read_text(encoding="utf-8")
    )
    return eqiora.compile(
        source,
        filename="fixed-reference-fsi.eqi",
    )


def solve(model: eqiora.Model) -> eqiora.fsi.FixedReferenceFsiResult:
    return eqiora.fsi.solve_fixed_reference_fsi(model)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--fsi-png",
        type=Path,
        help="save the accepted deformed field still (requires eqiora[matplotlib])",
    )
    parser.add_argument(
        "--pressure-png",
        type=Path,
        help="save the accepted pressure field still (requires eqiora[matplotlib])",
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

    model = compile_model()
    result = solve(model)
    print(result.trajectory.run_digest)
    print(result.trajectory.digest)
    for step in result.steps:
        print(
            f"step {step.ordinal} at {step.time_s:g} s",
            step.solve,
            f"energy defect {step.energy_defect_j_per_m:.6e} J/m",
        )
    if arguments.fsi_png is not None or arguments.pressure_png is not None:
        import eqiora.matplotlib as eqplot

        if arguments.pressure_png is not None:
            pressure = eqplot.plot_scalar_field(
                result.trajectory,
                step=arguments.step,
                field=model.field("fluid_pressure"),
            )
            pressure.savefig(arguments.pressure_png, dpi=160)
            print("Pressure field still", arguments.pressure_png)
        if arguments.fsi_png is not None:
            deformed = eqplot.plot_deformed_field(
                result.trajectory,
                step=arguments.step,
                field=model.field("solid_displacement"),
                scale=arguments.displacement_scale,
            )
            deformed.savefig(arguments.fsi_png, dpi=160)
            print("Deformed field still", arguments.fsi_png)


if __name__ == "__main__":
    main()
