"""Installed-package oracle for bounded authored-sketch composition."""

from __future__ import annotations

import ast
import gc
import inspect
import itertools
from pathlib import Path
import subprocess
import sys
from typing import Callable

import pytest

import eqiora


# Inherited from geometry.cad-authored-rectangle-extrusion. This adapter does
# not own or reinterpret the graph wire or digest.
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

# Inherited from geometry.cad-authored-circular-through-cut.
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
DFG_SECTION_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
EXPECTED_GEOMETRY_ALL = [
    "CadAuthoredBuild",
    "CadAuthoredFaceHandle",
    "CadAuthoredGraph",
    "CadAuthoredSketch",
    "Geometry",
]
WRONG_FACE_MESSAGE = "authored CAD circle sketch requires a v1 end-cap face handle"
FOREIGN_GRAPH_MESSAGE = (
    "CAD face handle belongs to a foreign authored graph identity or wire variant"
)
RECTANGLE_AS_CUT_MESSAGE = (
    "circular through-cut requires the admitted circle-on-face sketch"
)
CIRCLE_AS_EXTRUSION_MESSAGE = (
    "positive-z extrusion requires the admitted rectangle sketch"
)
SECOND_CUT_MESSAGE = (
    "authored CAD v2 admits exactly one cut after the rectangle extrusion"
)
NONFINITE = (float("nan"), float("inf"), float("-inf"))
NONPOSITIVE_OR_NONFINITE = (0.0, -0.0, -1.0, *NONFINITE)

GRAPH_PROPERTIES = (
    "x_bounds",
    "y_bounds",
    "plane_z",
    "extrusion_depth",
    "requested_modeling_tolerance",
    "requested_boolean_tolerance",
    "cut_center",
    "cut_radius",
    "bounds",
    "vertex_count",
    "edge_count",
    "face_count",
    "closed_shell_count",
    "body_count",
    "genus",
    "volume",
    "surface_area",
    "repair",
    "selection_names",
)
BUILD_PROPERTIES = (
    "graph_digest",
    "provider_profile",
    "requested_modeling_tolerance",
    "requested_boolean_tolerance",
    "effective_boolean_tolerance",
    "maximum_position_discrepancy",
    "maximum_area_discrepancy",
    "maximum_volume_discrepancy",
    "repair",
)
LINEAGE_PROPERTIES = (
    "retained_unchanged",
    "retained_modified",
    "created",
    "deleted",
    "split",
    "merged",
)


def rectangle_sketch(
    *,
    x_bounds: tuple[float, float] = (-2.0, 3.0),
    y_bounds: tuple[float, float] = (-1.0, 2.0),
    plane_z: float = 0.5,
    modeling_tolerance: float = 1e-9,
):
    return eqiora.geometry.CadAuthoredSketch.rectangle_xy(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=plane_z,
        modeling_tolerance=modeling_tolerance,
    )


def compatibility_rectangle_graph(**overrides: object):
    arguments = {
        "x_bounds": (-2.0, 3.0),
        "y_bounds": (-1.0, 2.0),
        "plane_z": 0.5,
        "depth": 4.0,
        "modeling_tolerance": 1e-9,
    }
    arguments.update(overrides)
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(**arguments)


def symmetric_base(*, explicit: bool):
    arguments = {
        "x_bounds": (-0.04, 0.04),
        "y_bounds": (-0.025, 0.025),
        "plane_z": 0.0,
        "depth": 0.02,
        "modeling_tolerance": 1e-10,
    }
    if explicit:
        depth = arguments.pop("depth")
        return rectangle_sketch(**arguments).extrude_positive_z(depth=depth)
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(**arguments)


def four_cut_routes(
    *,
    x_bounds: tuple[float, float],
    y_bounds: tuple[float, float],
    plane_z: float,
    depth: float,
    modeling_tolerance: float,
    center: tuple[float, float],
    radius: float,
    boolean_tolerance: float,
) -> tuple[object, object, object, object]:
    explicit_base = rectangle_sketch(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=plane_z,
        modeling_tolerance=modeling_tolerance,
    ).extrude_positive_z(depth=depth)
    compatibility_base = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=plane_z,
        depth=depth,
        modeling_tolerance=modeling_tolerance,
    )
    explicit_circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        explicit_base.face_handle("end-cap"),
        center=center,
        radius=radius,
    )
    compatibility_circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        compatibility_base.face_handle("end-cap"),
        center=center,
        radius=radius,
    )
    return (
        explicit_base.through_cut(explicit_circle, boolean_tolerance=boolean_tolerance),
        compatibility_base.circular_through_cut(
            center=center,
            radius=radius,
            boolean_tolerance=boolean_tolerance,
        ),
        explicit_base.circular_through_cut(
            center=center,
            radius=radius,
            boolean_tolerance=boolean_tolerance,
        ),
        compatibility_base.through_cut(
            compatibility_circle, boolean_tolerance=boolean_tolerance
        ),
    )


