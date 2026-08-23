"""Installed-Python contract for one exact-source-bound Gmsh chordal mesh."""

from __future__ import annotations

import gc
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
from collections.abc import Callable
from collections import Counter
from pathlib import Path
from typing import Any

import numpy as np
import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "exact_cylinder_mesh.py"

GEOMETRY_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
MESH_SCHEMA = b"eqiora.simplicial-mesh-envelope/v1"
GMSH_VERSION = "4.15.2"
GMSH_PROVIDER = f"eqiora.gmsh-cli/{GMSH_VERSION}"
OLD_REFERENCE_MESH_DIGEST = (
    "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a"
)
MESH_CANONICAL_BYTES = 42_388
MESH_CANONICAL_RAW_SHA256 = (
    "9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2"
)
MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
COORDINATE_BUFFER_SHA256 = (
    "42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d"
)
TRIANGLE_BUFFER_SHA256 = (
    "05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642"
)

REQUESTED_MAX_BOUNDARY_ERROR = 1.0e-4
BOUNDARY_EVALUATION_ALLOWANCE = 6.252_776_074_688_882e-14
SAGITTA_50 = 9.866_357_858_642_19e-5
REQUIRED_MINIMUM_MEAN_RATIO = 1.0e-5
REJECTED_MINIMUM_MEAN_RATIO = 0.75
MINIMUM_MEAN_RATIO = 0.523_652_268_685_533_6
MINIMUM_SIGNED_MEASURE_SCALE = 2.609_303_845_007_427_3e-5
VERTEX_COUNT = 662
CELL_COUNT = 1_210
BOUNDARY_EDGE_COUNT = 114

