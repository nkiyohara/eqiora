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
def _():
    def current_delegate_model_id(target_mesh):
        # `interfaces.python-rich-mesh-display` evidence O7: the kernel names
        # the close target from the Mesh
        # object it holds. The display hook reuses the open delegate, so this
        # returns the current delegate's model id without any browser-supplied
        # value.
        widget_view_mime = "application/vnd.jupyter.widget-view+json"
        bundle = target_mesh._repr_mimebundle_(include={widget_view_mime})
        return bundle[widget_view_mime]["model_id"]

    def close_delegate(model_id):
        from ipywidgets import Widget

        Widget.widgets[model_id].close()

    return close_delegate, current_delegate_model_id


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
def _(current_delegate_model_id, gc, make_mesh, show_temporary, weakref):
    temporary_output = None
    temporary_model_id = None
    if show_temporary.value:
        temporary_mesh = make_mesh()
        weakref.finalize(
            temporary_mesh,
            print,
            "EQIORA_TEMPORARY_MESH_FINALIZED",
            flush=True,
        )
        temporary_output = temporary_mesh
        # Recorded at display time, kernel-side, for the close trigger below.
        temporary_model_id = current_delegate_model_id(temporary_mesh)
    else:
        gc.collect()
    temporary_output
    return (temporary_model_id,)


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


@app.cell
def _(mo):
    close_main_button = mo.ui.button(
        label="Close accepted Mesh delegate",
        value=0,
        on_click=lambda value: value + 1,
    )
    close_main_button
    return (close_main_button,)


@app.cell
def _(close_delegate, close_main_button, current_delegate_model_id, mesh, mo):
    # Kernel-side close affordance from `interfaces.python-rich-mesh-display`
    # evidence O7: each press closes the
    # accepted Mesh's current delegate, named only from the kernel-held Mesh
    # object — no browser-supplied identifier is read — so the same trigger
    # also closes a fresh delegate created by a later redisplay. The counter
    # button re-runs this cell on every press.
    if close_main_button.value:
        close_delegate(current_delegate_model_id(mesh))
        close_main_result = mo.md("**EQIORA_MAIN_DELEGATE_CLOSED**")
    else:
        close_main_result = mo.md("Main delegate close trigger is ready.")
    close_main_result
    return


@app.cell
def _(mo):
    close_temporary_button = mo.ui.button(
        label="Close temporary Mesh delegate",
        value=0,
        on_click=lambda value: value + 1,
    )
    close_temporary_button
    return (close_temporary_button,)


@app.cell
def _(close_delegate, close_temporary_button, mo, temporary_model_id):
    # Kernel-side close affordance from `interfaces.python-rich-mesh-display`
    # evidence O7: the target is the model
    # id this app recorded at display time. Note the reactive bound: toggling
    # "Show temporary Mesh" re-runs this cell with a fresh recorded id, so the
    # trigger addresses whichever temporary delegate is current.
    if close_temporary_button.value and temporary_model_id is not None:
        close_delegate(temporary_model_id)
        close_temporary_result = mo.md("**EQIORA_TEMPORARY_DELEGATE_CLOSED**")
    else:
        close_temporary_result = mo.md("Temporary delegate close trigger is ready.")
    close_temporary_result
    return


if __name__ == "__main__":
    app.run()
