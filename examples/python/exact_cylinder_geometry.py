"""Author the exact geometry used by Eqiora's cylinder demonstrations."""

import eqiora


def build_geometry() -> eqiora.geometry.Geometry:
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=(0.2, 0.2), radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    return graph.build(fluid, named_topology={
        "fluid": fluid.region,
        "inlet": rectangle.boundaries[0],
        "outlet": rectangle.boundaries[1],
        "walls": rectangle.boundaries[2:4],
        "cylinder": circle.boundaries[0],
    })


if __name__ == "__main__":
    geometry = build_geometry()
    print(geometry.digest)
    for selection in geometry.selection_names:
        print(selection, geometry.selection_dimension(selection))