def symmetric_routes(
    center: tuple[float, float] = (0.02, 0.0),
) -> tuple[object, object, object, object]:
    return four_cut_routes(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        depth=0.02,
        modeling_tolerance=1e-10,
        center=center,
        radius=0.008,
        boolean_tolerance=1e-9,
    )


def keys(handles: tuple[object, ...]) -> tuple[str, ...]:
    return tuple(handle.provenance_key for handle in handles)


def assert_lineage_partition(graph: object, build: object) -> None:
    lineage = tuple(
        name
        for property_name in LINEAGE_PROPERTIES
        for name in keys(getattr(build, property_name))
    )
    assert len(lineage) == len(set(lineage)) == len(graph.selection_names)
    assert set(lineage) == set(graph.selection_names)


def assert_route_equivalence(explicit: object, compatibility: object) -> None:
    assert explicit == compatibility
    assert explicit.canonical_bytes == compatibility.canonical_bytes
    assert explicit.graph_digest == compatibility.graph_digest
    for property_name in GRAPH_PROPERTIES:
        assert getattr(explicit, property_name) == getattr(compatibility, property_name)

    explicit_handles = tuple(
        explicit.face_handle(name) for name in explicit.selection_names
    )
    compatibility_handles = tuple(
        compatibility.face_handle(name) for name in compatibility.selection_names
    )
    assert explicit_handles == compatibility_handles
    for explicit_handle, compatibility_handle in zip(
        explicit_handles, compatibility_handles, strict=True
    ):
        assert explicit_handle.canonical_bytes == compatibility_handle.canonical_bytes
        assert explicit.resolve_face(explicit_handle) == compatibility.resolve_face(
            compatibility_handle
        )
        assert explicit.face_area(explicit_handle) == compatibility.face_area(
            compatibility_handle
        )
        assert explicit.face_boundary_loop_count(
            explicit_handle
        ) == compatibility.face_boundary_loop_count(compatibility_handle)
        assert explicit.rectangular_face_vertices(
            explicit_handle
        ) == compatibility.rectangular_face_vertices(compatibility_handle)
        assert explicit.rectangular_face_centroid(
            explicit_handle
        ) == compatibility.rectangular_face_centroid(compatibility_handle)
        assert explicit.planar_face_outward_normal(
            explicit_handle
        ) == compatibility.planar_face_outward_normal(compatibility_handle)

    explicit_build = explicit.build()
    compatibility_build = compatibility.build()
    for property_name in BUILD_PROPERTIES:
        assert getattr(explicit_build, property_name) == getattr(
            compatibility_build, property_name
        )
    for property_name in LINEAGE_PROPERTIES:
        assert getattr(explicit_build, property_name) == getattr(
            compatibility_build, property_name
        )
    assert_lineage_partition(explicit, explicit_build)

    replayed = eqiora.geometry.CadAuthoredGraph.decode_canonical(
        explicit.canonical_bytes
    )
    assert replayed == explicit
    assert replayed.canonical_bytes == explicit.canonical_bytes
    assert replayed.graph_digest == explicit.graph_digest


def assert_native_validation(
    operation: Callable[[], object],
    *,
    compatibility_operation: Callable[[], object] | None = None,
    expected_message: str | None = None,
    graphs: tuple[object, ...] = (),
    sketch_pairs: tuple[tuple[object, object], ...] = (),
    handles: tuple[object, ...] = (),
) -> None:
    graph_states = tuple(
        (graph.canonical_bytes, graph.graph_digest) for graph in graphs
    )
    handle_states = tuple(
        (handle.canonical_bytes, handle.graph_digest, handle.provenance_key)
        for handle in handles
    )
    assert (compatibility_operation is None) != (expected_message is None)
    if compatibility_operation is not None:
        with pytest.raises(eqiora.ValidationError) as compatibility_caught:
            compatibility_operation()
        compatibility_error = compatibility_caught.value
        assert compatibility_error.category == "validation"
        assert len(compatibility_error.diagnostics) == 1
        compatibility_diagnostic = compatibility_error.diagnostics[0]
        assert compatibility_diagnostic.source == "kernel"
        assert compatibility_diagnostic.severity == "error"
        assert compatibility_diagnostic.code == "EQ0901"
        assert compatibility_diagnostic.message
        expected_message = compatibility_diagnostic.message

    with pytest.raises(eqiora.ValidationError) as caught:
        operation()

    error = caught.value
    assert error.category == "validation"
    assert len(error.diagnostics) == 1
    diagnostic = error.diagnostics[0]
    assert diagnostic.source == "kernel"
    assert diagnostic.severity == "error"
    assert diagnostic.code == "EQ0901"
    assert diagnostic.message
    assert diagnostic.message == expected_message

    assert (
        tuple((graph.canonical_bytes, graph.graph_digest) for graph in graphs)
        == graph_states
    )
    for sketch, snapshot in sketch_pairs:
        assert sketch == snapshot
    assert (
        tuple(
            (handle.canonical_bytes, handle.graph_digest, handle.provenance_key)
            for handle in handles
        )
        == handle_states
    )


