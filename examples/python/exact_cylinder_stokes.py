"""Solve and optionally plot the accepted exact-cylinder steady-Stokes case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def solve() -> eqiora.fluid.CircularHoleSteadyStokesResult:
    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=(0.2, 0.2),
        radius=0.05,
        boolean_tolerance=1e-10,
    )
    geometry = graph.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
    mesh = eqiora.meshing.circular_hole_chordal(
        geometry,
        max_boundary_error=1e-4,
        required_minimum_mean_ratio=1e-5,
        max_segments=50,
    )
    model = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model.json")
        .read_bytes()
    )
    return eqiora.fluid.solve_exact_cylinder_stokes(
        model=model,
        geometry=geometry,
        mesh=mesh,
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pressure-png",
        type=Path,
        help="save the accepted P1 pressure still (requires eqiora[matplotlib])",
    )
    arguments = parser.parse_args()

    result = solve()
    print(result.run_digest)
    print(result.solve)
    print(
        "pressure",
        result.pressure_minimum,
        result.pressure_maximum,
        "Pa",
    )
    print("cylinder force on fluid", result.cylinder_force_on_fluid, "N/m")
    print("net flux", result.net_flux, "m^2/s")
    if arguments.pressure_png is not None:
        import eqiora.matplotlib as eqplot

        figure = eqplot.plot_pressure(result)
        figure.savefig(arguments.pressure_png, dpi=180)
        print("pressure still", arguments.pressure_png)


if __name__ == "__main__":
    main()
