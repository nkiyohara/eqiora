"""Author the exact geometry used by Eqiora's cylinder demonstrations."""

import eqiora


def build_geometry() -> eqiora.geometry.RectangleWithCircularHole:
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
    return graph.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )


if __name__ == "__main__":
    geometry = build_geometry()
    print(geometry.digest)
    for selection in geometry.selection_names:
        print(selection, geometry.selection_dimension(selection))
