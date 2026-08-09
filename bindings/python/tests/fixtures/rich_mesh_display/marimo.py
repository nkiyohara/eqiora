import marimo


__generated_with = "0.23.16"
app = marimo.App(width="medium")


@app.cell
def _():
    import gc
    import weakref

    import eqiora
    import marimo as mo

    return eqiora, gc, mo, weakref


@app.cell
def _(eqiora):
    def make_mesh():
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
        request = eqiora.meshing.MeshRequest(
            maximum_boundary_error=1e-4,
            minimum_mean_ratio=1e-5,
            maximum_boundary_facets=50,
        )
        plan = eqiora.meshing.resolve(geometry, request)
        return eqiora.meshing.generate(geometry, plan=plan)

    return (make_mesh,)


@app.cell
def _(make_mesh):
    mesh = make_mesh()
    coordinates = mesh.coordinates
    cells = mesh.cells
    accepted_snapshot = {
        "text": repr(mesh),
        "source_digest": mesh.source_digest,
        "realized_geometry_digest": mesh.realized_geometry_digest,
        "mesh_digest": mesh.digest,
        "correspondence_digest": mesh.correspondence_digest,
        "realization_digest": mesh.realization_digest,
        "canonical_bytes": mesh.canonical_bytes,
        "coordinates": coordinates,
        "coordinate_bytes": coordinates.tobytes(order="C"),
        "coordinate_shape": coordinates.shape,
        "coordinate_dtype": coordinates.dtype.str,
        "coordinate_writeable": coordinates.flags.writeable,
        "cells": cells,
        "cell_bytes": cells.tobytes(order="C"),
        "cell_shape": cells.shape,
        "cell_dtype": cells.dtype.str,
        "cell_writeable": cells.flags.writeable,
    }
    return accepted_snapshot, mesh


@app.cell
def _(mesh):
    mesh
    return


@app.cell
def _(mo):
    show_third = mo.ui.checkbox(value=True, label="Show third Mesh")
    show_third
    return (show_third,)


@app.cell
def _(mesh):
    mesh
    return


@app.cell
def _(mesh, show_third):
    # This checkbox is labelled "Show third Mesh", so it gates the third Mesh
    # view in document order, which is the view the host oracle clears and
    # re-runs.
    mesh if show_third.value else None
    return


@app.cell
def _(mo):
    show_temporary = mo.ui.checkbox(value=True, label="Show temporary Mesh")
    show_temporary
    return (show_temporary,)


@app.cell
def _(gc, make_mesh, show_temporary, weakref):
    temporary_output = None
    if show_temporary.value:
        temporary_mesh = make_mesh()
        weakref.finalize(
            temporary_mesh,
            print,
            "EQIORA_TEMPORARY_MESH_FINALIZED",
            flush=True,
        )
        temporary_output = temporary_mesh
    else:
        gc.collect()
    temporary_output
    return


@app.cell
def _(mo):
    assert_unchanged_button = mo.ui.button(
        label="Assert accepted Mesh unchanged",
        value=False,
        on_click=lambda value: not value,
    )
    assert_unchanged_button
    return (assert_unchanged_button,)


@app.cell
def _(accepted_snapshot, assert_unchanged_button, mesh, mo):
    if assert_unchanged_button.value:
        assert repr(mesh) == accepted_snapshot["text"]
        assert mesh.source_digest == accepted_snapshot["source_digest"]
        assert (
            mesh.realized_geometry_digest
            == accepted_snapshot["realized_geometry_digest"]
        )
        assert mesh.digest == accepted_snapshot["mesh_digest"]
        assert mesh.correspondence_digest == accepted_snapshot["correspondence_digest"]
        assert mesh.realization_digest == accepted_snapshot["realization_digest"]
        assert mesh.canonical_bytes == accepted_snapshot["canonical_bytes"]
        assert mesh.coordinates is accepted_snapshot["coordinates"]
        assert (
            mesh.coordinates.tobytes(order="C") == accepted_snapshot["coordinate_bytes"]
        )
        assert mesh.coordinates.shape == accepted_snapshot["coordinate_shape"]
        assert mesh.coordinates.dtype.str == accepted_snapshot["coordinate_dtype"]
        assert (
            mesh.coordinates.flags.writeable
            is accepted_snapshot["coordinate_writeable"]
        )
        assert mesh.cells is accepted_snapshot["cells"]
        assert mesh.cells.tobytes(order="C") == accepted_snapshot["cell_bytes"]
        assert mesh.cells.shape == accepted_snapshot["cell_shape"]
        assert mesh.cells.dtype.str == accepted_snapshot["cell_dtype"]
        assert mesh.cells.flags.writeable is accepted_snapshot["cell_writeable"]
        identity_result = mo.md("**EQIORA_MESH_UNCHANGED**")
    else:
        identity_result = mo.md("Mesh identity check is ready.")
    identity_result
    return


if __name__ == "__main__":
    app.run()
