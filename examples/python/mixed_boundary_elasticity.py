"""Run and optionally plot the accepted mixed-boundary elasticity case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def solve() -> tuple[eqiora.Model, eqiora.Result]:
    source = (
        files(eqiora)
        .joinpath("examples", "mixed-boundary-elasticity.eqi")
        .read_text(encoding="utf-8")
    )
    model = eqiora.compile(
        source,
        filename="mixed-boundary-elasticity.eqi",
    )
    # This checked-in reference case follows the tuple enforced by
    # `crates/eqiora-api/src/elasticity.rs::require_supported_intent`.
    intent = eqiora.solid.LinearElasticity(
        cells_per_axis=16,
        relative_tolerance=1.0e-12,
        absolute_tolerance=1.0e-14,
        maximum_iterations=10_000,
    )
    plan = eqiora.solid.resolve(model, intent)
    run = eqiora.submit(model, plan=plan)
    return model, run.result()


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

    model, result = solve()
    evidence = eqiora.solid.linear_elasticity_evidence(result)
    print(result.run_manifest().digest)
    print(evidence.solve)
    print("constrained reaction", evidence.constrained_reaction, "N")
    print("integrated body force", evidence.integrated_body_force, "N")
    if arguments.displacement_png is not None:
        import eqiora.matplotlib as eqplot

        figure = eqplot.plot_deformed_field(
            result,
            field=model.field("displacement"),
            scale=arguments.scale,
        )
        figure.savefig(arguments.displacement_png, dpi=160)
        print("displacement still", arguments.displacement_png)


if __name__ == "__main__":
    main()
