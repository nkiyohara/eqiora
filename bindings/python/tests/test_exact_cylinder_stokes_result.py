"""Installed-wheel contract for one exact-cylinder steady Stokes Plan and Run."""

from __future__ import annotations

import asyncio
import gc
import hashlib
import importlib.resources
import json
import math
import subprocess
import sys
from pathlib import Path
from typing import Any

import numpy as np
import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "exact_cylinder_stokes.py"
PACKAGED_MODEL = (
    importlib.resources.files("eqiora")
    .joinpath("examples")
    .joinpath("steady-flow-past-cylinder.model.json")
)

SOURCE_DIGEST = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9"
MESH_DIGEST = "148e2fb4f3d5c801eaa4e3a376f0b8ec547abdcfebc1108cf0577e5c952a946a"
MODEL_DIGEST = "8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146"
MODEL_RESOURCE_BYTES = 16_798
MODEL_RESOURCE_SHA256 = (
    "5c5c7924d6efe624a4b4df5f03f2fab03e423fc2ebafb658ba8ad050a7496387"
)
SEMANTIC_REVISION = 1
REALIZATION_REVISION = 133

LENGTH_SCALE_M = 0.41
VELOCITY_SCALE_M_PER_S = 0.3
PRESSURE_SCALE_PA = 0.001 * 0.3 / 0.41
RELATIVE_TOLERANCE = 1.0e-6
ABSOLUTE_TOLERANCE = 1.0e-13
MAXIMUM_ITERATIONS = 10_000
SPATIAL_DIMENSION = 2
VELOCITY_SPACE = "simplex-p1-bubble"
PRESSURE_SPACE = "continuous-lagrange-1"
SOLVER_ALGORITHM = "sparse-lu"
PRECONDITIONER = "identity"
REDUCTION = "fast"
SOLVER_BACKEND = "eqiora.faer"
WORKERS = 1

INTENT_ARGUMENTS: dict[str, Any] = {
    "length_scale_m": LENGTH_SCALE_M,
    "velocity_scale_m_per_s": VELOCITY_SCALE_M_PER_S,
    "pressure_scale_pa": PRESSURE_SCALE_PA,
    "relative_tolerance": RELATIVE_TOLERANCE,
    "absolute_tolerance": ABSOLUTE_TOLERANCE,
    "maximum_iterations": MAXIMUM_ITERATIONS,
}
PLAN_PROPERTIES = (
    "model_digest",
    "semantic_revision",
    "geometry_digest",
    "correspondence_digest",
    "mesh_digest",
    "realization_digest",
    "realization_revision",
    "spatial_dimension",
    "velocity_space",
    "pressure_space",
    "length_scale_m",
    "velocity_scale_m_per_s",
    "pressure_scale_pa",
    "solver_algorithm",
    "preconditioner",
    "reduction",
    "relative_tolerance",
    "absolute_tolerance",
    "maximum_iterations",
    "solver_backend",
    "execution_adapter",
    "workers",
    "canonical_bytes",
)
REMOVED_ENTRY_POINT = "solve_exact_cylinder_stokes"

PRESSURE_DIMENSION = (1, -1, -2, 0, 0, 0, 0)
PRESSURE_TOLERANCE = 2.0e-14 + 5.0e-7 * (0.001 * 0.3 / 0.41)
FLUX_TOLERANCE = 2.0e-13 + 5.0e-7 * (0.3 * 0.41)
REACTION_TOLERANCE = 2.0e-14 + 5.0e-7 * (0.001 * 0.3)
RESIDUAL_TARGET = 1.323_962_765_120_967_3e-7

PRESSURE_PROBES = (
    ((0.15000000000000002, 0.2), 20.611897142913634),
    ((0.25, 0.2), 0.111521650853062),
    ((0.19686047402353435, 0.15009866357858642), 11.03786740720071),
    ((0.19686047402353435, 0.2499013364214136), 10.315730130178096),
    ((0.0, 0.20000000000000004), 19.780390332641403),
    ((2.2, 0.2), -0.04836168726748482),
)
EXPECTED_INLET_FLUX = -0.08149573099927537
EXPECTED_OUTLET_FLUX = 0.08149573099927537
EXPECTED_CYLINDER_REACTION = (-4.617062540501679, 0.03952008400301018)
EXPECTED_GLOBAL_REACTION = (-5.345112862320582e-41, 1.140769053837547e-40)
EXPECTED_ZERO_FORCE = (0.0, 0.0)

BINDING_FIELDS = (
    "schema",
    "encoding",
    "source_geometry_sha256",
    "realized_geometry_sha256",
    "mesh_sha256",
    "correspondence_sha256",
    "requested_max_boundary_error_m",
    "boundary_evaluation_allowance_m",
    "boundary_error_bound_m",
    "circle_segments",
    "circle_area_deficit_m2",
    "circle_perimeter_deficit_m",
    "required_minimum_mean_ratio",
)
RUN_FIELDS = (
    "schema",
    "encoding",
    "model_sha256",
    "semantic_revision",
    "realization_sha256",
    "execution",
    "output_sha256",
)