def test_explicit_rectangle_replays_the_exact_v1_authority() -> None:
    sketch = rectangle_sketch()
    explicit = sketch.extrude_positive_z(depth=4.0)
    reused = sketch.extrude_positive_z(depth=4.0)
    inline = rectangle_sketch().extrude_positive_z(depth=4.0)
    compatibility = compatibility_rectangle_graph()

    for graph in (explicit, reused, inline):
        assert_route_equivalence(graph, compatibility)
        assert len(graph.canonical_bytes) == 731
        assert graph.canonical_bytes == V1_WIRE
        assert graph.graph_digest == V1_DIGEST

    build = explicit.build()
    assert build.retained_unchanged == ()
    assert build.retained_modified == ()
    assert len(build.created) == len(explicit.selection_names)
    assert build.deleted == build.split == build.merged == ()
    assert_lineage_partition(explicit, build)


def test_all_four_cut_compositions_replay_the_exact_v2_authority() -> None:
    routes = symmetric_routes()
    compatibility = routes[1]
    for graph in routes:
        assert_route_equivalence(graph, compatibility)
        assert len(graph.canonical_bytes) == 1292
        assert graph.canonical_bytes == V2_WIRE
        assert graph.graph_digest == V2_DIGEST

    build = routes[0].build()
    assert set(keys(build.retained_unchanged)) == {
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    }
    assert set(keys(build.retained_modified)) == {"start-cap", "end-cap"}
    assert len(build.created) == 1
    assert build.deleted == build.split == build.merged == ()
    assert_lineage_partition(routes[0], build)


def test_separate_dfg_fixture_replays_the_exact_planar_authority() -> None:
    routes = four_cut_routes(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
        center=(0.2, 0.2),
        radius=0.05,
        boolean_tolerance=1e-10,
    )
    compatibility = routes[1]
    for graph in routes:
        assert_route_equivalence(graph, compatibility)

    sections = tuple(
        graph.planar_circular_section(
            classification_tolerance=1e-12,
            region="fluid",
            x_lower="inlet",
            x_upper="outlet",
            y_lower="walls",
            y_upper="walls",
            hole="cylinder",
        )
        for graph in routes
    )
    assert all(section == sections[0] for section in sections)
    assert all(
        section.canonical_bytes == sections[0].canonical_bytes for section in sections
    )
    assert len(sections[0].canonical_bytes) == 511
    assert sections[0].digest == DFG_SECTION_DIGEST


def test_rectangle_and_circle_signed_zero_ownership_are_separate() -> None:
    positive_rectangle = rectangle_sketch(
        x_bounds=(0.0, 0.04),
        y_bounds=(0.0, 0.025),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    ).extrude_positive_z(depth=0.02)
    negative_rectangle = rectangle_sketch(
        x_bounds=(-0.0, 0.04),
        y_bounds=(-0.0, 0.025),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    ).extrude_positive_z(depth=0.02)
    assert positive_rectangle == negative_rectangle
    assert positive_rectangle.canonical_bytes == negative_rectangle.canonical_bytes
    assert positive_rectangle.graph_digest == negative_rectangle.graph_digest

    positive_center = symmetric_routes((0.02, 0.0))[0]
    negative_center = symmetric_routes((0.02, -0.0))[0]
    assert positive_center == negative_center
    assert positive_center.canonical_bytes == negative_center.canonical_bytes == V2_WIRE
    assert positive_center.graph_digest == negative_center.graph_digest == V2_DIGEST