SELECTION_COUNTS = {
    "cylinder": 50,
    "inlet": 14,
    "outlet": 2,
    "walls": 48,
    "fluid": CELL_COUNT,
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
    f"{GMSH_PROVIDER} 50",
    f"2 {VERTEX_COUNT} {CELL_COUNT}",
    "cylinder 50",
    "inlet 14",
    "outlet 2",
    "walls 48",
    f"fluid {CELL_COUNT}",
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


def configured_gmsh() -> Path:
    explicit = os.environ.get("EQIORA_GMSH")
    discovered = explicit if explicit is not None else shutil.which("gmsh")
    assert discovered is not None, (
        "this evidence requires Gmsh 4.15.2 through EQIORA_GMSH or PATH"
    )
    executable = Path(discovered).resolve()
    completed = subprocess.run(
        [str(executable), "--version"],
        check=True,
        capture_output=True,
        text=True,
        timeout=10,
    )
    assert completed.stderr == ""
    assert completed.stdout.strip() == GMSH_VERSION
    return executable


def write_python_executable(path: Path, body: str) -> Path:
    path.write_text(f"#!{sys.executable}\n{body}", encoding="utf-8")
    path.chmod(0o755)
    return path


def path_with_gmsh(directory: Path, executable: Path) -> str:
    directory.mkdir()
    (directory / "gmsh").symlink_to(executable)
    inherited = os.environ.get("PATH", "")
    return os.pathsep.join((str(directory), inherited))


def affine_quality_observations(mesh: object) -> tuple[float, float]:
    minimum_mean_ratio = math.inf
    minimum_signed_measure_scale = math.inf
    for cell in mesh.cells:
        a, b, c = (mesh.coordinates[int(index)] for index in cell)
        j00 = float(b[0] - a[0])
        j01 = float(c[0] - a[0])
        j10 = float(b[1] - a[1])
        j11 = float(c[1] - a[1])
        signed_measure_scale = j00 * j11 - j01 * j10
        frobenius_squared = sum(
            value * value for value in (j00, j01, j10, j11)
        )
        mean_ratio = (
            2.0 * math.pow(abs(signed_measure_scale), 1.0) / frobenius_squared
        )
        assert signed_measure_scale > 0.0
        minimum_mean_ratio = min(minimum_mean_ratio, mean_ratio)
        minimum_signed_measure_scale = min(
            minimum_signed_measure_scale, signed_measure_scale
        )
    return minimum_mean_ratio, minimum_signed_measure_scale


def geometric_boundary_counts(mesh: object) -> Counter[str]:
    incidence: Counter[tuple[int, int]] = Counter()
    for cell in mesh.cells:
        a, b, c = (int(index) for index in cell)
        incidence.update(
            tuple(sorted(edge)) for edge in ((a, b), (b, c), (c, a))
        )
    boundary = [edge for edge, count in incidence.items() if count == 1]
    assert len(boundary) == BOUNDARY_EDGE_COUNT

    counts: Counter[str] = Counter()
    for edge in boundary:
        points = [mesh.coordinates[index] for index in edge]
        matches = {
            "inlet": all(abs(float(point[0])) <= 1e-12 for point in points),
            "outlet": all(
                abs(float(point[0]) - 2.2) <= 1e-12 for point in points
            ),
            "walls": all(
                abs(float(point[1])) <= 1e-12
                or abs(float(point[1]) - 0.41) <= 1e-12
                for point in points
            ),
            "cylinder": all(
                abs(
                    math.hypot(
                        float(point[0]) - 0.2, float(point[1]) - 0.2
                    )
                    - 0.05
                )
                <= 1e-12
                for point in points
            ),
        }
        names = [name for name, matched in matches.items() if matched]
        assert len(names) == 1
        counts[names[0]] += 1
    return counts


def test_gmsh_mesh_replays_the_frozen_public_artifact() -> None:
    configured_gmsh()
    source = geometry()
    plan = resolve_plan(source)
    mesh = eqiora.meshing.generate(source, plan=plan)

    assert type(mesh).__module__ == "eqiora._eqiora"
    assert type(mesh).__name__ == "Mesh"
    assert mesh.source_digest == GEOMETRY_DIGEST
    assert mesh.dimension == 2
    assert mesh.vertex_count == VERTEX_COUNT
    assert mesh.cell_count == CELL_COUNT
    assert plan.source_digest == GEOMETRY_DIGEST
    assert plan.provider == GMSH_PROVIDER
    assert plan.boundary_facets == 50
    assert mesh.selection_names == tuple(SELECTION_COUNTS)
    realized_counts = {
        name: mesh.selection_entity_count(name) for name in mesh.selection_names
    }
    assert realized_counts == SELECTION_COUNTS
    assert mesh.selection_entity_count(name="cylinder") == SELECTION_COUNTS["cylinder"]
    assert (
        sum(realized_counts[name] for name in ("inlet", "outlet", "walls", "cylinder"))
        == BOUNDARY_EDGE_COUNT
    )
    assert geometric_boundary_counts(mesh) == Counter(
        {name: count for name, count in SELECTION_COUNTS.items() if name != "fluid"}
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
    assert plan.achieved_minimum_mean_ratio == MINIMUM_MEAN_RATIO
    assert mesh.minimum_mean_ratio == MINIMUM_MEAN_RATIO
    assert mesh.minimum_mean_ratio >= plan.request.minimum_mean_ratio
    assert affine_quality_observations(mesh) == (
        MINIMUM_MEAN_RATIO,
        MINIMUM_SIGNED_MEASURE_SCALE,
    )

    canonical = mesh.canonical_bytes
    assert isinstance(canonical, bytes)
    assert len(canonical) == MESH_CANONICAL_BYTES
    assert hashlib.sha256(canonical).hexdigest() == MESH_CANONICAL_RAW_SHA256
    framed = MESH_SCHEMA + b"\0" + canonical
    assert hashlib.sha256(framed).hexdigest() == MESH_DIGEST
    assert mesh.digest == MESH_DIGEST
    assert mesh.digest != OLD_REFERENCE_MESH_DIGEST

    assert mesh.coordinates is mesh.coordinates
    assert mesh.cells is mesh.cells
    assert mesh.coordinates.shape == (VERTEX_COUNT, 2)
    assert mesh.cells.shape == (CELL_COUNT, 3)
    assert mesh.coordinates.dtype == np.float64
    assert mesh.cells.dtype == np.uint32
    assert hashlib.sha256(mesh.coordinates.tobytes()).hexdigest() == (
        COORDINATE_BUFFER_SHA256
    )
    assert hashlib.sha256(mesh.cells.tobytes()).hexdigest() == TRIANGLE_BUFFER_SHA256
    for view in (mesh.coordinates, mesh.cells):
        assert view.flags.c_contiguous and view.flags.aligned
        assert not view.flags.owndata
        assert not view.flags.writeable
        with pytest.raises(ValueError):
            view.setflags(write=True)
        with pytest.raises(ValueError):
            view.flat[0] = 0

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
    assert len(document["vertices"]) == VERTEX_COUNT
    assert len(document["cells"]) == CELL_COUNT
    assert document["acceptance"]["minimum_mean_ratio"] == (REQUIRED_MINIMUM_MEAN_RATIO)
    assert document["evidence"] == {
        "minimum_mean_ratio": MINIMUM_MEAN_RATIO,
        "minimum_signed_measure_scale": MINIMUM_SIGNED_MEASURE_SCALE,
    }
    assert "source" not in document
    assert "source_digest" not in document


def test_plan_publishes_the_exact_mesh_it_inspected(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    log = tmp_path / "calls.log"
    blocked = tmp_path / "blocked"
    wrapper = write_python_executable(
        tmp_path / "gmsh-wrapper",
        f"""import os
import sys
from pathlib import Path

log = Path({str(log)!r})
with log.open("a", encoding="utf-8") as stream:
    stream.write(repr(sys.argv[1:]) + "\\n")
if Path({str(blocked)!r}).exists():
    raise SystemExit(86)
real = {str(real_gmsh)!r}
os.execv(real, [real, *sys.argv[1:]])
""",
    )
    monkeypatch.setenv("EQIORA_GMSH", str(wrapper))

    source = geometry()
    plan = resolve_plan(source)
    calls_after_resolve = log.read_text(encoding="utf-8")
    assert calls_after_resolve
    blocked.write_text("generation must not launch Gmsh again", encoding="utf-8")

    first = eqiora.meshing.generate(source, plan=plan)
    second = eqiora.meshing.generate(source, plan=plan)
    assert log.read_text(encoding="utf-8") == calls_after_resolve
    assert first.digest == second.digest == MESH_DIGEST
    assert first.canonical_bytes == second.canonical_bytes
    np.testing.assert_array_equal(first.coordinates, second.coordinates)
    np.testing.assert_array_equal(first.cells, second.cells)


def test_explicit_gmsh_path_precedes_path_lookup(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    wrong = write_python_executable(
        tmp_path / "wrong-gmsh",
        'print("4.15.1")\n',
    )
    monkeypatch.setenv("PATH", path_with_gmsh(tmp_path / "bin", wrong))
    monkeypatch.setenv("EQIORA_GMSH", str(real_gmsh))

    assert resolve_plan().provider == GMSH_PROVIDER


def test_gmsh_path_lookup_is_used_without_explicit_override(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    monkeypatch.setenv("PATH", path_with_gmsh(tmp_path / "bin", real_gmsh))
    monkeypatch.delenv("EQIORA_GMSH", raising=False)

    assert resolve_plan().provider == GMSH_PROVIDER


def test_missing_gmsh_is_structured_validation_without_reference_fallback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.delenv("EQIORA_GMSH", raising=False)
    monkeypatch.setenv("PATH", str(tmp_path))

    assert_structured_validation(resolve_plan)


def test_gmsh_launch_failure_is_structured_validation_without_fallback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    monkeypatch.setenv("PATH", path_with_gmsh(tmp_path / "bin", real_gmsh))
    broken = tmp_path / "broken-gmsh"
    broken.write_text("#!/definitely/missing/interpreter\n", encoding="utf-8")
    broken.chmod(0o755)
    monkeypatch.setenv("EQIORA_GMSH", str(broken))

    assert_structured_validation(resolve_plan)


def test_gmsh_nonzero_exit_is_structured_validation_without_fallback(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    monkeypatch.setenv("PATH", path_with_gmsh(tmp_path / "bin", real_gmsh))
    failing = write_python_executable(
        tmp_path / "failing-gmsh",
        f"""import sys
if any("version" in argument.lower() for argument in sys.argv[1:]):
    print({GMSH_VERSION!r})
    raise SystemExit(0)
raise SystemExit(23)
""",
    )
    monkeypatch.setenv("EQIORA_GMSH", str(failing))

    assert_structured_validation(resolve_plan)


@pytest.mark.parametrize(
    "payload",
    (
        b"not a Gmsh mesh\n",
        b"$MeshFormat\n2.2 0 8\n$EndMeshFormat\n",
    ),
    ids=("malformed", "unsupported-msh-version"),
)
def test_gmsh_invalid_output_is_structured_validation_without_fallback(
    payload: bytes, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    monkeypatch.setenv("PATH", path_with_gmsh(tmp_path / "bin", real_gmsh))
    invalid = write_python_executable(
        tmp_path / "invalid-output-gmsh",
        f"""import sys
from pathlib import Path
if any("version" in argument.lower() for argument in sys.argv[1:]):
    print({GMSH_VERSION!r})
    raise SystemExit(0)
args = sys.argv[1:]
if "-o" in args and args.index("-o") + 1 < len(args):
    Path(args[args.index("-o") + 1]).write_bytes({payload!r})
else:
    sys.stdout.buffer.write({payload!r})
""",
    )
    monkeypatch.setenv("EQIORA_GMSH", str(invalid))

    assert_structured_validation(resolve_plan)


@pytest.mark.parametrize(
    "reported_version",
    ("4.15.1", "4.15.3", "4.15.2-git", "4.15.2\nunexpected"),
)
def test_every_nonexact_gmsh_version_is_structured_validation_without_fallback(
    reported_version: str, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_gmsh = configured_gmsh()
    monkeypatch.setenv("PATH", path_with_gmsh(tmp_path / "bin", real_gmsh))
    wrong = write_python_executable(
        tmp_path / "wrong-version-gmsh",
        f"import sys\nsys.stdout.write({reported_version!r} + '\\n')\n",
    )
    monkeypatch.setenv("EQIORA_GMSH", str(wrong))

    assert_structured_validation(resolve_plan)


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
    assert coordinates.shape == (VERTEX_COUNT, 2) and not coordinates.flags.writeable
    assert cells.shape == (CELL_COUNT, 3) and not cells.flags.writeable


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
    assert swapped.selection_entity_count("walls") == 48
    assert swapped.selection_entity_count("cylinder") == 50
    assert swapped.selection_entity_count("fluid") == CELL_COUNT


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
    assert MINIMUM_MEAN_RATIO < REJECTED_MINIMUM_MEAN_RATIO
    assert_structured_validation(
        lambda: realize(minimum_mean_ratio=REJECTED_MINIMUM_MEAN_RATIO)
    )


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
