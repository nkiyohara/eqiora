"""Installed-Python contract for one exact-source-bound chordal reference mesh."""

from __future__ import annotations

import hashlib
import json
import math
import subprocess
import sys
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

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
AREA_DEFICIT_50 = 2.065_453_620_546_776e-5
AREA_ALLOWANCE = 1.964_367_538_078_461_7e-14
PERIMETER_DEFICIT_50 = 2.066_677_124_124_434_7e-4
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
    "2 104 104 50",
    "cylinder 50",
    "inlet 14",
    "outlet 2",
    "walls 38",
    "fluid 104",
]
ISOLATED_MESH_PROGRAM = """
import eqiora

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
mesh = eqiora.meshing.circular_hole_chordal(
    geometry,
    max_boundary_error=1e-4,
    required_minimum_mean_ratio=1e-5,
    max_segments=50,
)
print(mesh.source_digest)
print(mesh.mesh_digest)
print(
    mesh.dimension,
    mesh.vertex_count,
    mesh.cell_count,
    mesh.circle_segments,
)
for selection in mesh.selection_names:
    print(selection, mesh.selection_entity_count(selection))
"""


def geometry(**overrides: object) -> object:
    return eqiora.geometry.RectangleWithCircularHole(**(STANDARD_ARGUMENTS | overrides))


def realize(
    authored: object | None = None,
    *,
    max_boundary_error: float = REQUESTED_MAX_BOUNDARY_ERROR,
    required_minimum_mean_ratio: float = REQUIRED_MINIMUM_MEAN_RATIO,
    max_segments: int = 50,
) -> object:
    return eqiora.meshing.circular_hole_chordal(
        geometry() if authored is None else authored,
        max_boundary_error=max_boundary_error,
        required_minimum_mean_ratio=required_minimum_mean_ratio,
        max_segments=max_segments,
    )


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
    mesh = realize()

    assert type(mesh).__module__ == "eqiora._eqiora"
    assert type(mesh).__name__ == "CircularHoleChordalMesh"
    assert mesh.source_digest == GEOMETRY_DIGEST
    assert mesh.dimension == 2
    assert mesh.vertex_count == 104
    assert mesh.cell_count == 104
    assert mesh.circle_segments == 50
    assert mesh.selection_names == tuple(SELECTION_COUNTS)
    realized_counts = {
        name: mesh.selection_entity_count(name) for name in mesh.selection_names
    }
    assert realized_counts == SELECTION_COUNTS
    assert (
        sum(realized_counts[name] for name in ("inlet", "outlet", "walls", "cylinder"))
        == 104
    )

    assert mesh.requested_max_boundary_error == REQUESTED_MAX_BOUNDARY_ERROR
    assert mesh.boundary_evaluation_allowance == BOUNDARY_EVALUATION_ALLOWANCE
    assert mesh.boundary_error_bound <= REQUESTED_MAX_BOUNDARY_ERROR
    assert math.isclose(
        mesh.boundary_error_bound - mesh.boundary_evaluation_allowance,
        SAGITTA_50,
        rel_tol=0.0,
        abs_tol=BOUNDARY_EVALUATION_ALLOWANCE,
    )
    assert math.isclose(
        mesh.circle_area_deficit,
        AREA_DEFICIT_50,
        rel_tol=0.0,
        abs_tol=AREA_ALLOWANCE,
    )
    assert math.isclose(
        mesh.circle_perimeter_deficit,
        PERIMETER_DEFICIT_50,
        rel_tol=0.0,
        abs_tol=BOUNDARY_EVALUATION_ALLOWANCE,
    )
    assert mesh.required_minimum_mean_ratio == REQUIRED_MINIMUM_MEAN_RATIO
    assert mesh.minimum_mean_ratio == MINIMUM_MEAN_RATIO

    canonical = mesh.mesh_canonical_json
    assert isinstance(canonical, bytes)
    assert len(canonical) == MESH_CANONICAL_BYTES
    assert hashlib.sha256(canonical).hexdigest() == MESH_CANONICAL_RAW_SHA256
    framed = MESH_SCHEMA + b"\0" + canonical
    assert hashlib.sha256(framed).hexdigest() == MESH_DIGEST
    assert mesh.mesh_digest == MESH_DIGEST

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
    mesh = realize()

    with pytest.raises(AttributeError):
        mesh.circle_segments = 49
    with pytest.raises(AttributeError):
        mesh.mesh_digest = "0" * 64


def test_swapped_side_names_separate_source_identity_from_mesh_identity() -> None:
    standard = realize()
    swapped_geometry = geometry(x_lower="outlet", x_upper="inlet")
    swapped = realize(swapped_geometry)

    assert swapped.source_digest == swapped_geometry.digest
    assert swapped.source_digest != standard.source_digest
    assert swapped.mesh_digest == standard.mesh_digest
    assert swapped.mesh_canonical_json == standard.mesh_canonical_json
    assert swapped.selection_names == standard.selection_names
    assert swapped.selection_entity_count("inlet") == 2
    assert swapped.selection_entity_count("outlet") == 14
    assert swapped.selection_entity_count("walls") == 38
    assert swapped.selection_entity_count("cylinder") == 50
    assert swapped.selection_entity_count("fluid") == 104


def test_mesh_policy_is_distinct_from_geometry_classification_tolerance() -> None:
    fine_classification = realize(geometry(tolerance=1e-12))
    coarse_classification = realize(geometry(tolerance=1e-10))

    assert fine_classification.source_digest != coarse_classification.source_digest
    assert fine_classification.mesh_digest == coarse_classification.mesh_digest
    assert fine_classification.requested_max_boundary_error == (
        coarse_classification.requested_max_boundary_error
    )
    assert fine_classification.circle_segments == coarse_classification.circle_segments
    assert fine_classification.boundary_error_bound == (
        coarse_classification.boundary_error_bound
    )


def test_insufficient_work_budget_fails_before_a_mesh_is_returned() -> None:
    assert_structured_validation(lambda: realize(max_segments=49))


def test_quality_gate_stricter_than_the_frozen_mesh_fails_closed() -> None:
    assert MINIMUM_MEAN_RATIO < 0.5
    assert_structured_validation(lambda: realize(required_minimum_mean_ratio=0.5))


def test_unknown_realized_selection_is_structured_validation() -> None:
    mesh = realize()
    assert_structured_validation(lambda: mesh.selection_entity_count("missing"))


def test_bounded_meshing_surface_does_not_claim_generic_mesh_workflows() -> None:
    for unsupported in (
        "Mesh",
        "MeshRequest",
        "generate",
        "import_gmsh",
        "Triangular",
        "Quality",
    ):
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
