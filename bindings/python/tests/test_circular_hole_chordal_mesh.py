"""Installed-Python contract for one exact-source-bound chordal reference mesh."""

from __future__ import annotations

import hashlib
import json
import math
import subprocess
import sys
import gc
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest
import numpy as np

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "exact_cylinder_mesh.py"

GEOMETRY_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
MESH_SCHEMA = b"eqiora.simplicial-mesh-envelope/v1"
MESH_CANONICAL_BYTES = 4_835
MESH_CANONICAL_RAW_SHA256 = (
    "d977d9125488fffee72deaf9a0f146bc42dc05a135692919a374d746da0f1079"
)
MESH_DIGEST = "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a"

REQUESTED_MAX_BOUNDARY_ERROR = 1.0e-4
BOUNDARY_EVALUATION_ALLOWANCE = 6.252_776_074_688_882e-14
SAGITTA_50 = 9.866_357_858_642_19e-5
REQUIRED_MINIMUM_MEAN_RATIO = 1.0e-5
MINIMUM_MEAN_RATIO = 0.003_213_006_369_764_433
MINIMUM_SIGNED_MEASURE_SCALE = 0.000_421_024_591_498_332_1

SELECTION_COUNTS = {
    "cylinder": 50,
    "inlet": 14,
    "outlet": 2,
    "walls": 38,
    "fluid": 104,
}
STANDARD_ARGUMENTS: dict[str, Any] = {
    "bounds": ((0.0, 2.2), (0.0, 0.41)),
    "circle_center": (0.2, 0.2),
    "circle_radius": 0.05,
    "tolerance": 1e-12,
    "region": "fluid",
    "x_lower": "inlet",
    "x_upper": "outlet",
    "y_lower": "walls",
    "y_upper": "walls",
    "hole": "cylinder",
}

EXPECTED_PUBLIC_PROGRAM_STDOUT = [
    GEOMETRY_DIGEST,
    MESH_DIGEST,
    "eqiora.reference.chordal-triangle/v1 50",
    "2 104 104",
    "cylinder 50",
    "inlet 14",
    "outlet 2",
    "walls 38",
    "fluid 104",
]
ISOLATED_MESH_PROGRAM = """
import eqiora
import sys

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
mesh = eqiora.meshing.generate(geometry, plan=plan)
print(mesh.source_digest)
print(mesh.digest)
print(plan.provider, plan.boundary_facets)
print(mesh.dimension, mesh.vertex_count, mesh.cell_count)
for selection in mesh.selection_names:
    print(selection, mesh.selection_entity_count(selection))
"""


def geometry(**overrides: object) -> object:
    arguments = STANDARD_ARGUMENTS | overrides
    (x_bounds, y_bounds) = arguments["bounds"]
    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=arguments["circle_center"],
        radius=arguments["circle_radius"],
        boolean_tolerance=1e-10,
    )
    return graph.planar_circular_section(
        classification_tolerance=arguments["tolerance"],
        region=arguments["region"],
        x_lower=arguments["x_lower"],
        x_upper=arguments["x_upper"],
        y_lower=arguments["y_lower"],
        y_upper=arguments["y_upper"],
        hole=arguments["hole"],
    )


def request(
    *,
    maximum_boundary_error: float = REQUESTED_MAX_BOUNDARY_ERROR,
    minimum_mean_ratio: float = REQUIRED_MINIMUM_MEAN_RATIO,
    maximum_boundary_facets: int = 50,
) -> object:
    return eqiora.meshing.MeshRequest(
        maximum_boundary_error=maximum_boundary_error,
        minimum_mean_ratio=minimum_mean_ratio,
        maximum_boundary_facets=maximum_boundary_facets,
    )


def resolve_plan(
    authored: object | None = None,
    *,
    maximum_boundary_error: float = REQUESTED_MAX_BOUNDARY_ERROR,
    minimum_mean_ratio: float = REQUIRED_MINIMUM_MEAN_RATIO,
    maximum_boundary_facets: int = 50,
) -> object:
    source = geometry() if authored is None else authored
    return eqiora.meshing.resolve(
        source,
        request(
            maximum_boundary_error=maximum_boundary_error,
            minimum_mean_ratio=minimum_mean_ratio,
            maximum_boundary_facets=maximum_boundary_facets,
        ),
    )


def realize(
    authored: object | None = None,
    *,
    maximum_boundary_error: float = REQUESTED_MAX_BOUNDARY_ERROR,
    minimum_mean_ratio: float = REQUIRED_MINIMUM_MEAN_RATIO,
    maximum_boundary_facets: int = 50,
) -> object:
    source = geometry() if authored is None else authored
    plan = resolve_plan(
        source,
        maximum_boundary_error=maximum_boundary_error,
        minimum_mean_ratio=minimum_mean_ratio,
        maximum_boundary_facets=maximum_boundary_facets,
    )
    return eqiora.meshing.generate(source, plan=plan)


