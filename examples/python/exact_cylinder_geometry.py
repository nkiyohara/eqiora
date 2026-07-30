"""Author the exact geometry used by Eqiora's cylinder demonstrations."""

import eqiora


def build_geometry() -> eqiora.geometry.RectangleWithCircularHole:
    return eqiora.geometry.RectangleWithCircularHole(
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


if __name__ == "__main__":
    geometry = build_geometry()
    print(geometry.digest)
    for selection in geometry.selection_names:
        print(selection, geometry.selection_dimension(selection))
