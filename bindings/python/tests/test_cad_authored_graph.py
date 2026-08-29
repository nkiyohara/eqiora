"""Common GeometryGraph coverage for the admitted solid operation family."""

from __future__ import annotations

import math

import pytest

import eqiora


V1_WIRE = (
    b'{"schema":"eqiora.cad-authored-operation-graph-envelope/v1"'
    b',"encoding":"eqiora.canonical-json/v1","length_unit":"metre"'
    b',"requested_modeling_tolerance_m":1e-9'
    b',"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.5}'
    b',"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle"'
    b',"sketch_plane":"sketch-plane","constraint":"closed-by-construction"'
    b',"x_bounds_m":[-2.0,3.0],"y_bounds_m":[-1.0,2.0]}'
    b',"face":{"id":"profile-face","kind":"one-closed-loop-face"'
    b',"profile":"rectangle-profile","region_count":1}'
    b',"extrusion":{"id":"positive-z-extrusion","kind":"positive-z"'
    b',"face":"profile-face","depth_m":4.0,"repair":"none"}'
    b',"selections":["start-cap","end-cap","profile-x-lower"'
    b',"profile-x-upper","profile-y-lower","profile-y-upper"]}'
)
V1_DIGEST = "919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36"
V2_WIRE = (
    b'{"schema":"eqiora.cad-authored-operation-graph-envelope/v2"'
    b',"encoding":"eqiora.canonical-json/v1","length_unit":"metre"'
    b',"requested_modeling_tolerance_m":1e-10'
    b',"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.0}'
    b',"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle"'
    b',"sketch_plane":"sketch-plane","constraint":"closed-by-construction"'
    b',"x_bounds_m":[-0.04,0.04],"y_bounds_m":[-0.025,0.025]}'
    b',"face":{"id":"profile-face","kind":"one-closed-loop-face"'
    b',"profile":"rectangle-profile","region_count":1}'
    b',"extrusion":{"id":"positive-z-extrusion","kind":"positive-z"'
    b',"face":"profile-face","depth_m":0.02,"repair":"none"}'
    b',"cut_sketch_plane":{"id":"cut-sketch-plane","kind":"on-face"'
    b',"face":"end-cap"}'
    b',"cut_profile":{"id":"circle-profile","kind":"circle"'
    b',"sketch_plane":"cut-sketch-plane","constraint":"closed-by-construction"'
    b',"center_m":[0.02,0.0],"radius_m":0.008}'
    b',"cut_face":{"id":"cut-profile-face","kind":"one-closed-loop-face"'
    b',"profile":"circle-profile","region_count":1}'
    b',"cut":{"id":"circular-through-cut"'
    b',"kind":"difference-through-all-negative-z"'
    b',"target":"positive-z-extrusion","tool_face":"cut-profile-face"'
    b',"requested_tolerance_m":1e-9,"repair":"none"}'
    b',"selections":["start-cap","end-cap","profile-x-lower"'
    b',"profile-x-upper","profile-y-lower","profile-y-upper","cut-wall"]}'
)
V2_DIGEST = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47"


def rectangle(graph: eqiora.geometry.GeometryGraph):
    return graph.rectangle_extrusion(
        x_bounds=(-2.0, 3.0),
        y_bounds=(-1.0, 2.0),
        plane_z=0.5,
        depth=4.0,
        modeling_tolerance=1e-9,
    )


def cut(graph: eqiora.geometry.GeometryGraph):
    base = graph.rectangle_extrusion(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        depth=0.02,
        modeling_tolerance=1e-10,
    )
    return graph.circular_through_cut(
        base,
        center=(0.02, 0.0),
        radius=0.008,
        boolean_tolerance=1e-9,
    )


def test_one_graph_owner_constructs_and_decodes_exact_solid_operations() -> None:
    graph = eqiora.geometry.GeometryGraph()
    v1 = rectangle(graph)
    v2 = cut(graph)

    assert type(v1).__name__ == "GeometrySolidOperation"
    assert v1.canonical_bytes == V1_WIRE
    assert v1.graph_digest == V1_DIGEST
    assert graph.decode_solid(V1_WIRE) == v1
    assert v2.canonical_bytes == V2_WIRE
    assert v2.graph_digest == V2_DIGEST
    assert graph.decode_solid(V2_WIRE) == v2
    assert v1.bounds == ((-2.0, 3.0), (-1.0, 2.0), (0.5, 4.5))
    assert (v1.vertex_count, v1.edge_count, v1.face_count) == (8, 12, 6)
    assert (v1.body_count, v1.closed_shell_count, v1.genus) == (1, 1, 0)
    assert (v1.volume, v1.surface_area, v1.repair) == (60.0, 94.0, "none")