def model_bytes() -> bytes:
    encoded = PACKAGED_MODEL.read_bytes()
    assert len(encoded) == MODEL_RESOURCE_BYTES
    assert hashlib.sha256(encoded).hexdigest() == MODEL_RESOURCE_SHA256
    assert encoded.endswith(b"\n")
    return encoded


def semantic_model_digest(encoded: bytes) -> str:
    document = json.loads(encoded)
    content = {
        key: document[key]
        for key in (
            "schema",
            "encoding",
            "model_ulid",
            "nodes",
            "values",
            "edges",
            "boundary",
        )
    }
    canonical = json.dumps(content, separators=(",", ":"), ensure_ascii=False).encode()
    return hashlib.sha256(b"eqiora.model-envelope/v8\0" + canonical).hexdigest()


def model_result_ids(encoded: bytes) -> tuple[str, str]:
    document = json.loads(encoded)
    nodes = document["nodes"]
    fields = {
        node["id"]["ulid"]: node["definition"]
        for node in nodes
        if node["id"]["kind"] == "field"
    }
    pressure_fields = set()
    for node in nodes:
        definition = node["definition"]
        if definition["kind"] != "relation":
            continue
        expression_nodes = definition["residuals"]["nodes"]
        for expression in expression_nodes:
            if expression["op"] != "isotropic-lift":
                continue
            source = expression_nodes[expression["value"]]
            if source["op"] != "symbol" or source["symbol"]["kind"] != "field":
                continue
            identifier = source["symbol"]["id"]["ulid"]
            dimension = fields[identifier]["dimension"]
            if (
                tuple(
                    dimension[key]
                    for key in (
                        "mass",
                        "length",
                        "time",
                        "current",
                        "temperature",
                        "amount",
                        "luminous_intensity",
                    )
                )
                == PRESSURE_DIMENSION
            ):
                pressure_fields.add(identifier)
    support_domains = {
        node["id"]["ulid"]
        for node in nodes
        if node["id"]["kind"] == "domain"
        and node["definition"]
        == {
            "kind": "domain",
            "domain": {
                "kind": "geometry-region",
                "geometry": SOURCE_DIGEST,
                "entity_set": "fluid",
            },
        }
    }
    assert len(pressure_fields) == len(support_domains) == 1
    return pressure_fields.pop(), support_domains.pop()


def geometry(**overrides: object) -> Any:
    arguments: dict[str, object] = {
        "bounds": ((0.0, 2.2), (0.0, 0.41)),
        "circle_center": (0.2, 0.2),
        "circle_radius": 0.05,
        "tolerance": 1.0e-12,
        "region": "fluid",
        "x_lower": "inlet",
        "x_upper": "outlet",
        "y_lower": "walls",
        "y_upper": "walls",
        "hole": "cylinder",
    }
    arguments.update(overrides)
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


def mesh_plan(
    source: Any,
    *,
    maximum_boundary_error: float = 1.0e-4,
    minimum_mean_ratio: float = 1.0e-5,
    maximum_boundary_facets: int = 50,
) -> Any:
    request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=maximum_boundary_error,
        minimum_mean_ratio=minimum_mean_ratio,
        maximum_boundary_facets=maximum_boundary_facets,
    )
    return eqiora.meshing.resolve(source, request)


def mesh(
    source: Any,
    *,
    maximum_boundary_error: float = 1.0e-4,
    minimum_mean_ratio: float = 1.0e-5,
    maximum_boundary_facets: int = 50,
) -> Any:
    plan = mesh_plan(
        source,
        maximum_boundary_error=maximum_boundary_error,
        minimum_mean_ratio=minimum_mean_ratio,
        maximum_boundary_facets=maximum_boundary_facets,
    )
    return eqiora.meshing.generate(source, plan=plan)


def intent(**overrides: object) -> Any:
    arguments: dict[str, Any] = dict(INTENT_ARGUMENTS)
    arguments.update(overrides)
    return eqiora.fluid.SteadyStokes(**arguments)


def replayed(model: bytes | None = None) -> Any:
    return eqiora.replay(model_bytes() if model is None else model)


def resolve_plan(
    realized: Any, *, model: bytes | None = None, **overrides: object
) -> Any:
    return eqiora.fluid.resolve(replayed(model), intent(**overrides), mesh=realized)


def solve(realized: Any, *, model: bytes | None = None) -> Any:
    current = replayed(model)
    resolved = eqiora.fluid.resolve(current, intent(), mesh=realized)
    return eqiora.submit(current, plan=resolved).result()


@pytest.fixture(scope="module")
def accepted() -> tuple[Any, Any, Any]:
    source = geometry()
    realized = mesh(source)
    return source, realized, solve(realized)


def assert_digest(value: str) -> None:
    assert len(value) == 64
    assert all(character in "0123456789abcdef" for character in value)


