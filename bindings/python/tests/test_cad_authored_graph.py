"""Installed-package transparency for the closed authored-CAD graph."""

from __future__ import annotations

import math
from pathlib import Path
import subprocess
import sys

import pytest

import eqiora


# Frozen by geometry.cad-authored-rectangle-extrusion; this adapter does not
# own or reinterpret the canonical wire or digest.
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

# Frozen by geometry.cad-authored-circular-through-cut.
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
RELATIVE_TOLERANCE = 4e-15
DISTINCT_Y_SECTION_DIGEST = (
    "51ece8fa2d8709d932b0c758d59c187e4fd572f73217c31dcbe407f8d873be7f"
)


def rectangle(*, tolerance: float = 1e-9, depth: float = 4.0):
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(-2.0, 3.0),
        y_bounds=(-1.0, 2.0),
        plane_z=0.5,
        depth=depth,
        modeling_tolerance=tolerance,
    )


def cut(*, boolean_tolerance: float = 1e-9):
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        depth=0.02,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=(0.02, 0.0),
        radius=0.008,
        boolean_tolerance=boolean_tolerance,
    )


def dfg_cut(
    *, plane_z: float = 0.0, depth: float = 1.0, modeling_tolerance: float = 1e-10
):
    return eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=plane_z,
        depth=depth,
        modeling_tolerance=modeling_tolerance,
    ).circular_through_cut(
        center=(0.2, 0.2),
        radius=0.05,
        boolean_tolerance=1e-10,
    )


def dfg_section(graph, *, y_lower: str = "walls", y_upper: str = "walls"):
    lower = graph.face_handle("profile-y-lower")
    upper = graph.face_handle("profile-y-upper")
    side_topology = (
        {y_lower: (lower, upper)}
        if y_lower == y_upper
        else {y_lower: lower, y_upper: upper}
    )
    return graph.planar_section(
        named_topology={
            "fluid": graph.face_handle("end-cap"),
            "inlet": graph.face_handle("profile-x-lower"),
            "outlet": graph.face_handle("profile-x-upper"),
            **side_topology,
            "cylinder": graph.face_handle("cut-wall"),
        }
    )


def keys(handles: tuple[object, ...]) -> tuple[str, ...]:
    return tuple(handle.provenance_key for handle in handles)


def test_both_frozen_histories_replay_native_identity_and_geometry_evidence() -> None:
    v1 = rectangle()
    assert type(v1).__module__ == "eqiora._eqiora"
    assert len(V1_WIRE) == 731
    assert v1.canonical_bytes == V1_WIRE
    assert v1.graph_digest == V1_DIGEST
    assert eqiora.geometry.CadAuthoredGraph.decode_canonical(V1_WIRE) == v1
    assert v1.bounds == ((-2.0, 3.0), (-1.0, 2.0), (0.5, 4.5))
    assert (v1.vertex_count, v1.edge_count, v1.face_count) == (8, 12, 6)
    assert (v1.body_count, v1.closed_shell_count, v1.genus) == (1, 1, 0)
    assert (v1.volume, v1.surface_area, v1.repair) == (60.0, 94.0, "none")

    v2 = cut()
    assert len(V2_WIRE) == 1292
    assert v2.canonical_bytes == V2_WIRE
    assert v2.graph_digest == V2_DIGEST
    assert eqiora.geometry.CadAuthoredGraph.decode_canonical(V2_WIRE) == v2
    assert v2.bounds == ((-0.04, 0.04), (-0.025, 0.025), (0.0, 0.02))
    assert (v2.vertex_count, v2.edge_count, v2.face_count) == (None, None, 7)
    assert (v2.body_count, v2.closed_shell_count, v2.genus) == (1, 1, 1)
    assert math.isclose(v2.volume, 7.597876140340507e-5, rel_tol=RELATIVE_TOLERANCE)
    assert math.isclose(
        v2.surface_area, 0.01380318578948924, rel_tol=RELATIVE_TOLERANCE
    )


