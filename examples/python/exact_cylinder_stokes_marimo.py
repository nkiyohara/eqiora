"""Inspect the accepted exact-cylinder steady-Stokes result in Marimo."""

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

    return BytesIO, eqiora, eqplot, files, mo


@app.cell
def _(mo):
    mo.md(r"""
    # Exact-cylinder steady Stokes
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
        maximum_boundary_error=1e-4,
        maximum_target_size=0.025,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
    mesh = eqiora.meshing.generate(mesh_plan)
    return mesh, mesh_plan


@app.cell
def _(eqiora, files, geometry):
    source_path = files(eqiora).joinpath("examples", "steady-flow-past-cylinder.eqi")
    model = eqiora.compile(
        path=source_path,
        geometry=geometry,
        parameters={
            "dynamic_viscosity": 1.0e-3,
            "zero_pressure": 0.0,
            "inlet_speed": 0.3,
            "channel_height": geometry.bounds[1][1] - geometry.bounds[1][0],
        },
    )
    return (model,)


@app.cell
def _(eqiora, mesh, model):
    linear = eqiora.solve.Linear(
        relative_tolerance=1e-6,
        absolute_tolerance=1e-13,
        maximum_iterations=10_000,
    )
    stokes_plan = eqiora.resolve(
        model,
        mesh=mesh,
        spatial=eqiora.fem.MiniP1(),
        solve=linear,
        scaling=None,
    )
    return (stokes_plan,)


@app.cell
def _(eqiora, stokes_plan):
    result = eqiora.run(stokes_plan)
    return (result,)


@app.cell
def _(eqplot, result, stokes_plan):
    pressure = result.output(stokes_plan.capability.pressure)
    pressure_figure = eqplot.plot_scalar_field(result, field=stokes_plan.capability.pressure)
    return pressure, pressure_figure


@app.cell
def _(
    geometry,
    mesh,
    mesh_plan,
    mo,
    model,
    pressure,
    result,
    stokes_plan,
):
    result_identity = result.plan_key
    summary = mo.md(
        f"""
        <div data-testid="eqiora-stokes-geometry">
          {type(geometry).__name__} {geometry.digest}
        </div>
        <div data-testid="eqiora-stokes-mesh-plan">
          {type(mesh_plan).__name__} {mesh_plan.source_digest}
        </div>
        <div data-testid="eqiora-stokes-mesh">
          {type(mesh).__name__} {mesh.digest}
        </div>
        <div data-testid="eqiora-stokes-model">
          {type(model).__name__} {model.digest}
        </div>
        <div data-testid="eqiora-stokes-plan">
          {type(stokes_plan).__name__} {stokes_plan.realization_digest}
        </div>
        <div data-testid="eqiora-stokes-result">
          {type(result).__name__} {result_identity};
          pressure vertices {pressure.coefficient_count("vertex")}
        </div>
        """
    )
    summary_presented = True
    summary
    return (summary_presented,)


@app.cell
def _(BytesIO, mo, pressure_figure, summary_presented):
    pressure_png = BytesIO()
    pressure_figure.savefig(pressure_png, format="png")
    pressure_png.seek(0)
    pressure_image = mo.image(
        pressure_png,
        alt="Steady Stokes pressure field",
    )
    pressure_figure_presented = summary_presented
    pressure_image
    return (pressure_figure_presented,)


@app.cell
def _(mo, pressure_figure_presented):
    ready = (
        mo.md("**EQIORA_EXACT_CYLINDER_STOKES_READY**")
        if pressure_figure_presented
        else mo.md("")
    )
    ready
    return


if __name__ == "__main__":
    app.run()