def test_sketch_equality_rejects_materially_distinct_values_and_variants() -> None:
    rectangle = rectangle_sketch()
    equal_rectangle = rectangle_sketch()
    changed_bounds = rectangle_sketch(x_bounds=(-2.0, 4.0))
    changed_plane = rectangle_sketch(plane_z=0.75)
    changed_tolerance = rectangle_sketch(modeling_tolerance=2e-9)
    assert rectangle == equal_rectangle
    assert rectangle != changed_bounds
    assert rectangle != changed_plane
    assert rectangle != changed_tolerance
    assert rectangle != object()

    base = symmetric_base(explicit=True)
    circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    equal_circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    changed_center = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.01, 0.0), radius=0.008
    )
    changed_radius = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.007
    )
    changed_source = rectangle_sketch(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    ).extrude_positive_z(depth=0.03)
    changed_binding = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        changed_source.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    assert circle == equal_circle
    assert circle != changed_center
    assert circle != changed_radius
    assert circle != changed_binding
    assert circle != rectangle
    assert circle != object()


@pytest.mark.parametrize(
    ("axis", "index", "value"),
    tuple(itertools.product(("x_bounds", "y_bounds"), (0, 1), NONFINITE)),
)
def test_every_nonfinite_rectangle_coordinate_rejects(
    axis: str, index: int, value: float
) -> None:
    arguments = {
        "x_bounds": [0.0, 2.0],
        "y_bounds": [0.0, 1.0],
        "plane_z": 0.0,
        "modeling_tolerance": 1e-10,
    }
    arguments[axis][index] = value
    arguments["x_bounds"] = tuple(arguments["x_bounds"])
    arguments["y_bounds"] = tuple(arguments["y_bounds"])
    assert_native_validation(
        lambda: eqiora.geometry.CadAuthoredSketch.rectangle_xy(**arguments),
        compatibility_operation=lambda: (
            eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(**arguments, depth=1.0)
        ),
    )


@pytest.mark.parametrize(
    ("axis", "bounds"),
    (
        ("x_bounds", (1.0, 1.0)),
        ("x_bounds", (2.0, 1.0)),
        ("y_bounds", (1.0, 1.0)),
        ("y_bounds", (2.0, 1.0)),
    ),
)
def test_degenerate_and_reversed_rectangle_bounds_reject(
    axis: str, bounds: tuple[float, float]
) -> None:
    arguments = {
        "x_bounds": (0.0, 2.0),
        "y_bounds": (0.0, 1.0),
        "plane_z": 0.0,
        "modeling_tolerance": 1e-10,
    }
    arguments[axis] = bounds
    assert_native_validation(
        lambda: eqiora.geometry.CadAuthoredSketch.rectangle_xy(**arguments),
        compatibility_operation=lambda: (
            eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(**arguments, depth=1.0)
        ),
    )


@pytest.mark.parametrize("plane_z", NONFINITE)
def test_nonfinite_rectangle_plane_rejects(plane_z: float) -> None:
    assert_native_validation(
        lambda: rectangle_sketch(
            x_bounds=(0.0, 2.0),
            y_bounds=(0.0, 1.0),
            plane_z=plane_z,
            modeling_tolerance=1e-10,
        ),
        compatibility_operation=lambda: (
            eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
                x_bounds=(0.0, 2.0),
                y_bounds=(0.0, 1.0),
                plane_z=plane_z,
                depth=1.0,
                modeling_tolerance=1e-10,
            )
        ),
    )


@pytest.mark.parametrize("modeling_tolerance", NONPOSITIVE_OR_NONFINITE)
def test_modeling_tolerance_rejects_at_native_admission(
    modeling_tolerance: float,
) -> None:
    assert_native_validation(
        lambda: rectangle_sketch(modeling_tolerance=modeling_tolerance),
        compatibility_operation=lambda: compatibility_rectangle_graph(
            modeling_tolerance=modeling_tolerance
        ),
    )


@pytest.mark.parametrize("depth", NONPOSITIVE_OR_NONFINITE)
def test_extrusion_depth_rejects_without_changing_the_sketch(depth: float) -> None:
    sketch = rectangle_sketch()
    snapshot = rectangle_sketch()
    assert_native_validation(
        lambda: sketch.extrude_positive_z(depth=depth),
        compatibility_operation=lambda: compatibility_rectangle_graph(depth=depth),
        sketch_pairs=((sketch, snapshot),),
    )


def test_derived_end_plane_overflow_rejects_without_changing_the_sketch() -> None:
    sketch = rectangle_sketch(
        x_bounds=(0.0, 1.0),
        y_bounds=(0.0, 1.0),
        plane_z=sys.float_info.max,
        modeling_tolerance=1e-10,
    )
    snapshot = rectangle_sketch(
        x_bounds=(0.0, 1.0),
        y_bounds=(0.0, 1.0),
        plane_z=sys.float_info.max,
        modeling_tolerance=1e-10,
    )
    assert_native_validation(
        lambda: sketch.extrude_positive_z(depth=sys.float_info.max),
        compatibility_operation=lambda: (
            eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
                x_bounds=(0.0, 1.0),
                y_bounds=(0.0, 1.0),
                plane_z=sys.float_info.max,
                depth=sys.float_info.max,
                modeling_tolerance=1e-10,
            )
        ),
        sketch_pairs=((sketch, snapshot),),
    )


