"""Realize Eqiora's exact cylinder geometry as one bounded chordal mesh."""

import eqiora


def build_mesh_plan() -> tuple[eqiora.meshing.MeshPlan, eqiora.meshing.Mesh]:
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=(0.2, 0.2), radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    geometry = graph.build(fluid, named_topology={
        "fluid": fluid.region,
        "inlet": rectangle.boundaries[0],
        "outlet": rectangle.boundaries[1],
        "walls": rectangle.boundaries[2:4],
        "cylinder": circle.boundaries[0],
    })
    request = eqiora.meshing.MeshRequest(
        eqiora.meshing.GmshMesher(
            maximum_boundary_error=1e-4,
            minimum_mean_ratio=1e-5,
            maximum_boundary_facets=50,
        )
    )
    plan = eqiora.meshing.resolve(geometry, request)
    return plan, eqiora.meshing.generate(geometry, plan=plan)


def build_mesh() -> eqiora.meshing.Mesh:
    return build_mesh_plan()[1]


if __name__ == "__main__":
    plan, mesh = build_mesh_plan()
    print(mesh.source_digest)
    print(mesh.digest)
    print(mesh.production_lineage_digest)
    print(plan.provider, plan.boundary_facets)
    print(mesh.dimension, mesh.vertex_count, mesh.cell_count)
    for selection in mesh.selection_names:
        print(selection, mesh.selection_entity_count(selection))
