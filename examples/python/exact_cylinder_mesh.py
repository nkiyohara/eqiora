"""Realize Eqiora's exact cylinder geometry as one bounded chordal mesh."""

import eqiora


def build_mesh() -> eqiora.meshing.CircularHoleChordalMesh:
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
    return eqiora.meshing.circular_hole_chordal(
        geometry,
        max_boundary_error=1e-4,
        required_minimum_mean_ratio=1e-5,
        max_segments=50,
    )


if __name__ == "__main__":
    mesh = build_mesh()
    print(mesh.source_digest)
    print(mesh.mesh_digest)
    print(mesh.dimension, mesh.vertex_count, mesh.cell_count, mesh.circle_segments)
    for selection in mesh.selection_names:
        print(selection, mesh.selection_entity_count(selection))
