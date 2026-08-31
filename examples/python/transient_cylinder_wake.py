"""Run and optionally plot a bounded, unverified transient cylinder startup."""

import argparse
from importlib.resources import files
from pathlib import Path

import numpy as np

import eqiora


def solve() -> tuple[
    eqiora.Plan,
    eqiora.Result,
    eqiora.trajectory.DerivedFieldSnapshot,
    eqiora.trajectory.BoundaryForce,
    eqiora.trajectory.FieldSample,
    eqiora.trajectory.FieldSample,
]:
    geometry_graph = eqiora.geometry.GeometryGraph()
    rectangle = geometry_graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = geometry_graph.circle(center=(0.2, 0.2), radius=0.05)
    fluid = geometry_graph.subtract(rectangle, circle)
    geometry = geometry_graph.build(
        fluid,
        named_topology={
            "fluid": fluid.region,
            "inlet": rectangle.boundaries[0],
            "outlet": rectangle.boundaries[1],
            "walls": rectangle.boundaries[2:4],
            "cylinder": circle.boundaries[0],
        },
    )
    mesh_request = eqiora.meshing.GmshMesher(
        maximum_boundary_error=1.0e-4,
        maximum_target_size=0.025,
        minimum_mean_ratio=1.0e-5,
        maximum_boundary_facets=50,
    )
    mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
    mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
    source_root = files(eqiora).joinpath("examples")
    parameters = {
        "dynamic_viscosity": 1.0e-3,
        "zero_pressure": 0.0,
        "inlet_speed": 0.3,
        "channel_height": geometry.bounds[1][1] - geometry.bounds[1][0],
    }
    steady_model = eqiora.compile(
        path=source_root.joinpath("steady-flow-past-cylinder.eqi"),
        geometry=geometry,
        parameters=parameters,
    )
    linear = eqiora.solve.Linear(
        relative_tolerance=1.0e-6,
        absolute_tolerance=1.0e-9,
        maximum_iterations=20_000,
    )
    steady_plan = eqiora.resolve(
        steady_model,
        mesh=mesh,
        spatial=eqiora.fem.MiniP1(),
        solve=linear,
        scaling=None,
    )
    steady_result = eqiora.run(steady_plan)

    model = eqiora.compile(
        path=source_root.joinpath("transient-flow-past-cylinder.eqi"),
        geometry=geometry,
        parameters={"density": 1.0, **parameters},
    )
    plan = eqiora.resolve(
        model,
        mesh=mesh,
        spatial=eqiora.fem.MiniP1(),
        temporal=eqiora.time.BackwardEuler(0.01),
        solve=eqiora.solve.Newton(linear=linear),
        scaling=eqiora.fluid.IncompressibleScaling(
            length_m=0.41,
            velocity_m_per_s=0.3,
            pressure_pa=0.09,
        ),
    )
    steady_velocity = steady_result.output(steady_plan.capability.velocity)
    steady_pressure = steady_result.output(steady_plan.capability.pressure)
    state = eqiora.State.initial(
        plan,
        time_s=0.0,
        fields=(
            eqiora.InitialField(
                plan.capability.velocity,
                vertex_values=np.asarray(steady_velocity.values("vertex")).reshape(
                    mesh.vertex_count, 2
                ),
                cell_values=np.asarray(steady_velocity.values("cell-bubble")).reshape(
                    mesh.cell_count, 2
                ),
            ),
            eqiora.InitialField(
                plan.capability.pressure,
                vertex_values=np.asarray(steady_pressure.values("vertex")),
            ),
        ),
    )
    result = eqiora.run(plan, state=state, steps=10, output_steps=tuple(range(1, 11)))
    accepted = result.trajectory.state(10)
    vorticity = accepted.curl(plan.capability.velocity)
    cylinder_force = accepted.boundary_force(geometry.selection("cylinder"))
    front_pressure = accepted.sample(plan.capability.pressure, at=(0.15, 0.2))
    rear_pressure = accepted.sample(plan.capability.pressure, at=(0.25, 0.2))
    return plan, result, vorticity, cylinder_force, front_pressure, rear_pressure


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--vorticity-png",
        type=Path,
        help="save the accepted vorticity still (requires eqiora[matplotlib])",
    )
    arguments = parser.parse_args()

    plan, result, vorticity, cylinder_force, front_pressure, rear_pressure = solve()
    accepted = result.trajectory.state(10)
    values = vorticity.values("cell")
    print("UNVERIFIED PRODUCT EXAMPLE — no benchmark acceptance is claimed")
    print("plan", plan.identity)
    print("trajectory", result.trajectory.digest)
    print("state", accepted.step, accepted.time_s, accepted.digest)
    print("vorticity", float(values.min()), float(values.max()), "s^-1")
    print("force on cylinder", cylinder_force.on_selection, "N/m")
    print("pressure probes", front_pressure.value, rear_pressure.value, "Pa")
    print("pressure difference", front_pressure.value - rear_pressure.value, "Pa")
    if arguments.vorticity_png is not None:
        import eqiora.matplotlib as eqplot

        figure = eqplot.plot_scalar_field(
            result.trajectory,
            step=10,
            field=vorticity,
        )
        figure.savefig(arguments.vorticity_png, dpi=180)
        print("vorticity still", arguments.vorticity_png)


if __name__ == "__main__":
    main()
