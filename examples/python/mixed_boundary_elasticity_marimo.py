"""Inspect the accepted mixed-boundary elasticity result in Marimo."""

import marimo

__generated_with = "0.23.16"
app = marimo.App(width="medium")


@app.cell
def _():
    from io import BytesIO

    import eqiora.matplotlib as eqplot
    import marimo as mo

    from mixed_boundary_elasticity import solve

    return BytesIO, eqplot, mo, solve


@app.cell
def _(mo):
    mo.md(r"""
    # Mixed-boundary linear elasticity

    Run the shared installed-Python workflow, then present its common
    displacement output with a caller-owned figure.
    """)
    return


@app.cell
def _(solve):
    plan, result = solve()
    displacement = result.output(plan.field)
    return displacement, plan, result


@app.cell
def _(mo, plan, displacement, result):
    summary = mo.md(
        f"""
        <div data-testid="eqiora-elasticity-plan">
          {type(plan).__name__} {plan.identity}
        </div>
        <div data-testid="eqiora-elasticity-result">
          {type(result).__name__} {result.plan_key};
          displacement vertices {displacement.vertex_count}
        </div>
        """
    )
    summary
    return


@app.cell
def _(BytesIO, eqplot, mo, plan, result):
    displacement_figure = eqplot.plot_deformed_field(
        result,
        field=plan.field,
        scale=1.0,
    )
    displacement_png = BytesIO()
    displacement_figure.savefig(displacement_png, format="png")
    displacement_png.seek(0)
    mo.vstack(
        [
            mo.image(
                displacement_png,
                alt=(
                    "Reference and deformed meshes for the bounded mixed-boundary "
                    "linear-elasticity demonstration; deformation scale 1."
                ),
            ),
            mo.md("**EQIORA_MIXED_BOUNDARY_ELASTICITY_READY**"),
        ]
    )
    return


if __name__ == "__main__":
    app.run()
