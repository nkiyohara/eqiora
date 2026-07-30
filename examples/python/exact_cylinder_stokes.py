"""Solve the accepted exact-cylinder steady-Stokes case with installed Eqiora."""

from importlib.resources import files

import eqiora


def solve() -> eqiora.fluid.CircularHoleSteadyStokesResult:
    geometry = eqiora.geometry.RectangleWithCircularHole(
        bounds=((0.0, 2.2), (0.0, 0.41)),
        circle_center=(0.2, 0.2),
        circle_radius=0.05,
        tolerance=1e-12,
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
    model_v7 = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model-v7.json")
        .read_bytes()
    )
    return eqiora.fluid.solve_exact_cylinder_stokes(
        model_v7=model_v7,
        geometry=geometry,
        mesh=mesh,
    )


if __name__ == "__main__":
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
