"""Realize Eqiora's exact cylinder geometry as one bounded chordal mesh."""

import eqiora


def build_mesh_plan() -> tuple[eqiora.meshing.MeshPlan, eqiora.meshing.Mesh]:
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
    return plan, eqiora.meshing.generate(geometry, plan=plan)


def build_mesh() -> eqiora.meshing.Mesh:
    return build_mesh_plan()[1]


if __name__ == "__main__":
    plan, mesh = build_mesh_plan()
    print(mesh.source_digest)
    print(mesh.digest)
    print(plan.provider, plan.boundary_facets)
    print(mesh.dimension, mesh.vertex_count, mesh.cell_count)
    for selection in mesh.selection_names:
        print(selection, mesh.selection_entity_count(selection))