@pytest.mark.parametrize(
    ("index", "value"), tuple(itertools.product((0, 1), NONFINITE))
)
def test_every_nonfinite_circle_coordinate_rejects(index: int, value: float) -> None:
    base = symmetric_base(explicit=True)
    handle = base.face_handle("end-cap")
    center = [0.02, 0.0]
    center[index] = value
    assert_native_validation(
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            handle, center=tuple(center), radius=0.008
        ),
        compatibility_operation=lambda: base.circular_through_cut(
            center=tuple(center), radius=0.008, boolean_tolerance=1e-9
        ),
        graphs=(base,),
        handles=(handle,),
    )


@pytest.mark.parametrize("radius", NONPOSITIVE_OR_NONFINITE)
def test_circle_radius_rejects_without_changing_graph_or_handle(radius: float) -> None:
    base = symmetric_base(explicit=True)
    handle = base.face_handle("end-cap")
    assert_native_validation(
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            handle, center=(0.02, 0.0), radius=radius
        ),
        compatibility_operation=lambda: base.circular_through_cut(
            center=(0.02, 0.0), radius=radius, boolean_tolerance=1e-9
        ),
        graphs=(base,),
        handles=(handle,),
    )


@pytest.mark.parametrize("boolean_tolerance", NONPOSITIVE_OR_NONFINITE)
def test_boolean_tolerance_rejects_without_changing_inputs(
    boolean_tolerance: float,
) -> None:
    base = symmetric_base(explicit=True)
    circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    circle_snapshot = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    assert_native_validation(
        lambda: base.through_cut(circle, boolean_tolerance=boolean_tolerance),
        compatibility_operation=lambda: base.circular_through_cut(
            center=(0.02, 0.0),
            radius=0.008,
            boolean_tolerance=boolean_tolerance,
        ),
        graphs=(base,),
        sketch_pairs=((circle, circle_snapshot),),
    )


@pytest.mark.parametrize(
    "face",
    (
        "start-cap",
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    ),
)
def test_start_cap_and_every_lateral_face_reject_circle_binding(face: str) -> None:
    base = symmetric_base(explicit=True)
    handle = base.face_handle(face)
    assert_native_validation(
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            handle, center=(0.02, 0.0), radius=0.008
        ),
        expected_message=WRONG_FACE_MESSAGE,
        graphs=(base,),
        handles=(handle,),
    )


def test_v2_end_cap_rejects_circle_binding() -> None:
    cut = symmetric_routes()[0]
    handle = cut.face_handle("end-cap")
    assert_native_validation(
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            handle, center=(0.02, 0.0), radius=0.008
        ),
        expected_message=WRONG_FACE_MESSAGE,
        graphs=(cut,),
        handles=(handle,),
    )


@pytest.mark.parametrize(
    "target",
    (
        pytest.param("foreign", id="foreign-rectangle"),
        pytest.param("depth", id="changed-depth"),
        pytest.param("tolerance", id="changed-modeling-tolerance"),
    ),
)
def test_foreign_and_stale_end_cap_sketches_reject_at_the_target(target: str) -> None:
    source = symmetric_base(explicit=True)
    circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        source.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    circle_snapshot = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        source.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    if target == "foreign":
        destination = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
            x_bounds=(-0.05, 0.05),
            y_bounds=(-0.025, 0.025),
            plane_z=0.0,
            depth=0.02,
            modeling_tolerance=1e-10,
        )
    elif target == "depth":
        destination = rectangle_sketch(
            x_bounds=(-0.04, 0.04),
            y_bounds=(-0.025, 0.025),
            plane_z=0.0,
            modeling_tolerance=1e-10,
        ).extrude_positive_z(depth=0.03)
    else:
        destination = rectangle_sketch(
            x_bounds=(-0.04, 0.04),
            y_bounds=(-0.025, 0.025),
            plane_z=0.0,
            modeling_tolerance=2e-10,
        ).extrude_positive_z(depth=0.02)

    assert_native_validation(
        lambda: destination.through_cut(circle, boolean_tolerance=1e-9),
        expected_message=FOREIGN_GRAPH_MESSAGE,
        graphs=(source, destination),
        sketch_pairs=((circle, circle_snapshot),),
    )