def assert_structured_validation(operation: Callable[[], object]) -> None:
    with pytest.raises(eqiora.ValidationError) as caught:
        operation()

    error = caught.value
    assert error.category == "validation"
    assert error.diagnostics
    assert all(diagnostic.code.startswith("EQ") for diagnostic in error.diagnostics)
    assert all(diagnostic.severity == "error" for diagnostic in error.diagnostics)
    assert all(diagnostic.message for diagnostic in error.diagnostics)


def test_standard_mesh_replays_the_frozen_rfc_and_inner_artifact() -> None:
    plan = resolve_plan()
    mesh = realize()

    assert type(mesh).__module__ == "eqiora._eqiora"
    assert type(mesh).__name__ == "Mesh"
    assert mesh.source_digest == GEOMETRY_DIGEST
    assert mesh.dimension == 2
    assert mesh.vertex_count == 104
    assert mesh.cell_count == 104
    assert plan.source_digest == GEOMETRY_DIGEST
    assert plan.provider == "eqiora.reference.chordal-triangle/v1"
    assert plan.boundary_facets == 50
    assert mesh.selection_names == tuple(SELECTION_COUNTS)
    realized_counts = {
        name: mesh.selection_entity_count(name) for name in mesh.selection_names
    }
    assert realized_counts == SELECTION_COUNTS
    assert (
        sum(realized_counts[name] for name in ("inlet", "outlet", "walls", "cylinder"))
        == 104
    )

    assert plan.request.maximum_boundary_error == REQUESTED_MAX_BOUNDARY_ERROR
    assert plan.request.minimum_mean_ratio == REQUIRED_MINIMUM_MEAN_RATIO
    assert plan.request.maximum_boundary_facets == 50
    assert plan.boundary_evaluation_allowance == BOUNDARY_EVALUATION_ALLOWANCE
    assert plan.boundary_error_bound <= REQUESTED_MAX_BOUNDARY_ERROR
    assert math.isclose(
        plan.boundary_error_bound - plan.boundary_evaluation_allowance,
        SAGITTA_50,
        rel_tol=0.0,
        abs_tol=BOUNDARY_EVALUATION_ALLOWANCE,
    )
    assert plan.minimum_mean_ratio == MINIMUM_MEAN_RATIO
    assert mesh.minimum_mean_ratio == MINIMUM_MEAN_RATIO

    canonical = mesh.canonical_bytes
    assert isinstance(canonical, bytes)
    assert len(canonical) == MESH_CANONICAL_BYTES
    assert hashlib.sha256(canonical).hexdigest() == MESH_CANONICAL_RAW_SHA256
    framed = MESH_SCHEMA + b"\0" + canonical
    assert hashlib.sha256(framed).hexdigest() == MESH_DIGEST
    assert mesh.digest == MESH_DIGEST

    assert mesh.coordinates is mesh.coordinates
    assert mesh.cells is mesh.cells
    assert mesh.coordinates.shape == (104, 2)
    assert mesh.cells.shape == (104, 3)
    assert mesh.coordinates.dtype == np.float64
    assert mesh.cells.dtype == np.uint32
    assert not mesh.coordinates.flags.writeable
    assert not mesh.cells.flags.writeable

    document = json.loads(canonical)
    assert document["schema"] == MESH_SCHEMA.decode()
    assert document["topology"] == {
        "dimension": 2,
        "cell_family": "simplex",
    }
    assert document["geometry"] == {
        "coordinate_scalar": "f64",
        "mapping": "affine",
    }
    assert len(document["vertices"]) == 104
    assert len(document["cells"]) == 104
    assert document["acceptance"]["minimum_mean_ratio"] == (REQUIRED_MINIMUM_MEAN_RATIO)
    assert document["evidence"] == {
        "minimum_mean_ratio": MINIMUM_MEAN_RATIO,
        "minimum_signed_measure_scale": MINIMUM_SIGNED_MEASURE_SCALE,
    }
    assert "source" not in document
    assert "source_digest" not in document


def test_mesh_wrapper_is_immutable() -> None:
    source = geometry()
    request_value = request()
    plan = eqiora.meshing.resolve(source, request_value)
    mesh = realize()

    with pytest.raises(AttributeError):
        request_value.maximum_boundary_error = 0.1
    with pytest.raises(AttributeError):
        plan.source_digest = "0" * 64
    with pytest.raises(AttributeError):
        mesh.cells = np.empty((0, 3), dtype=np.uint32)
    with pytest.raises(AttributeError):
        mesh.digest = "0" * 64


def test_mesh_arrays_outlive_the_wrapper_and_generations_do_not_share_storage() -> None:
    source = geometry()
    plan = resolve_plan(source)
    first = eqiora.meshing.generate(source, plan=plan)
    second = eqiora.meshing.generate(source, plan=plan)
    coordinates = first.coordinates
    cells = first.cells

    np.testing.assert_array_equal(coordinates, second.coordinates)
    np.testing.assert_array_equal(cells, second.cells)
    assert not np.shares_memory(coordinates, second.coordinates)
    assert not np.shares_memory(cells, second.cells)
    del first
    gc.collect()
    assert coordinates.shape == (104, 2) and not coordinates.flags.writeable
    assert cells.shape == (104, 3) and not cells.flags.writeable


