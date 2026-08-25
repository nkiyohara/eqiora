"""Author the exact geometry used by Eqiora's cylinder demonstrations."""

import eqiora


def build_geometry() -> eqiora.geometry.Geometry:
    base_sketch = eqiora.geometry.CadAuthoredSketch.rectangle_xy(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    )
    base = base_sketch.extrude_positive_z(depth=1.0)
    cut_sketch = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"),
        center=(0.2, 0.2),
        radius=0.05,
    )
    graph = base.through_cut(cut_sketch, boolean_tolerance=1e-10)
    return graph.planar_section(
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


if __name__ == "__main__":
    geometry = build_geometry()
    print(geometry.digest)
    for selection in geometry.selection_names:
        print(selection, geometry.selection_dimension(selection))
