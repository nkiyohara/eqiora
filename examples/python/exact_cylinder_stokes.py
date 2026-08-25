"""Solve and optionally plot the accepted exact-cylinder steady-Stokes case."""

import argparse
from importlib.resources import files
from pathlib import Path

import eqiora


def solve() -> eqiora.Result:
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
    geometry = graph.planar_section(
        named_topology={
            "fluid": graph.face_handle("end-cap"),
            "inlet": graph.face_handle("profile-x-lower"),
            "outlet": graph.face_handle("profile-x-upper"),
            "walls": (
                graph.face_handle("profile-y-lower"),
                graph.face_handle("profile-y-upper"),
            ),
            "cylinder": graph.face_handle("cut-wall"),
        }
    )
    request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    plan = eqiora.meshing.resolve(geometry, request)
    mesh = eqiora.meshing.generate(geometry, plan=plan)
    model_bytes = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model.json")
        .read_bytes()
    )
    model = eqiora.replay(model_bytes)
    intent = eqiora.fluid.SteadyStokes(
        length_scale_m=0.41,
        velocity_scale_m_per_s=0.3,
        pressure_scale_pa=0.001 * 0.3 / 0.41,
        relative_tolerance=1e-6,
        absolute_tolerance=1e-13,
        maximum_iterations=10_000,
    )
    plan = eqiora.fluid.resolve(model, intent, mesh=mesh)
    return eqiora.run(model, plan=plan)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--pressure-png",
        type=Path,
        help="save the accepted P1 pressure still (requires eqiora[matplotlib])",
    )
    arguments = parser.parse_args()

    result = solve()
    pressure = result.snapshots[0]
    evidence = eqiora.fluid.steady_stokes_evidence(result)
    print(result.run_manifest().digest)
    print(evidence.solve)
    print(
        "pressure",
        evidence.pressure_minimum,
        evidence.pressure_maximum,
        "Pa",
    )
    print("cylinder force on fluid", evidence.cylinder_force_on_fluid, "N/m")
    print("net flux", evidence.net_flux, "m^2/s")
    if arguments.pressure_png is not None:
        import eqiora.matplotlib as eqplot

        figure = eqplot.plot_scalar_field(result, field=pressure.field)
        figure.savefig(arguments.pressure_png, dpi=180)
        print("pressure still", arguments.pressure_png)


if __name__ == "__main__":
    main()