def test_mesh_plan_rejects_a_foreign_exact_geometry() -> None:
    source = geometry()
    foreign = geometry(tolerance=1e-10)
    plan = resolve_plan(source)

    assert plan.source_digest == source.digest != foreign.digest
    assert_structured_validation(
        lambda: eqiora.meshing.generate(foreign, plan=plan)
    )


def test_swapped_side_names_separate_source_identity_from_mesh_identity() -> None:
    standard = realize()
    swapped_geometry = geometry(x_lower="outlet", x_upper="inlet")
    swapped = realize(swapped_geometry)

    assert swapped.source_digest == swapped_geometry.digest
    assert swapped.source_digest != standard.source_digest
    assert swapped.digest == standard.digest
    assert swapped.canonical_bytes == standard.canonical_bytes
    assert swapped.selection_names == standard.selection_names
    assert swapped.selection_entity_count("inlet") == 2
    assert swapped.selection_entity_count("outlet") == 14
    assert swapped.selection_entity_count("walls") == 38
    assert swapped.selection_entity_count("cylinder") == 50
    assert swapped.selection_entity_count("fluid") == 104


def test_mesh_policy_is_distinct_from_geometry_classification_tolerance() -> None:
    fine_classification = realize(geometry(tolerance=1e-12))
    coarse_classification = realize(geometry(tolerance=1e-10))
    fine_plan = resolve_plan(geometry(tolerance=1e-12))
    coarse_plan = resolve_plan(geometry(tolerance=1e-10))

    assert fine_classification.source_digest != coarse_classification.source_digest
    assert fine_classification.digest == coarse_classification.digest
    assert fine_plan.request.maximum_boundary_error == (
        coarse_plan.request.maximum_boundary_error
    )
    assert fine_plan.boundary_facets == coarse_plan.boundary_facets
    assert fine_plan.boundary_error_bound == coarse_plan.boundary_error_bound


def test_insufficient_work_budget_fails_before_a_mesh_is_returned() -> None:
    assert_structured_validation(lambda: realize(maximum_boundary_facets=49))


def test_quality_gate_stricter_than_the_frozen_mesh_fails_closed() -> None:
    assert MINIMUM_MEAN_RATIO < 0.5
    assert_structured_validation(lambda: realize(minimum_mean_ratio=0.5))


def test_unknown_realized_selection_is_structured_validation() -> None:
    mesh = realize()
    assert_structured_validation(lambda: mesh.selection_entity_count("missing"))


def test_meshing_surface_uses_common_boundaries_without_overclaiming() -> None:
    for supported in ("Mesh", "MeshPlan", "MeshRequest", "generate", "resolve"):
        assert hasattr(eqiora.meshing, supported)
    for removed in ("CircularHoleChordalMesh", "circular_hole_chordal"):
        assert not hasattr(eqiora.meshing, removed)
    for unsupported in ("import_gmsh", "Triangular", "Quality"):
        assert not hasattr(eqiora.meshing, unsupported)

    mesh = realize()
    for unsupported in (
        "save",
        "solve",
        "result",
        "trajectory",
        "animate",
        "import_source",
        "source_manifest",
        "realization_envelope",
    ):
        assert not hasattr(mesh, unsupported)


def test_public_mesh_program_runs_in_an_isolated_subprocess(tmp_path: Path) -> None:
    completed = subprocess.run(
        [sys.executable, "-I", "-c", ISOLATED_MESH_PROGRAM],
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )

    assert completed.stderr == ""
    assert completed.stdout.splitlines() == EXPECTED_PUBLIC_PROGRAM_STDOUT


def test_numpy_import_is_lazy_until_mesh_array_projection(tmp_path: Path) -> None:
    program = ISOLATED_MESH_PROGRAM.replace(
        "print(mesh.source_digest)",
        'assert "numpy" not in sys.modules\n_ = mesh.coordinates\n'
        'assert "numpy" in sys.modules\nprint(mesh.source_digest)',
    )
    completed = subprocess.run(
        [sys.executable, "-I", "-c", program],
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert completed.stderr == ""
    assert completed.stdout.splitlines() == EXPECTED_PUBLIC_PROGRAM_STDOUT


def test_checked_in_python_demo_runs_from_installed_package() -> None:
    if not PYTHON_DEMO.is_file():
        pytest.skip("consumer tree does not carry the checked-in Python example")

    completed = subprocess.run(
        [sys.executable, "-I", str(PYTHON_DEMO)],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )

    assert completed.stderr == ""
    assert completed.stdout.splitlines() == EXPECTED_PUBLIC_PROGRAM_STDOUT