def assert_vector_close(
    actual: tuple[float, float],
    expected: tuple[float, float],
    tolerance: float,
) -> None:
    assert all(
        math.isfinite(observed) and abs(observed - reference) <= tolerance
        for observed, reference in zip(actual, expected, strict=True)
    )


def assert_error(
    operation: Any,
    exception: type[eqiora.EqioraError],
    *,
    category: str,
    code: str,
) -> None:
    with pytest.raises(exception) as caught:
        operation()
    error = caught.value
    assert error.category == category
    assert error.diagnostics
    assert any(diagnostic.code == code for diagnostic in error.diagnostics)
    assert all(diagnostic.severity == "error" for diagnostic in error.diagnostics)


def test_model_oracle_is_independent_of_source_revision_provenance() -> None:
    encoded = model_bytes()
    assert semantic_model_digest(encoded) == MODEL_DIGEST

    changed_revision = json.loads(encoded)
    changed_revision["source_revision"] = 2
    changed = json.dumps(changed_revision, separators=(",", ":")).encode()
    assert semantic_model_digest(changed) == MODEL_DIGEST


def test_complete_result_replays_binding_run_and_frozen_observations(
    accepted: tuple[Any, Any, Any],
) -> None:
    source, realized, result = accepted
    assert type(result).__module__ == "eqiora._eqiora"
    assert type(result).__name__ == "CircularHoleSteadyStokesResult"
    assert isinstance(result, eqiora.fluid.CircularHoleSteadyStokesResult)

    assert result.model_digest == MODEL_DIGEST
    assert result.semantic_revision == SEMANTIC_REVISION
    assert result.realization_revision == REALIZATION_REVISION
    assert result.exact_source_digest == source.digest == SOURCE_DIGEST
    assert realized.source_digest == SOURCE_DIGEST
    assert result.mesh_digest == realized.digest == MESH_DIGEST
    assert result.correspondence_digest == realized.correspondence_digest
    assert result.chordal_realization_digest == realized.realization_digest
    assert result.pressure_dimension == PRESSURE_DIMENSION
    pressure_field_id, support_domain_id = model_result_ids(model_bytes())
    assert result.pressure_field_id == pressure_field_id
    assert result.support_domain_id == support_domain_id
    assert result.bounds == ((0.0, 2.2), (0.0, 0.41))
    plan = mesh_plan(source)
    assert result.requested_max_boundary_error == 1.0e-4
    assert result.boundary_evaluation_allowance == plan.boundary_evaluation_allowance
    assert result.boundary_error_bound == plan.boundary_error_bound
    assert result.circle_segments == plan.boundary_facets == 50

    identities = (
        result.model_digest,
        result.chordal_realization_digest,
        result.exact_source_digest,
        result.realized_geometry_digest,
        result.correspondence_digest,
        result.realization_digest,
        result.run_digest,
        result.snapshot_digest,
        result.mesh_digest,
    )
    for identity in identities:
        assert_digest(identity)
    assert len(set(identities)) == len(identities)

    binding_bytes = result.chordal_realization_json
    assert isinstance(binding_bytes, bytes)
    binding = json.loads(binding_bytes)
    assert tuple(binding) == BINDING_FIELDS
    assert binding["schema"] == ("eqiora.circular-hole-chordal-realization-envelope/v1")
    assert binding["source_geometry_sha256"] == result.exact_source_digest
    assert binding["realized_geometry_sha256"] == result.realized_geometry_digest
    assert binding["mesh_sha256"] == result.mesh_digest
    assert binding["correspondence_sha256"] == result.correspondence_digest
    assert binding["requested_max_boundary_error_m"] == (
        result.requested_max_boundary_error
    )
    assert binding["boundary_evaluation_allowance_m"] == (
        result.boundary_evaluation_allowance
    )
    assert binding["boundary_error_bound_m"] == result.boundary_error_bound
    assert binding["circle_segments"] == result.circle_segments
    assert binding["required_minimum_mean_ratio"] == 1.0e-5
    assert binding["circle_area_deficit_m2"] > 0.0
    assert binding["circle_perimeter_deficit_m"] > 0.0
    assert (
        hashlib.sha256(binding["schema"].encode() + b"\0" + binding_bytes).hexdigest()
        == result.chordal_realization_digest
    )

    run_bytes = result.run_manifest_json
    assert isinstance(run_bytes, bytes)
    run = json.loads(run_bytes)
    assert tuple(run) == RUN_FIELDS
    assert run["schema"] == "eqiora.run-manifest/v2"
    assert run["model_sha256"] == result.model_digest
    assert run["semantic_revision"] == result.semantic_revision
    assert run["realization_sha256"] == result.realization_digest
    assert run["output_sha256"] == [result.snapshot_digest]
    assert run["execution"]["solver_backend"] == result.solve.backend == "eqiora.faer"
    assert run["execution"]["adapter"] == result.solve.adapter
    assert run["execution"]["reduction"] == result.solve.reduction == "fast"
    assert run["execution"]["topology"] == {"kind": "host", "workers": 1}
    assert run["execution"]["libraries"]["faer"] == "0.24.4"
    assert (
        hashlib.sha256(run["schema"].encode() + b"\0" + run_bytes).hexdigest()
        == result.run_digest
    )

    mesh_document = json.loads(realized.canonical_bytes)
    coordinates = result.coordinates
    triangles = result.triangles
    pressure = result.pressure.numpy(copy=False)
    np.testing.assert_array_equal(
        coordinates, np.asarray(mesh_document["vertices"], dtype=np.float64)
    )
    np.testing.assert_array_equal(
        triangles, np.asarray(mesh_document["cells"], dtype=np.uint32)
    )
    assert pressure.shape == (104,)
    assert np.isfinite(pressure).all()
    assert float(pressure.min()) == result.pressure_minimum
    assert float(pressure.max()) == result.pressure_maximum
    assert len(result.pressure) == len(pressure)
    assert result.pressure[-1] == pressure[-1]
    with pytest.raises(IndexError):
        result.pressure[-sys.maxsize]
    with pytest.raises(IndexError):
        result.pressure[len(result.pressure)]
    with pytest.raises(IndexError):
        result.pressure[sys.maxsize]

    for position, expected in PRESSURE_PROBES:
        squared_distance = np.square(
            coordinates - np.asarray(position, dtype=np.float64)
        ).sum(axis=1)
        index = int(np.argmin(squared_distance))
        assert squared_distance[index] <= result.boundary_evaluation_allowance**2
        assert result.pressure[index] == pressure[index]
        assert abs(pressure[index] - expected) <= PRESSURE_TOLERANCE

    assert abs(result.inlet_flux - EXPECTED_INLET_FLUX) <= FLUX_TOLERANCE
    assert abs(result.outlet_flux - EXPECTED_OUTLET_FLUX) <= FLUX_TOLERANCE
    assert result.net_flux == result.inlet_flux + result.outlet_flux
    assert abs(result.net_flux) <= 1.0e-8
    assert_vector_close(
        result.cylinder_force_on_fluid,
        EXPECTED_CYLINDER_REACTION,
        REACTION_TOLERANCE,
    )
    assert_vector_close(
        result.constrained_reaction,
        EXPECTED_GLOBAL_REACTION,
        REACTION_TOLERANCE,
    )
    assert_vector_close(
        result.integrated_body_force,
        EXPECTED_ZERO_FORCE,
        REACTION_TOLERANCE,
    )
    assert_vector_close(
        result.integrated_boundary_traction,
        EXPECTED_ZERO_FORCE,
        REACTION_TOLERANCE,
    )
    expected_closure = tuple(
        result.constrained_reaction[axis]
        + result.integrated_body_force[axis]
        + result.integrated_boundary_traction[axis]
        for axis in range(2)
    )
    assert result.momentum_closure == expected_closure
    assert all(abs(component) <= 1.0e-10 for component in result.momentum_closure)

    report = result.solve
    assert report.algorithm == "sparse-lu"
    assert report.preconditioner == "identity"
    assert report.reduction == "fast"
    assert report.relative_tolerance == 1.0e-6
    assert report.absolute_tolerance == 1.0e-13
    assert report.maximum_iterations == 10_000
    assert report.residual_target == RESIDUAL_TARGET
    assert 0.0 <= report.true_residual_norm <= report.residual_target
    assert math.isfinite(report.reported_residual_norm)
    continuity = result.continuity_residual_norm
    weak_bound = report.residual_target + 4096.0 * sys.float_info.epsilon * (
        1.0 + continuity + report.residual_target
    )
    assert math.isfinite(continuity) and 0.0 <= continuity <= weak_bound

    assert repr(result) == (
        f"CircularHoleSteadyStokesResult(run_digest={result.run_digest!r})"
    )
    with pytest.raises(AttributeError):
        result.run_digest = "0" * 64

    pretty_model = json.dumps(json.loads(model_bytes()), indent=2).encode()
    replay = solve(realized, model=pretty_model)
    assert replay == result
    assert hash(replay) == hash(result)
    assert replay.run_digest == result.run_digest