@pytest.mark.parametrize(
    "center",
    (
        (0.10, 0.0),
        (0.0335, 0.0),
        (-0.0335, 0.0),
        (0.0, 0.018),
        (0.0, -0.018),
    ),
)
def test_outside_and_asymmetric_near_boundary_circles_reject(
    center: tuple[float, float],
) -> None:
    base = symmetric_base(explicit=True)
    circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=center, radius=0.008
    )
    snapshot = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=center, radius=0.008
    )
    assert_native_validation(
        lambda: base.through_cut(circle, boolean_tolerance=1e-9),
        compatibility_operation=lambda: base.circular_through_cut(
            center=center, radius=0.008, boolean_tolerance=1e-9
        ),
        graphs=(base,),
        sketch_pairs=((circle, snapshot),),
    )


def test_exact_signed_clearance_equality_rejects() -> None:
    base = rectangle_sketch(
        x_bounds=(0.0, 4e-9),
        y_bounds=(0.0, 4e-9),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    ).extrude_positive_z(depth=1.0)
    circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(2e-9, 2e-9), radius=1e-9
    )
    snapshot = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(2e-9, 2e-9), radius=1e-9
    )
    assert_native_validation(
        lambda: base.through_cut(circle, boolean_tolerance=1e-9),
        compatibility_operation=lambda: base.circular_through_cut(
            center=(2e-9, 2e-9), radius=1e-9, boolean_tolerance=1e-9
        ),
        graphs=(base,),
        sketch_pairs=((circle, snapshot),),
    )


def test_wrong_operation_order_rejects_atomically() -> None:
    base = symmetric_base(explicit=True)
    rectangle = rectangle_sketch(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    )
    rectangle_snapshot = rectangle_sketch(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        modeling_tolerance=1e-10,
    )
    assert_native_validation(
        lambda: base.through_cut(rectangle, boolean_tolerance=1e-9),
        expected_message=RECTANGLE_AS_CUT_MESSAGE,
        graphs=(base,),
        sketch_pairs=((rectangle, rectangle_snapshot),),
    )

    circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    circle_snapshot = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
    )
    assert_native_validation(
        lambda: circle.extrude_positive_z(depth=0.02),
        expected_message=CIRCLE_AS_EXTRUSION_MESSAGE,
        graphs=(base,),
        sketch_pairs=((circle, circle_snapshot),),
    )

    once_cut = base.through_cut(circle, boolean_tolerance=1e-9)
    assert_native_validation(
        lambda: once_cut.through_cut(circle, boolean_tolerance=1e-9),
        expected_message=SECOND_CUT_MESSAGE,
        graphs=(base, once_cut),
        sketch_pairs=((circle, circle_snapshot),),
    )


def test_python_conversion_failures_do_not_fabricate_native_diagnostics() -> None:
    base = symmetric_base(explicit=True)
    base_state = (base.canonical_bytes, base.graph_digest)
    operations = (
        lambda: eqiora.geometry.CadAuthoredSketch.rectangle_xy(
            x_bounds=(0.0,),
            y_bounds=(0.0, 1.0),
            plane_z=0.0,
            modeling_tolerance=1e-10,
        ),
        lambda: eqiora.geometry.CadAuthoredSketch.rectangle_xy(
            x_bounds=("not-a-number", 1.0),
            y_bounds=(0.0, 1.0),
            plane_z=0.0,
            modeling_tolerance=1e-10,
        ),
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            object(), center=(0.02, 0.0), radius=0.008
        ),
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            base.face_handle("end-cap"), center=(0.02,), radius=0.008
        ),
        lambda: eqiora.geometry.CadAuthoredSketch.circle_on_face(
            base.face_handle("end-cap"), center=(object(), 0.0), radius=0.008
        ),
        lambda: base.through_cut(object(), boolean_tolerance=1e-9),
    )
    for operation in operations:
        with pytest.raises(TypeError) as caught:
            operation()
        assert not hasattr(caught.value, "diagnostics")
        assert (base.canonical_bytes, base.graph_digest) == base_state