def test_face_handles_keep_exact_provenance_and_observations_native() -> None:
    graph = cut()
    expected = {
        "start-cap": (0.0037989380701702533, 2),
        "end-cap": (0.0037989380701702533, 2),
        "profile-x-lower": (0.001, 1),
        "profile-x-upper": (0.001, 1),
        "profile-y-lower": (0.0016, 1),
        "profile-y-upper": (0.0016, 1),
        "cut-wall": (0.001005309649148734, 2),
    }
    assert graph.selection_names == tuple(expected)

    for name, (area, loops) in expected.items():
        handle = graph.face_handle(name)
        assert handle.graph_digest == graph.graph_digest
        assert handle.provenance_key == name
        replayed = eqiora.geometry.CadAuthoredFaceHandle.decode_canonical(
            handle.canonical_bytes
        )
        assert replayed == handle
        assert graph.resolve_face(replayed) == name
        assert math.isclose(graph.face_area(replayed), area, rel_tol=RELATIVE_TOLERANCE)
        assert graph.face_boundary_loop_count(replayed) == loops

    wall = graph.face_handle("cut-wall")
    assert graph.rectangular_face_vertices(wall) is None
    assert graph.rectangular_face_centroid(wall) is None
    assert graph.planar_face_outward_normal(wall) is None


def test_rectangle_face_projection_preserves_the_complete_frozen_oracle() -> None:
    graph = rectangle()
    expected = {
        "start-cap": (
            (0.5, 0.5, 0.5),
            15.0,
            (0.0, 0.0, -1.0),
            (
                (-2.0, -1.0, 0.5),
                (-2.0, 2.0, 0.5),
                (3.0, 2.0, 0.5),
                (3.0, -1.0, 0.5),
            ),
        ),
        "end-cap": (
            (0.5, 0.5, 4.5),
            15.0,
            (0.0, 0.0, 1.0),
            (
                (-2.0, -1.0, 4.5),
                (3.0, -1.0, 4.5),
                (3.0, 2.0, 4.5),
                (-2.0, 2.0, 4.5),
            ),
        ),
        "profile-x-lower": (
            (-2.0, 0.5, 2.5),
            12.0,
            (-1.0, 0.0, 0.0),
            (
                (-2.0, -1.0, 0.5),
                (-2.0, -1.0, 4.5),
                (-2.0, 2.0, 4.5),
                (-2.0, 2.0, 0.5),
            ),
        ),
        "profile-x-upper": (
            (3.0, 0.5, 2.5),
            12.0,
            (1.0, 0.0, 0.0),
            (
                (3.0, -1.0, 0.5),
                (3.0, 2.0, 0.5),
                (3.0, 2.0, 4.5),
                (3.0, -1.0, 4.5),
            ),
        ),
        "profile-y-lower": (
            (0.5, -1.0, 2.5),
            20.0,
            (0.0, -1.0, 0.0),
            (
                (-2.0, -1.0, 0.5),
                (3.0, -1.0, 0.5),
                (3.0, -1.0, 4.5),
                (-2.0, -1.0, 4.5),
            ),
        ),
        "profile-y-upper": (
            (0.5, 2.0, 2.5),
            20.0,
            (0.0, 1.0, 0.0),
            (
                (-2.0, 2.0, 0.5),
                (-2.0, 2.0, 4.5),
                (3.0, 2.0, 4.5),
                (3.0, 2.0, 0.5),
            ),
        ),
    }
    assert graph.selection_names == tuple(expected)
    for name, (centroid, area, normal, vertices) in expected.items():
        handle = graph.face_handle(name)
        assert graph.face_boundary_loop_count(handle) == 1
        assert graph.rectangular_face_centroid(handle) == centroid
        assert graph.face_area(handle) == area
        assert graph.planar_face_outward_normal(handle) == normal
        assert graph.rectangular_face_vertices(handle) == vertices


def test_complete_build_receipt_does_not_conflate_identity_or_tolerance() -> None:
    graph = cut()
    build = graph.build()
    assert build.graph_digest == graph.graph_digest
    assert build.provider_profile == "eqiora.cad.analytic-circular-through-cut-v1"
    assert build.requested_modeling_tolerance == 1e-10
    assert build.requested_boolean_tolerance == 1e-9
    assert build.effective_boolean_tolerance == 1e-9
    assert (
        build.maximum_position_discrepancy,
        build.maximum_area_discrepancy,
        build.maximum_volume_discrepancy,
    ) == (0.0, 0.0, 0.0)
    assert build.repair == "none"
    assert keys(build.retained_unchanged) == (
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    )
    assert keys(build.retained_modified) == ("start-cap", "end-cap")
    assert keys(build.created) == ("cut-wall",)
    assert build.deleted == build.split == build.merged == ()

    discriminator = cut(boolean_tolerance=1e-11).build()
    assert discriminator.requested_modeling_tolerance == 1e-10
    assert discriminator.requested_boolean_tolerance == 1e-11
    assert discriminator.effective_boolean_tolerance == 1e-11


