"""Inspect the accepted exact-cylinder steady-Stokes result in Marimo."""

import marimo

__generated_with = "0.23.16"
app = marimo.App(width="medium")


@app.cell
def _():
    from importlib.resources import files

    import eqiora
    import eqiora.matplotlib as eqplot
    import marimo as mo

    return eqiora, eqplot, files, mo


@app.cell
def _(mo):
    mo.md(r"""
    # Exact-cylinder steady Stokes
    """)
    return


@app.cell
def _(eqiora):
    geometry_graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
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
    geometry = geometry_graph.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
    return (geometry,)


@app.cell
def _(eqiora, geometry):
    mesh_request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
    mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)
    return mesh, mesh_plan


@app.cell
def _(eqiora, files):
    model_bytes = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model.json")
        .read_bytes()
    )
    model = eqiora.replay(model_bytes)
    return (model,)


@app.cell
def _(eqiora, mesh, model):
    stokes_intent = eqiora.fluid.SteadyStokes(
        length_scale_m=0.41,
        velocity_scale_m_per_s=0.3,
        pressure_scale_pa=0.001 * 0.3 / 0.41,
        relative_tolerance=1e-6,
        absolute_tolerance=1e-13,
        maximum_iterations=10_000,
    )
    stokes_plan = eqiora.fluid.resolve(model, stokes_intent, mesh=mesh)
    return (stokes_plan,)


@app.cell
def _(eqiora, model, stokes_plan):
    run = eqiora.submit(model, plan=stokes_plan)
    result = run.result()
    return result, run


@app.cell
def _(eqiora, eqplot, result):
    pressure = result.snapshots[0]
    evidence = eqiora.fluid.steady_stokes_evidence(result)
    pressure_figure = eqplot.plot_scalar_field(result, field=pressure.field)
    return evidence, pressure_figure


@app.cell
def _(
    evidence,
    geometry,
    mesh,
    mesh_plan,
    mo,
    model,
    pressure_figure,
    result,
    run,
    stokes_plan,
):
    run_identity = evidence.run_digest
    result_identity = result.run_manifest().digest
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
        <div data-testid="eqiora-stokes-run">
          {type(run).__name__} {run_identity}
        </div>
        <div data-testid="eqiora-stokes-result">
          {type(result).__name__} {result_identity}
        </div>
        <div data-testid="eqiora-stokes-evidence">
          {type(evidence).__name__};
          pressure {evidence.pressure_minimum} to {evidence.pressure_maximum} Pa;
          force {evidence.cylinder_force_on_fluid} N/m;
          flux {evidence.net_flux} m^2/s;
          solve {evidence.solve}
        </div>
        """
    )
    ready = mo.md("**EQIORA_EXACT_CYLINDER_STOKES_READY**")
    mo.vstack([summary, pressure_figure, ready])
    return


if __name__ == "__main__":
    app.run()
