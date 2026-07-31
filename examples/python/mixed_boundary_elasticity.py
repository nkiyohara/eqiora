"""Solve and optionally plot the accepted mixed-boundary elasticity case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def solve() -> eqiora.solid.MixedBoundaryElasticityResult:
    source = (
        files(eqiora)
        .joinpath("examples", "mixed-boundary-elasticity.eqi")
        .read_text(encoding="utf-8")
    )
    model = eqiora.compatibility.compile_exact(
        source,
        filename="mixed-boundary-elasticity.eqi",
        codec=eqiora.compatibility.ExactModelCodec.V4,
    )
    return eqiora.solid.solve_mixed_boundary_elasticity(model)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--displacement-png",
        type=Path,
        help="save the accepted displacement still (requires eqiora[matplotlib])",
    )
    parser.add_argument(
        "--scale",
        type=float,
        default=1.0,
        help="visible displacement scale used only by the optional still",
    )
    arguments = parser.parse_args()

    result = solve()
    print(result.run_digest)
    print(result.solve)
    print("constrained reaction", result.constrained_reaction, "N")
    print("integrated body force", result.integrated_body_force, "N")
    if arguments.displacement_png is not None:
        import eqiora.matplotlib as eqplot

        figure = eqplot.plot_displacement(result, scale=arguments.scale)
        figure.savefig(arguments.displacement_png, dpi=160)
        print("displacement still", arguments.displacement_png)


if __name__ == "__main__":
    main()