def test_tolerance_only_change_separates_graph_identity_from_geometry_evidence() -> None:
    first = rectangle(tolerance=1e-9)
    changed = rectangle(tolerance=2e-9)

    assert first != changed
    assert first.graph_digest != changed.graph_digest
    assert first.bounds == changed.bounds
    assert first.volume == changed.volume
    assert first.surface_area == changed.surface_area
    assert not hasattr(first, "geometry_digest")
    assert not hasattr(first.build(), "geometry_digest")

    with pytest.raises(eqiora.ValidationError):
        changed.resolve_face(first.face_handle("profile-x-lower"))


def test_wire_and_handle_mutants_fail_closed_at_the_native_owner() -> None:
    graph = cut()
    for mutant in (
        V2_WIRE.replace(b"envelope/v2", b"envelope/v3"),
        V2_WIRE.replace(b'"length_unit":', b'"unknown":0,"length_unit":'),
        V2_WIRE.replace(
            b'"length_unit":', b'"length_unit":"metre","length_unit":'
        ),
        V2_WIRE.replace(b"difference-through-all-negative-z", b"blind-cut"),
        b"x" * 4097,
    ):
        with pytest.raises(eqiora.ValidationError):
            eqiora.geometry.CadAuthoredGraph.decode_canonical(mutant)

    old_handle = rectangle().face_handle("end-cap")
    with pytest.raises(eqiora.ValidationError):
        graph.resolve_face(old_handle)

    handle = graph.face_handle("cut-wall")
    foreign_wire = handle.canonical_bytes.replace(
        V2_DIGEST.encode(), ("1" + V2_DIGEST[1:]).encode(), 1
    )
    foreign = eqiora.geometry.CadAuthoredFaceHandle.decode_canonical(foreign_wire)
    with pytest.raises(eqiora.ValidationError):
        graph.resolve_face(foreign)
    with pytest.raises(eqiora.ValidationError):
        eqiora.geometry.CadAuthoredFaceHandle.decode_canonical(b"x" * 513)
    with pytest.raises(eqiora.ValidationError):
        graph.face_handle("unknown-face")


@pytest.mark.parametrize(
    "arguments",
    [
        {"x_bounds": (0.0, 0.0)},
        {"y_bounds": (1.0, 0.0)},
        {"plane_z": float("nan")},
        {"depth": 0.0},
        {"depth": float("inf")},
        {"modeling_tolerance": 0.0},
    ],
)
def test_invalid_rectangle_inputs_are_structured_validation(arguments: dict[str, object]) -> None:
    complete = {
        "x_bounds": (-2.0, 3.0),
        "y_bounds": (-1.0, 2.0),
        "plane_z": 0.5,
        "depth": 4.0,
        "modeling_tolerance": 1e-9,
    }
    with pytest.raises(eqiora.ValidationError) as caught:
        eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(**(complete | arguments))
    assert caught.value.category == "validation"
    assert caught.value.diagnostics


@pytest.mark.parametrize(
    "arguments",
    [
        {"center": (float("nan"), 0.0)},
        {"radius": 0.0},
        {"radius": float("inf")},
        {"boolean_tolerance": 0.0},
    ],
)
def test_invalid_cut_inputs_are_structured_validation(arguments: dict[str, object]) -> None:
    complete = {
        "center": (0.02, 0.0),
        "radius": 0.008,
        "boolean_tolerance": 1e-9,
    }
    cut_graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(-0.04, 0.04),
        y_bounds=(-0.025, 0.025),
        plane_z=0.0,
        depth=0.02,
        modeling_tolerance=1e-10,
    )
    with pytest.raises(eqiora.ValidationError) as caught:
        cut_graph.circular_through_cut(**(complete | arguments))
    assert caught.value.category == "validation"
    assert caught.value.diagnostics


def test_planar_section_preserves_distinct_y_roles_and_ignores_nonplanar_facts() -> None:
    oriented = dfg_section(dfg_cut(), y_lower="floor", y_upper="ceiling")
    assert oriented.digest == DISTINCT_Y_SECTION_DIGEST
    assert oriented.selection_names == (
        "ceiling",
        "cylinder",
        "floor",
        "inlet",
        "outlet",
        "fluid",
    )

    same_section = dfg_section(
        dfg_cut(plane_z=4.0, depth=3.0, modeling_tolerance=2e-10),
        y_lower="floor",
        y_upper="ceiling",
    )
    assert same_section.canonical_bytes == oriented.canonical_bytes
    assert same_section.digest == oriented.digest

    with pytest.raises(eqiora.ValidationError):
        rectangle().planar_section(named_topology={})