def test_handles_and_build_receipt_remain_revision_bound() -> None:
    graph = eqiora.geometry.GeometryGraph()
    operation = cut(graph)
    handles = {name: operation.face_handle(name) for name in operation.selection_names}

    for name, handle in handles.items():
        assert type(handle).__name__ == "GeometryFaceHandle"
        assert handle.graph_digest == operation.graph_digest
        assert handle.provenance_key == name
        assert operation.resolve_face(handle) == name
        assert operation.face_area(handle) > 0.0

    receipt = graph.build(operation)
    assert type(receipt).__name__ == "GeometryBuildReceipt"
    assert receipt.graph_digest == operation.graph_digest
    assert receipt.provider_profile == "eqiora.cad.analytic-circular-through-cut-v1"
    assert receipt.requested_modeling_tolerance == 1e-10
    assert receipt.requested_boolean_tolerance == 1e-9
    assert receipt.effective_boolean_tolerance == 1e-9
    assert receipt.repair == "none"
    assert tuple(handle.provenance_key for handle in receipt.created) == ("cut-wall",)


def test_solid_cut_can_publish_common_planar_geometry_once_names_are_complete() -> None:
    graph = eqiora.geometry.GeometryGraph()
    operation = cut(graph)
    handles = {name: operation.face_handle(name) for name in operation.selection_names}
    geometry = graph.build(
        operation,
        named_topology={
            "fluid": handles["end-cap"],
            "inlet": handles["profile-x-lower"],
            "outlet": handles["profile-x-upper"],
            "walls": (handles["profile-y-lower"], handles["profile-y-upper"]),
            "cylinder": handles["cut-wall"],
        },
    )

    assert isinstance(geometry, eqiora.geometry.Geometry)
    assert geometry.dimension == 2
    assert geometry.selection_names == ("cylinder", "inlet", "outlet", "walls", "fluid")


def test_foreign_operations_and_handles_fail_before_publication() -> None:
    owner = eqiora.geometry.GeometryGraph()
    foreign = eqiora.geometry.GeometryGraph()
    operation = cut(owner)
    foreign_operation = cut(foreign)

    with pytest.raises(eqiora.ValidationError, match="foreign GeometryGraph"):
        owner.circular_through_cut(
            foreign_operation,
            center=(0.02, 0.0),
            radius=0.004,
            boolean_tolerance=1e-9,
        )

    names = {name: operation.face_handle(name) for name in operation.selection_names}
    names["cylinder"] = foreign_operation.face_handle("cut-wall")
    with pytest.raises(eqiora.ValidationError, match="foreign GeometryGraph"):
        owner.build(operation, named_topology=names)


def test_stale_and_incomplete_names_fail_closed() -> None:
    graph = eqiora.geometry.GeometryGraph()
    base = graph.rectangle_extrusion(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        depth=0.02,
        modeling_tolerance=1e-10,
    )
    stale = base.face_handle("end-cap")
    operation = graph.circular_through_cut(
        base,
        center=(0.02, 0.0),
        radius=0.008,
        boolean_tolerance=1e-9,
    )

    with pytest.raises(eqiora.ValidationError, match="foreign or stale"):
        graph.build(operation, named_topology={"fluid": stale})
    with pytest.raises(eqiora.ValidationError, match="exactly once"):
        graph.build(
            operation,
            named_topology={"fluid": operation.face_handle("end-cap")},
        )


def test_solid_observations_are_native_and_old_public_lifecycle_is_gone() -> None:
    graph = eqiora.geometry.GeometryGraph()
    operation = cut(graph)
    assert math.isclose(operation.volume, 7.597876140340507e-5, rel_tol=4e-15)
    assert math.isclose(operation.surface_area, 0.01380318578948924, rel_tol=4e-15)
    assert not hasattr(operation, "build")
    assert eqiora.geometry.GeometrySolidOperation is type(operation)
    assert not hasattr(eqiora.geometry, "CadAuthoredGraph")
    assert not hasattr(eqiora.geometry, "CadAuthoredSketch")
