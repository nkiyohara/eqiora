"""Inspect accepted planar Geometry and Mesh values in the shared viewer."""

import marimo

__generated_with = "0.23.16"
app = marimo.App(width="full")


@app.cell
def _():
    import eqiora
    import marimo as mo

    return eqiora, mo


@app.cell
def _(mo):
    mo.md(r"""
    # Shared semantic viewer

    This read-only V0--V3 surface presents accepted Eqiora values; browser
    state is not scientific evidence.
    """)
    return


@app.cell
def _(eqiora):
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.0), y_bounds=(-1.0, 1.0))
    geometry = graph.build(
        rectangle,
        named_topology={
            "region": rectangle.region,
            "left": rectangle.boundaries[0],
            "right": rectangle.boundaries[1],
            "bottom": rectangle.boundaries[2],
            "top": rectangle.boundaries[3],
        },
    )
    mesh_request = eqiora.meshing.CartesianMesher(cells=(4, 3))
    mesh = eqiora.meshing.generate(eqiora.meshing.resolve(geometry, mesh_request))
    return geometry, mesh


@app.cell
def _(eqiora, geometry, mesh):
    viewer = eqiora.View().add(geometry).add(mesh)
    viewer
    return (viewer,)


@app.cell
def _(geometry, mesh, mo, viewer):
    summary = mo.md(
        f"""
        <div data-testid="eqiora-viewer-geometry">
          {type(geometry).__name__} {geometry.digest}
        </div>
        <div data-testid="eqiora-viewer-mesh">
          {type(mesh).__name__} {mesh.digest};
          {mesh.vertex_count} accepted vertices; {mesh.cell_count} accepted cells
        </div>
        <div data-testid="eqiora-viewer-python-host">
          {type(viewer).__name__} installed-wheel anywidget host
        </div>
        **EQIORA_SHARED_SEMANTIC_VIEWER_READY**
        """
    )
    summary
    return


if __name__ == "__main__":
    app.run()