def test_atomic_topology_mapping_uses_build_lineage_without_classification_input() -> None:
    graph = dfg_cut()
    named_topology = {
        "fluid": graph.face_handle("end-cap"),
        "inlet": graph.face_handle("profile-x-lower"),
        "outlet": graph.face_handle("profile-x-upper"),
        "walls": (
            graph.face_handle("profile-y-lower"),
            graph.face_handle("profile-y-upper"),
        ),
        "cylinder": graph.face_handle("cut-wall"),
    }
    section = graph.planar_section(named_topology=named_topology)
    predecessor = dfg_section(graph)
    assert section.canonical_bytes == predecessor.canonical_bytes
    assert section.digest == predecessor.digest

    arbitrary = dict(named_topology)
    arbitrary["left boundary"] = arbitrary.pop("inlet")
    renamed = graph.planar_section(named_topology=arbitrary)
    assert "left boundary" in renamed.selection_names
    assert "inlet" not in renamed.selection_names

    with pytest.raises(TypeError):
        graph.planar_section(named_topology=[])
    with pytest.raises(eqiora.ValidationError):
        graph.planar_section(
            named_topology={
                key: value for key, value in named_topology.items() if key != "outlet"
            }
        )
    with pytest.raises(eqiora.ValidationError):
        graph.planar_section(
            named_topology={
                **named_topology,
                "fluid": graph.face_handle("start-cap"),
            }
        )


def test_runtime_surface_and_installed_stub_name_the_same_bounded_api() -> None:
    expected = {
        eqiora.geometry.CadAuthoredFaceHandle: {
            "decode_canonical",
            "canonical_bytes",
            "graph_digest",
            "provenance_key",
        },
        eqiora.geometry.CadAuthoredBuild: {
            "graph_digest",
            "provider_profile",
            "requested_modeling_tolerance",
            "requested_boolean_tolerance",
            "effective_boolean_tolerance",
            "maximum_position_discrepancy",
            "maximum_area_discrepancy",
            "maximum_volume_discrepancy",
            "repair",
            "retained_unchanged",
            "retained_modified",
            "created",
            "deleted",
            "split",
            "merged",
        },
        eqiora.geometry.CadAuthoredGraph: {
            "rectangle_extrusion",
            "decode_canonical",
            "circular_through_cut",
            "through_cut",
            "planar_section",
            "canonical_bytes",
            "graph_digest",
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
            "face_handle",
            "resolve_face",
            "face_area",
            "face_boundary_loop_count",
            "rectangular_face_vertices",
            "rectangular_face_centroid",
            "planar_face_outward_normal",
            "build",
        },
    }
    stub = Path(eqiora.geometry.__file__).with_suffix(".pyi").read_text(encoding="utf-8")
    for cls, names in expected.items():
        runtime_names = {name for name in cls.__dict__ if not name.startswith("_")}
        class_stub = stub.split(f"class {cls.__name__}:", 1)[1].split("\n@final", 1)[0]
        stub_names = {
            line.split("def ", 1)[1].split("(", 1)[0]
            for line in class_stub.splitlines()
            if line.lstrip().startswith("def ")
            and not line.lstrip().startswith("def __")
        }
        assert runtime_names == names == stub_names


def test_values_and_collections_are_immutable_and_hashable() -> None:
    graph = rectangle()
    same = rectangle()
    handle = graph.face_handle("start-cap")
    vertices = graph.rectangular_face_vertices(handle)
    assert graph == same
    assert hash(graph) == hash(same)
    assert isinstance(vertices, tuple)
    assert all(isinstance(vertex, tuple) for vertex in vertices)
    assert hash(handle) == hash(
        eqiora.geometry.CadAuthoredFaceHandle.decode_canonical(handle.canonical_bytes)
    )
    with pytest.raises(AttributeError):
        graph.graph_digest = "0" * 64
    with pytest.raises(TypeError):
        graph.selection_names[0] = "renamed"


def test_public_graph_runs_from_the_isolated_installed_interpreter(tmp_path) -> None:
    program = """
import eqiora
graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(-0.04, 0.04), y_bounds=(-0.025, 0.025), plane_z=0.0,
    depth=0.02, modeling_tolerance=1e-10,
).circular_through_cut(
    center=(0.02, 0.0), radius=0.008, boolean_tolerance=1e-9,
)
print(graph.graph_digest)
print(len(graph.canonical_bytes))
print(*graph.selection_names)
print(graph.build().provider_profile)
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
        V2_DIGEST,
        "1292",
        "start-cap end-cap profile-x-lower profile-x-upper profile-y-lower profile-y-upper cut-wall",
        "eqiora.cad.analytic-circular-through-cut-v1",
    ]