def test_retained_inline_dropped_and_replayed_wrappers_preserve_identity() -> None:
    retained_base = symmetric_base(explicit=True)
    retained_handle = retained_base.face_handle("end-cap")
    retained_circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        retained_handle, center=(0.02, 0.0), radius=0.008
    )
    retained = retained_base.through_cut(retained_circle, boolean_tolerance=1e-9)

    inline_base = symmetric_base(explicit=True)
    inline = inline_base.through_cut(
        eqiora.geometry.CadAuthoredSketch.circle_on_face(
            inline_base.face_handle("end-cap"),
            center=(0.02, 0.0),
            radius=0.008,
        ),
        boolean_tolerance=1e-9,
    )

    source = symmetric_base(explicit=True)
    replayed_target = eqiora.geometry.CadAuthoredGraph.decode_canonical(
        source.canonical_bytes
    )
    handle = source.face_handle("end-cap")
    detached_circle = eqiora.geometry.CadAuthoredSketch.circle_on_face(
        handle, center=(0.02, 0.0), radius=0.008
    )
    del handle
    del source
    gc.collect()
    detached = replayed_target.through_cut(detached_circle, boolean_tolerance=1e-9)

    assert retained == inline == detached
    assert (
        retained.canonical_bytes == inline.canonical_bytes == detached.canonical_bytes
    )
    assert retained.canonical_bytes == V2_WIRE
    assert (
        retained.graph_digest
        == inline.graph_digest
        == detached.graph_digest
        == V2_DIGEST
    )


def stub_class(module: ast.Module, name: str) -> ast.ClassDef:
    return next(
        node
        for node in module.body
        if isinstance(node, ast.ClassDef) and node.name == name
    )


def stub_method(class_node: ast.ClassDef, name: str) -> ast.FunctionDef:
    return next(
        node
        for node in class_node.body
        if isinstance(node, ast.FunctionDef) and node.name == name
    )


def assert_stub_signature(
    method: ast.FunctionDef,
    *,
    positional: tuple[tuple[str, str | None], ...] = (),
    positional_only: tuple[tuple[str, str | None], ...] = (),
    keyword_only: tuple[tuple[str, str], ...] = (),
    returns: str,
) -> None:
    def annotated(arguments: list[ast.arg]) -> tuple[tuple[str, str | None], ...]:
        return tuple(
            (
                argument.arg,
                ast.unparse(argument.annotation)
                if argument.annotation is not None
                else None,
            )
            for argument in arguments
        )

    assert annotated(method.args.args) == positional
    assert annotated(method.args.posonlyargs) == positional_only
    assert annotated(method.args.kwonlyargs) == keyword_only
    assert method.args.vararg is None
    assert method.args.kwarg is None
    assert method.args.defaults == []
    assert method.args.kw_defaults == [None] * len(keyword_only)
    assert ast.unparse(method.returns) == returns


