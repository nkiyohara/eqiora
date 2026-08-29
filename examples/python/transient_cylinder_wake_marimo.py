"""Inspect a bounded, unverified transient cylinder startup in Marimo."""

import marimo

__generated_with = "0.23.16"
app = marimo.App(width="medium")


@app.cell
def _():
    from importlib.resources import files
    from io import BytesIO

    import eqiora
    import eqiora.matplotlib as eqplot
    import marimo as mo
    import numpy as np

    return BytesIO, eqiora, eqplot, files, mo, np


@app.cell
def _(mo):
    mo.md(r"""
    # Transient cylinder wake

    **Unverified product example.** This demonstrates the public transient
    workflow and makes no benchmark, convergence, drag/lift, or shedding claim.
    """)
    return


@app.cell
def _(eqiora):
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
    return (geometry,)


@app.cell
def _(eqiora, geometry):
    mesh_request = eqiora.meshing.GmshMesher(
        maximum_boundary_error=1.0e-4,
        maximum_target_size=0.05,
        minimum_mean_ratio=1.0e-5,
        maximum_boundary_facets=50,
    )
    mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
    mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
    return mesh, mesh_plan


@app.cell
def _(eqiora, files, geometry):
    source_root = files(eqiora).joinpath("examples")
    parameters = {
        "dynamic_viscosity": 1.0e-3,
        "zero_pressure": 0.0,
        "inlet_speed": 0.3,
        "channel_height": geometry.bounds[1][1] - geometry.bounds[1][0],
    }
    return parameters, source_root


@app.cell
def _(eqiora, geometry, mesh, parameters, source_root):
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
    return linear, steady_plan, steady_result


@app.cell
def _(eqiora, geometry, linear, mesh, parameters, source_root):
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
    return model, plan


@app.cell
def _(eqiora, mesh, np, plan, steady_plan, steady_result):
    steady_velocity = steady_result.output(steady_plan.capability.velocity)
    steady_pressure = steady_result.output(steady_plan.capability.pressure)
    state = eqiora.State.initial(
        plan,
        time_s=0.0,
        fields=(
            eqiora.InitialField(
                plan.capability.velocity,
                vertex_values=np.asarray(steady_velocity.vertex_values).reshape(
                    mesh.vertex_count, 2
                ),
                cell_values=np.asarray(steady_velocity.cell_bubble_values).reshape(
                    mesh.cell_count, 2
                ),
            ),
            eqiora.InitialField(
                plan.capability.pressure,
                vertex_values=np.asarray(steady_pressure.vertex_values),
            ),
        ),
    )
    return (state,)


@app.cell
def _(eqiora, geometry, plan, state):
    result = eqiora.run(plan, state=state, steps=10, output_steps=tuple(range(1, 11)))
    accepted = result.trajectory.state(10)
    vorticity = accepted.curl(plan.capability.velocity)
    cylinder_force = accepted.boundary_force(geometry.selection("cylinder"))
    front_pressure = accepted.sample(plan.capability.pressure, at=(0.15, 0.2))
    rear_pressure = accepted.sample(plan.capability.pressure, at=(0.25, 0.2))
    return accepted, cylinder_force, front_pressure, rear_pressure, result, vorticity


@app.cell
def _(
    accepted,
    cylinder_force,
    front_pressure,
    geometry,
    mesh,
    mesh_plan,
    mo,
    model,
    plan,
    rear_pressure,
    result,
    vorticity,
):
    vorticity_values = vorticity.values("cell")
    summary = mo.md(
        f"""
        <div data-testid="eqiora-wake-status">
          <strong>UNVERIFIED PRODUCT EXAMPLE</strong> — no benchmark acceptance is claimed.
        </div>
        <div data-testid="eqiora-wake-geometry">
          {type(geometry).__name__} {geometry.digest}
        </div>
        <div data-testid="eqiora-wake-mesh-plan">
          {type(mesh_plan).__name__} {mesh_plan.source_digest}
        </div>
        <div data-testid="eqiora-wake-mesh">
          {type(mesh).__name__} {mesh.digest}
        </div>
        <div data-testid="eqiora-wake-model">
          {type(model).__name__} {model.digest}
        </div>
        <div data-testid="eqiora-wake-plan">
          {type(plan).__name__} {plan.identity}
        </div>
        <div data-testid="eqiora-wake-result">
          {type(result).__name__} {result.trajectory.digest};
          accepted step {accepted.step} at {accepted.time_s} s;
          cell vorticity range {float(vorticity_values.min())} to
          {float(vorticity_values.max())} s^-1;
          force on cylinder {cylinder_force.on_selection} N/m;
          pressure probes {front_pressure.value} and {rear_pressure.value} Pa
        </div>
        """
    )
    summary_presented = True
    summary
    return (summary_presented,)


@app.cell
def _(BytesIO, eqplot, mo, result, summary_presented, vorticity):
    vorticity_figure = eqplot.plot_scalar_field(
        result.trajectory,
        step=10,
        field=vorticity,
    )
    vorticity_png = BytesIO()
    vorticity_figure.savefig(vorticity_png, format="png")
    vorticity_png.seek(0)
    vorticity_image = mo.image(
        vorticity_png,
        alt="Cell-average vorticity after ten accepted transient cylinder-flow startup steps",
    )
    figure_presented = summary_presented
    vorticity_image
    return (figure_presented,)


@app.cell
def _(figure_presented, mo):
    ready = (
        mo.md("**EQIORA_TRANSIENT_CYLINDER_WAKE_READY**")
        if figure_presented
        else mo.md("")
    )
    ready
    return


if __name__ == "__main__":
    app.run()