def test_matrix_views_are_memoized_read_only_and_lifetime_safe() -> None:
    source = geometry()
    realized = mesh(source)
    result = solve(realized)
    coordinates = result.coordinates
    triangles = result.triangles
    pressure_owner = result.pressure
    pressure = pressure_owner.numpy(copy=False)

    assert coordinates is result.coordinates
    assert triangles is result.triangles
    assert pressure_owner is result.pressure
    assert coordinates.shape == (104, 2) and coordinates.dtype == np.float64
    assert triangles.shape == (104, 3) and triangles.dtype == np.uint32
    for view in (coordinates, triangles, pressure):
        assert view.flags.c_contiguous and view.flags.aligned
        assert not view.flags.owndata
        assert not view.flags.writeable
        with pytest.raises(ValueError):
            view.setflags(write=True)
        with pytest.raises(ValueError):
            view.flat[0] = 0

    expected_coordinates = coordinates.copy()
    expected_triangles = triangles.copy()
    expected_pressure = pressure.copy()
    del result, pressure_owner, source, realized
    gc.collect()
    np.testing.assert_array_equal(coordinates, expected_coordinates)
    np.testing.assert_array_equal(triangles, expected_triangles)
    np.testing.assert_array_equal(pressure, expected_pressure)


def test_model_and_exact_source_ownership_faults_fail_closed(
    accepted: tuple[Any, Any, Any],
) -> None:
    source, realized, _result = accepted
    assert_error(
        lambda: replayed(b'{"schema":'),
        eqiora.CompatibilityError,
        category="compatibility",
        code="EQ0901",
    )

    changed_revision = json.loads(model_bytes())
    changed_revision["source_revision"] = 2
    revision_bytes = json.dumps(changed_revision, separators=(",", ":")).encode()
    assert semantic_model_digest(revision_bytes) == MODEL_DIGEST
    assert_error(
        lambda: resolve_plan(realized, model=revision_bytes),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    foreign = geometry(tolerance=1.0e-10)
    foreign_mesh = mesh(foreign)
    assert foreign_mesh.digest == realized.digest
    assert foreign_mesh.source_digest != realized.source_digest
    assert_error(
        lambda: resolve_plan(foreign_mesh),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    swapped = geometry(x_lower="outlet", x_upper="inlet")
    swapped_mesh = mesh(swapped)
    assert swapped_mesh.digest == realized.digest
    assert swapped_mesh.source_digest != realized.source_digest
    assert_error(
        lambda: resolve_plan(swapped_mesh),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    coarse = mesh(
        source,
        maximum_boundary_error=0.2,
        minimum_mean_ratio=1.0e-8,
        maximum_boundary_facets=8,
    )
    assert coarse.digest != realized.digest
    assert_error(
        lambda: resolve_plan(coarse),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    plateau = mesh(source, minimum_mean_ratio=1.0e-6)
    assert plateau.digest != realized.digest
    assert_error(
        lambda: resolve_plan(plateau),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    with pytest.raises(TypeError):
        eqiora.fluid.resolve(replayed(), intent(), mesh=object())
    with pytest.raises(TypeError):
        eqiora.fluid.resolve(object(), intent(), mesh=realized)
    with pytest.raises(TypeError):
        eqiora.fluid.resolve(replayed(), object(), mesh=realized)


def test_steady_stokes_intent_is_mandatory_readable_and_fails_closed() -> None:
    requested = intent()
    assert type(requested).__module__ == "eqiora._eqiora"
    assert type(requested).__name__ == "SteadyStokes"
    assert isinstance(requested, eqiora.fluid.SteadyStokes)

    for name, value in INTENT_ARGUMENTS.items():
        assert getattr(requested, name) == value
        with pytest.raises(AttributeError):
            setattr(requested, name, value)
    assert requested.maximum_iterations == MAXIMUM_ITERATIONS
    assert isinstance(requested.maximum_iterations, int)
    assert not isinstance(requested.maximum_iterations, bool)

    assert requested == intent()
    assert hash(requested) == hash(intent())
    assert requested != intent(relative_tolerance=1.0e-7)

    with pytest.raises(TypeError):
        eqiora.fluid.SteadyStokes()
    for omitted in INTENT_ARGUMENTS:
        incomplete = {
            name: value for name, value in INTENT_ARGUMENTS.items() if name != omitted
        }
        with pytest.raises(TypeError):
            eqiora.fluid.SteadyStokes(**incomplete)
    with pytest.raises(TypeError):
        eqiora.fluid.SteadyStokes(*INTENT_ARGUMENTS.values())
    with pytest.raises(TypeError):
        intent(workers=2)
    with pytest.raises(TypeError):
        intent(maximum_iterations=1.5)
    with pytest.raises(TypeError):
        intent(length_scale_m="0.41")

    for rejected in (
        {"length_scale_m": 0.0},
        {"length_scale_m": -0.41},
        {"length_scale_m": float("nan")},
        {"length_scale_m": float("inf")},
        {"velocity_scale_m_per_s": 0.0},
        {"velocity_scale_m_per_s": -0.3},
        {"velocity_scale_m_per_s": float("nan")},
        {"velocity_scale_m_per_s": float("inf")},
        {"pressure_scale_pa": 0.0},
        {"pressure_scale_pa": -PRESSURE_SCALE_PA},
        {"pressure_scale_pa": float("nan")},
        {"pressure_scale_pa": float("inf")},
        {"relative_tolerance": 0.0},
        {"relative_tolerance": -1.0e-6},
        {"relative_tolerance": float("nan")},
        {"relative_tolerance": float("inf")},
        {"absolute_tolerance": 0.0},
        {"absolute_tolerance": -1.0e-13},
        {"absolute_tolerance": float("nan")},
        {"absolute_tolerance": float("inf")},
        {"maximum_iterations": 0},
        {"maximum_iterations": -1},
    ):
        with pytest.raises(eqiora.ValidationError) as caught:
            intent(**rejected)
        assert caught.value.category == "validation"
        assert caught.value.diagnostics
        assert all(item.severity == "error" for item in caught.value.diagnostics)


def test_resolved_plan_publishes_every_effective_value_before_submission(
    accepted: tuple[Any, Any, Any],
) -> None:
    source, realized, result = accepted
    current = replayed()
    resolved = eqiora.fluid.resolve(current, intent(), mesh=realized)
    assert type(resolved).__module__ == "eqiora._eqiora"
    assert type(resolved).__name__ == "SteadyStokesPlan"
    assert isinstance(resolved, eqiora.fluid.SteadyStokesPlan)

    for name in PLAN_PROPERTIES:
        value = getattr(resolved, name)
        with pytest.raises(AttributeError):
            setattr(resolved, name, value)

    assert resolved.model_digest == current.digest == MODEL_DIGEST
    assert resolved.semantic_revision == SEMANTIC_REVISION
    assert resolved.geometry_digest == source.digest == SOURCE_DIGEST
    assert resolved.mesh_digest == realized.digest == MESH_DIGEST
    assert resolved.correspondence_digest == realized.correspondence_digest
    assert resolved.realization_digest == result.realization_digest
    assert resolved.realization_revision == REALIZATION_REVISION
    for identity in (
        resolved.model_digest,
        resolved.geometry_digest,
        resolved.correspondence_digest,
        resolved.mesh_digest,
        resolved.realization_digest,
    ):
        assert_digest(identity)

    assert resolved.spatial_dimension == SPATIAL_DIMENSION
    assert resolved.length_scale_m == LENGTH_SCALE_M
    assert resolved.velocity_scale_m_per_s == VELOCITY_SCALE_M_PER_S
    assert resolved.pressure_scale_pa == PRESSURE_SCALE_PA
    assert resolved.solver_algorithm == SOLVER_ALGORITHM
    assert resolved.preconditioner == PRECONDITIONER
    assert resolved.reduction == REDUCTION
    assert resolved.relative_tolerance == RELATIVE_TOLERANCE
    assert resolved.absolute_tolerance == ABSOLUTE_TOLERANCE
    assert resolved.maximum_iterations == MAXIMUM_ITERATIONS
    assert resolved.solver_backend == SOLVER_BACKEND
    assert resolved.workers == WORKERS

    # The two space names are derived views of the resolved discretization,
    # spelled by the contract owner's frozen literals rather than by shape.
    assert resolved.velocity_space == VELOCITY_SPACE
    assert resolved.pressure_space == PRESSURE_SPACE

    # Every effective value is revalidated by the accepted Run evidence.
    report = result.solve
    assert resolved.solver_algorithm == report.algorithm
    assert resolved.preconditioner == report.preconditioner
    assert resolved.reduction == report.reduction
    assert resolved.relative_tolerance == report.relative_tolerance
    assert resolved.absolute_tolerance == report.absolute_tolerance
    assert resolved.maximum_iterations == report.maximum_iterations
    assert resolved.solver_backend == report.backend
    assert resolved.execution_adapter == report.adapter
    run = json.loads(result.run_manifest_json)
    assert resolved.execution_adapter == run["execution"]["adapter"]
    assert run["execution"]["topology"] == {"kind": "host", "workers": resolved.workers}
    assert resolved.realization_digest == run["realization_sha256"]
    assert resolved.model_digest == run["model_sha256"]
    assert resolved.semantic_revision == run["semantic_revision"]

    envelope = resolved.canonical_bytes
    assert isinstance(envelope, bytes) and envelope
    document = json.loads(envelope)
    assert document["schema"].startswith("eqiora.realization-envelope/")
    assert document["encoding"] == "eqiora.canonical-json/v1"
    assert document["model_sha256"] == resolved.model_digest
    assert document["semantic_revision"] == resolved.semantic_revision
    assert document["source"]["realization_revision"] == resolved.realization_revision
    assert (
        hashlib.sha256(document["schema"].encode() + b"\0" + envelope).hexdigest()
        == resolved.realization_digest
    )

    # Resolution is deterministic and cannot depend on ambient state or on the
    # Python lifetime of its inputs.
    again = eqiora.fluid.resolve(replayed(), intent(), mesh=mesh(geometry()))
    assert again == resolved
    assert hash(again) == hash(resolved)
    assert again.canonical_bytes == envelope
    assert again.velocity_space == resolved.velocity_space
    assert again.pressure_space == resolved.pressure_space
    del current, again
    gc.collect()
    assert resolved.canonical_bytes == envelope
    assert eqiora.submit(replayed(), plan=resolved).result() == result


def test_unsupported_intent_is_refused_during_resolution(
    accepted: tuple[Any, Any, Any],
) -> None:
    realized = accepted[1]
    for unsupported in (
        {"length_scale_m": 0.42},
        {"velocity_scale_m_per_s": 0.31},
        {"pressure_scale_pa": PRESSURE_SCALE_PA * 2.0},
        {"relative_tolerance": 1.0e-11},
        {"absolute_tolerance": 1.0e-14},
        {"maximum_iterations": 9_999},
    ):
        with pytest.raises(eqiora.CapabilityError) as caught:
            resolve_plan(realized, **unsupported)
        assert caught.value.category == "capability"
        assert caught.value.diagnostics
        assert all(item.severity == "error" for item in caught.value.diagnostics)


def test_foreign_model_cannot_submit_an_accepted_plan(
    accepted: tuple[Any, Any, Any],
) -> None:
    realized = accepted[1]
    resolved = resolve_plan(realized)

    changed_revision = json.loads(model_bytes())
    changed_revision["source_revision"] = 2
    revision_bytes = json.dumps(changed_revision, separators=(",", ":")).encode()
    shape_equal = replayed(revision_bytes)
    assert shape_equal.digest == MODEL_DIGEST
    assert_error(
        lambda: eqiora.submit(shape_equal, plan=resolved),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )
    assert_error(
        lambda: eqiora.run(shape_equal, plan=resolved),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    with pytest.raises(TypeError):
        eqiora.submit(replayed(), plan=object())
    with pytest.raises(TypeError):
        eqiora.submit(replayed(), plan=resolved, end_time=1.0, max_step=0.1)
    with pytest.raises(TypeError):
        eqiora.run(replayed(), plan=resolved, end_time=1.0, max_step=0.1)


def test_synchronous_await_and_repeated_result_share_one_occurrence(
    accepted: tuple[Any, Any, Any],
) -> None:
    realized, expected = accepted[1], accepted[2]
    current = replayed()
    resolved = eqiora.fluid.resolve(current, intent(), mesh=realized)

    submitted = eqiora.submit(current, plan=resolved)
    first = submitted.result()
    assert submitted.result() is first
    assert submitted.result() is first
    assert first == expected
    assert first.run_digest == expected.run_digest
    assert submitted.status == eqiora.RunStatus.Completed
    assert submitted.done
    assert submitted.cancellation is None
    assert submitted.model_digest == MODEL_DIGEST
    assert submitted.adapter == first.solve.adapter
    assert isinstance(first, eqiora.fluid.CircularHoleSteadyStokesResult)

    synchronous = eqiora.run(current, plan=resolved)
    assert synchronous == expected
    assert synchronous.run_digest == expected.run_digest

    async def await_result() -> Any:
        return await eqiora.submit(current, plan=resolved)

    awaited = asyncio.run(await_result())
    assert awaited == expected
    assert awaited.run_digest == expected.run_digest
    np.testing.assert_array_equal(
        awaited.pressure.numpy(copy=False), expected.pressure.numpy(copy=False)
    )
    assert awaited.run_manifest_json == expected.run_manifest_json


def test_cancellation_is_honest_and_publishes_no_partial_result(
    accepted: tuple[Any, Any, Any],
) -> None:
    realized = accepted[1]
    current = replayed()
    submitted = eqiora.submit(current, plan=resolve_plan(realized))
    requested = submitted.cancel()

    if requested and submitted.status == eqiora.RunStatus.Cancelled:
        with pytest.raises(eqiora.CancellationError) as caught:
            submitted.result()
        assert caught.value.diagnostics[0].code == "EQ0506"
        assert submitted.cancellation is not None
        assert not submitted.cancel()
    else:
        completed = submitted.result()
        assert submitted.status == eqiora.RunStatus.Completed
        assert completed == accepted[2]
        assert completed.run_digest == accepted[2].run_digest
        assert completed.pressure.shape == (104,)
    assert submitted.done


def test_the_removed_solve_entry_point_is_absent_from_the_installed_surface(
    accepted: tuple[Any, Any, Any],
) -> None:
    result = accepted[2]
    assert REMOVED_ENTRY_POINT not in eqiora.fluid.__all__
    assert not hasattr(eqiora.fluid, REMOVED_ENTRY_POINT)
    assert REMOVED_ENTRY_POINT not in dir(eqiora.fluid)
    assert not hasattr(eqiora, REMOVED_ENTRY_POINT)
    native = sys.modules["eqiora._eqiora"]
    assert not hasattr(native, REMOVED_ENTRY_POINT)
    assert sorted(eqiora.fluid.__all__) == [
        "CircularHoleSteadyStokesResult",
        "SteadyStokes",
        "SteadyStokesPlan",
        "resolve",
    ]

    stub = (
        importlib.resources.files("eqiora")
        .joinpath("fluid.pyi")
        .read_text(encoding="utf-8")
    )
    assert REMOVED_ENTRY_POINT not in stub
    assert "class SteadyStokes:" in stub
    assert "class SteadyStokesPlan:" in stub

    # CircularHoleSteadyStokesResult survives only as this Run output, and the
    # accepted pressure still keeps consuming it unchanged.
    assert isinstance(result, eqiora.fluid.CircularHoleSteadyStokesResult)
    with pytest.raises(TypeError):
        eqiora.fluid.CircularHoleSteadyStokesResult()
    pyplot = pytest.importorskip("matplotlib.pyplot")
    import eqiora.matplotlib as eqplot

    figure = eqplot.plot_pressure(result)
    try:
        assert figure is not None
    finally:
        pyplot.close(figure)


def test_bounded_surface_does_not_claim_the_future_workflow(
    accepted: tuple[Any, Any, Any],
) -> None:
    result = accepted[2]
    assert not hasattr(eqiora.fluid, "solve")
    for unsupported in (
        "velocity",
        "velocity_bubbles",
        "drag",
        "lift",
        "trajectory",
        "plot",
        "animate",
        "save",
    ):
        assert not hasattr(result, unsupported)


def test_isolated_installed_package_path_is_lazy_and_self_contained(
    tmp_path: Path,
) -> None:
    program = f"""
import gc
import hashlib
import importlib.resources
import sys
import eqiora

model = (
    importlib.resources.files("eqiora")
    .joinpath("examples")
    .joinpath("steady-flow-past-cylinder.model.json")
    .read_bytes()
)
assert len(model) == {MODEL_RESOURCE_BYTES}
assert hashlib.sha256(model).hexdigest() == {MODEL_RESOURCE_SHA256!r}
assert model.endswith(b"\\n")
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
assert "numpy" not in sys.modules
assert not hasattr(eqiora.fluid, "solve_exact_cylinder_stokes")
current = eqiora.replay(model)
intent = eqiora.fluid.SteadyStokes(
    length_scale_m={LENGTH_SCALE_M!r},
    velocity_scale_m_per_s={VELOCITY_SCALE_M_PER_S!r},
    pressure_scale_pa={PRESSURE_SCALE_PA!r},
    relative_tolerance={RELATIVE_TOLERANCE!r},
    absolute_tolerance={ABSOLUTE_TOLERANCE!r},
    maximum_iterations={MAXIMUM_ITERATIONS!r},
)
resolved = eqiora.fluid.resolve(current, intent, mesh=mesh)
assert "numpy" not in sys.modules
assert resolved.realization_revision == {REALIZATION_REVISION!r}
assert resolved.solver_algorithm == {SOLVER_ALGORITHM!r}
assert resolved.solver_backend == {SOLVER_BACKEND!r}
assert resolved.workers == {WORKERS!r}
assert resolved.velocity_space == {VELOCITY_SPACE!r}
assert resolved.pressure_space == {PRESSURE_SPACE!r}
assert len(resolved.canonical_bytes) > 0
run = eqiora.submit(current, plan=resolved)
result = run.result()
assert run.result() is result
assert "numpy" not in sys.modules
assert result.model_digest == {MODEL_DIGEST!r}
assert result.exact_source_digest == {SOURCE_DIGEST!r}
assert result.mesh_digest == {MESH_DIGEST!r}
assert result.pressure.shape == (104,)
assert "numpy" not in sys.modules
coordinates = result.coordinates
triangles = result.triangles
assert "numpy" in sys.modules
assert coordinates.shape == (104, 2)
assert triangles.shape == (104, 3)
pressure = result.pressure.numpy(copy=False)
assert pressure[0] == result.pressure[0]
del result, run, resolved, intent, current, geometry, mesh
gc.collect()
print({MODEL_DIGEST!r})
print({SOURCE_DIGEST!r})
print({MESH_DIGEST!r})
print(coordinates.shape[0], triangles.shape[0], pressure.shape[0])
"""
    completed = subprocess.run(
        [sys.executable, "-I", "-c", program],
        cwd=tmp_path,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert completed.stderr == ""
    assert completed.stdout.splitlines() == [
        MODEL_DIGEST,
        SOURCE_DIGEST,
        MESH_DIGEST,
        "104 104 104",
    ]


def test_checked_in_python_demo_runs_with_packaged_model_resource(
    accepted: tuple[Any, Any, Any],
) -> None:
    if not PYTHON_DEMO.is_file():
        pytest.skip("consumer tree does not carry the checked-in Python example")

    model_bytes()
    completed = subprocess.run(
        [sys.executable, "-I", str(PYTHON_DEMO)],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
        timeout=120,
    )

    assert completed.stderr == ""
    lines = completed.stdout.splitlines()
    assert len(lines) == 5
    assert lines[0] == accepted[2].run_digest
    assert lines[1].startswith("LinearSolveSummary(")
    assert lines[2].startswith("pressure ") and lines[2].endswith(" Pa")
    assert lines[3].startswith("cylinder force on fluid ")
    assert lines[3].endswith(" N/m")
    assert lines[4].startswith("net flux ") and lines[4].endswith(" m^2/s")