def test_runtime_stub_inventory_signatures_and_exports_are_exact() -> None:
    sketch_type = eqiora.geometry.CadAuthoredSketch
    assert type(sketch_type).__module__ == "builtins"
    assert sketch_type.__module__ == "eqiora._eqiora"
    assert {name for name in sketch_type.__dict__ if not name.startswith("_")} == {
        "rectangle_xy",
        "circle_on_face",
        "extrude_positive_z",
    }
    assert "__eq__" in sketch_type.__dict__
    for constructor_name in ("rectangle_xy", "circle_on_face"):
        assert isinstance(
            inspect.getattr_static(sketch_type, constructor_name), staticmethod
        )
    assert str(inspect.signature(sketch_type.rectangle_xy)) == (
        "(*, x_bounds, y_bounds, plane_z, modeling_tolerance)"
    )
    assert str(inspect.signature(sketch_type.circle_on_face)) == (
        "(face, /, *, center, radius)"
    )
    assert str(inspect.signature(sketch_type.extrude_positive_z)) == (
        "(self, /, *, depth)"
    )
    assert str(inspect.signature(sketch_type.__eq__)) == "(self, value, /)"
    assert sketch_type.__eq__.__text_signature__ == "($self, value, /)"
    assert str(inspect.signature(eqiora.geometry.CadAuthoredGraph.through_cut)) == (
        "(self, sketch, /, *, boolean_tolerance)"
    )
    assert (
        str(inspect.signature(eqiora.geometry.CadAuthoredGraph.rectangle_extrusion))
        == "(*, x_bounds, y_bounds, plane_z, depth, modeling_tolerance)"
    )
    assert (
        str(inspect.signature(eqiora.geometry.CadAuthoredGraph.circular_through_cut))
        == "(self, /, *, center, radius, boolean_tolerance)"
    )

    stub_path = Path(eqiora.geometry.__file__).with_suffix(".pyi")
    module = ast.parse(stub_path.read_text(encoding="utf-8"))
    sketch_stub = stub_class(module, "CadAuthoredSketch")
    assert [ast.unparse(decorator) for decorator in sketch_stub.decorator_list] == [
        "final"
    ]
    assert {
        node.name for node in sketch_stub.body if isinstance(node, ast.FunctionDef)
    } == {"rectangle_xy", "circle_on_face", "extrude_positive_z", "__eq__"}
    assert [
        ast.unparse(decorator)
        for decorator in stub_method(sketch_stub, "rectangle_xy").decorator_list
    ] == ["staticmethod"]
    assert [
        ast.unparse(decorator)
        for decorator in stub_method(sketch_stub, "circle_on_face").decorator_list
    ] == ["staticmethod"]
    assert_stub_signature(
        stub_method(sketch_stub, "rectangle_xy"),
        keyword_only=(
            ("x_bounds", "tuple[float, float]"),
            ("y_bounds", "tuple[float, float]"),
            ("plane_z", "float"),
            ("modeling_tolerance", "float"),
        ),
        returns="CadAuthoredSketch",
    )
    assert_stub_signature(
        stub_method(sketch_stub, "circle_on_face"),
        positional_only=(("face", "CadAuthoredFaceHandle"),),
        keyword_only=(
            ("center", "tuple[float, float]"),
            ("radius", "float"),
        ),
        returns="CadAuthoredSketch",
    )
    assert_stub_signature(
        stub_method(sketch_stub, "extrude_positive_z"),
        positional=(("self", None),),
        keyword_only=(("depth", "float"),),
        returns="CadAuthoredGraph",
    )
    assert_stub_signature(
        stub_method(sketch_stub, "__eq__"),
        positional_only=(("self", None), ("other", "object")),
        returns="bool",
    )

    graph_stub = stub_class(module, "CadAuthoredGraph")
    assert_stub_signature(
        stub_method(graph_stub, "rectangle_extrusion"),
        keyword_only=(
            ("x_bounds", "tuple[float, float]"),
            ("y_bounds", "tuple[float, float]"),
            ("plane_z", "float"),
            ("depth", "float"),
            ("modeling_tolerance", "float"),
        ),
        returns="CadAuthoredGraph",
    )
    assert_stub_signature(
        stub_method(graph_stub, "circular_through_cut"),
        positional=(("self", None),),
        keyword_only=(
            ("center", "tuple[float, float]"),
            ("radius", "float"),
            ("boolean_tolerance", "float"),
        ),
        returns="CadAuthoredGraph",
    )
    assert_stub_signature(
        stub_method(graph_stub, "through_cut"),
        positional_only=(
            ("self", None),
            ("sketch", "CadAuthoredSketch"),
        ),
        keyword_only=(("boolean_tolerance", "float"),),
        returns="CadAuthoredGraph",
    )

    stub_all = next(
        ast.literal_eval(node.value)
        for node in module.body
        if isinstance(node, ast.Assign)
        and any(
            isinstance(target, ast.Name) and target.id == "__all__"
            for target in node.targets
        )
    )
    assert list(eqiora.geometry.__all__) == stub_all == EXPECTED_GEOMETRY_ALL
    assert stub_all == sorted(stub_all)

    forbidden_types = ("Sketch", "Plane", "Profile", "Feature", "Boolean")
    assert all(not hasattr(eqiora.geometry, name) for name in forbidden_types)
    assert all(name not in stub_all for name in forbidden_types)
    assert not hasattr(eqiora, "CadAuthoredSketch")
    assert not hasattr(sketch_type, "xy")
    assert not hasattr(sketch_type, "on_face")
    assert not hasattr(sketch_type, "extrude")
    assert not hasattr(eqiora.geometry.CadAuthoredGraph, "cut")
    with pytest.raises(TypeError):
        sketch_type()


def test_explicit_composition_runs_from_the_isolated_installed_interpreter(
    tmp_path: Path,
) -> None:
    program = """
import eqiora

base_sketch = eqiora.geometry.CadAuthoredSketch.rectangle_xy(
    x_bounds=(-0.04, 0.04),
    y_bounds=(-0.025, 0.025),
    plane_z=0.0,
    modeling_tolerance=1e-10,
)
base = base_sketch.extrude_positive_z(depth=0.02)
cut_sketch = eqiora.geometry.CadAuthoredSketch.circle_on_face(
    base.face_handle("end-cap"),
    center=(0.02, 0.0),
    radius=0.008,
)
graph = base.through_cut(cut_sketch, boolean_tolerance=1e-9)
print(type(base_sketch).__module__ + "." + type(base_sketch).__name__)
print(graph.graph_digest)
print(len(graph.canonical_bytes))
print(graph == eqiora.geometry.CadAuthoredGraph.decode_canonical(graph.canonical_bytes))
"""
    completed = subprocess.run(
        [sys.executable, "-I", "-c", program],
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert completed.stderr == ""
    assert completed.stdout.splitlines() == [
        "eqiora._eqiora.CadAuthoredSketch",
        V2_DIGEST,
        "1292",
        "True",
    ]
