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
MESH_DIGEST = "5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b"
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
REMOVED_RESULT_TYPE = "CircularHoleSteadyStokesResult"

PRESSURE_DIMENSION = (1, -1, -2, 0, 0, 0, 0)
PRESSURE_TOLERANCE = 2.0e-14 + 5.0e-7 * (0.001 * 0.3 / 0.41)
FLUX_TOLERANCE = 2.0e-13 + 5.0e-7 * (0.3 * 0.41)
REACTION_TOLERANCE = 2.0e-14 + 5.0e-7 * (0.001 * 0.3)
RESIDUAL_TARGET = 6.138_485_578_780_151e-6

PRESSURE_PROBES = (
    ((0.15, 0.2), 0.06959832738138942),
    ((0.25, 0.2), 0.019333181397105),
    ((0.1968604740235343, 0.1500986635785864), 0.04389626088659296),
    ((0.1968604740235343, 0.2499013364214136), 0.045165230577321865),
    ((0.0, 0.2), 0.062148654204247),
    ((2.2, 0.2), 0.0004742049675737538),
)
EXPECTED_INLET_FLUX = -0.08149573099927537
EXPECTED_OUTLET_FLUX = 0.08149573099927537
EXPECTED_CYLINDER_REACTION = (-0.006384200476069211, -0.00006344553664047762)
EXPECTED_GLOBAL_REACTION = (7.368560570709604e-63, -6.624108059442036e-62)
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


def parameter_value_variant(encoded: bytes) -> bytes:
    """Change one Parameter value without changing any semantic node identity."""

    document = json.loads(encoded)
    parameter = next(
        node for node in document["nodes"] if node["id"]["kind"] == "parameter"
    )
    identifier = parameter["id"]["ulid"]
    original = parameter["definition"]["value"]["value"]
    replacement = original + 1.0
    parameter["definition"]["value"]["value"] = replacement
    value = next(
        item
        for item in document["values"]
        if item["target"] == {"kind": "parameter", "ulid": identifier}
    )
    assert value["value"]["value"] == original
    value["value"]["value"] = replacement
    return json.dumps(document, separators=(",", ":"), ensure_ascii=False).encode()


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
    assert type(result).__name__ == "Result"
    assert isinstance(result, eqiora.Result)

    current = replayed()
    resolved = eqiora.fluid.resolve(current, intent(), mesh=realized)
    assert result.model_id == current.model_id
    assert result.model_digest == MODEL_DIGEST == current.digest
    assert result.model_revision == SEMANTIC_REVISION
    plan_key_prefix = f"steady-stokes-v1:{resolved.realization_digest}:"
    assert result.plan_key.startswith(plan_key_prefix)
    assert result.plan_key.removeprefix(plan_key_prefix).startswith(
        f"{resolved.solver_backend}@"
    )
    assert not result.plan_key.endswith("@")
    assert result.adapter == resolved.execution_adapter
    assert result.adapter_version
    assert math.isfinite(result.elapsed_seconds) and result.elapsed_seconds > 0.0
    assert result.fields == []
    assert len(result) == 0
    with pytest.raises(KeyError):
        result["pressure"]

    assert len(result.snapshots) == 1
    snapshot = result.snapshots[0]
    assert result.field(snapshot.field) is snapshot
    assert result.mesh(snapshot.field) is realized
    assert snapshot.mesh_digest == realized.digest
    assert snapshot.field.model_digest == result.model_digest
    assert snapshot.associations == ("vertex",)
    assert snapshot.value_shape == ()
    assert snapshot.frame == "invariant"

    evidence = eqiora.fluid.steady_stokes_evidence(result)
    assert type(evidence).__module__ == "eqiora._eqiora"
    assert type(evidence).__name__ == "SteadyStokesEvidence"
    assert isinstance(evidence, eqiora.fluid.SteadyStokesEvidence)
    assert eqiora.fluid.steady_stokes_evidence(result) is evidence
    with pytest.raises(TypeError):
        eqiora.fluid.SteadyStokesEvidence()

    assert source.digest == SOURCE_DIGEST
    assert realized.source_digest == SOURCE_DIGEST
    assert snapshot.mesh_digest == realized.digest == MESH_DIGEST
    assert snapshot.dimension == PRESSURE_DIMENSION
    pressure_field_id, support_domain_id = model_result_ids(model_bytes())
    assert snapshot.field.id == pressure_field_id
    assert snapshot.support_domain_id == support_domain_id
    assert evidence.exact_bounds == ((0.0, 2.2), (0.0, 0.41))
    plan = mesh_plan(source)
    assert plan.request.maximum_boundary_error == 1.0e-4
    assert plan.boundary_evaluation_allowance > 0.0
    assert plan.boundary_error_bound >= plan.boundary_evaluation_allowance
    assert plan.boundary_facets == 50

    identities = (
        result.model_digest,
        source.digest,
        realized.realized_geometry_digest,
        realized.digest,
        realized.correspondence_digest,
        realized.realization_digest,
        resolved.realization_digest,
        result.run_manifest().digest,
        snapshot.digest,
    )
    for identity in identities:
        assert_digest(identity)
    assert len(set(identities)) == len(identities)

    binding_bytes = plan.canonical_bytes
    assert isinstance(binding_bytes, bytes)
    binding = json.loads(binding_bytes)
    assert tuple(binding) == BINDING_FIELDS
    assert binding["schema"] == ("eqiora.circular-hole-chordal-realization-envelope/v1")
    assert binding["source_geometry_sha256"] == source.digest
    assert binding["realized_geometry_sha256"] == realized.realized_geometry_digest
    assert binding["mesh_sha256"] == realized.digest
    assert binding["correspondence_sha256"] == realized.correspondence_digest
    assert binding["requested_max_boundary_error_m"] == (
        plan.request.maximum_boundary_error
    )
    assert binding["boundary_evaluation_allowance_m"] == (
        plan.boundary_evaluation_allowance
    )
    assert binding["boundary_error_bound_m"] == plan.boundary_error_bound
    assert binding["circle_segments"] == plan.boundary_facets
    assert binding["required_minimum_mean_ratio"] == 1.0e-5
    assert plan.request.minimum_mean_ratio == 1.0e-5
    assert binding["circle_area_deficit_m2"] > 0.0
    assert binding["circle_perimeter_deficit_m"] > 0.0
    assert (
        hashlib.sha256(binding["schema"].encode() + b"\0" + binding_bytes).hexdigest()
        == realized.realization_digest
    )

    manifest = result.run_manifest()
    assert result.run_manifest() is manifest
    run_bytes = manifest.to_json()
    assert isinstance(run_bytes, bytes)
    run = json.loads(run_bytes)
    assert tuple(run) == RUN_FIELDS
    assert run["schema"] == "eqiora.run-manifest/v2"
    assert run["model_sha256"] == result.model_digest
    assert run["semantic_revision"] == result.model_revision
    assert run["realization_sha256"] == resolved.realization_digest
    assert run["output_sha256"] == [snapshot.digest]
    assert run["execution"]["solver_backend"] == evidence.solve.backend == "eqiora.faer"
    assert run["execution"]["adapter"] == evidence.solve.adapter == result.adapter
    assert run["execution"]["reduction"] == evidence.solve.reduction == "fast"
    assert run["execution"]["topology"] == {"kind": "host", "workers": 1}
    assert run["execution"]["libraries"]["faer"] == "0.24.4"
    assert (
        hashlib.sha256(run["schema"].encode() + b"\0" + run_bytes).hexdigest()
        == manifest.digest
    )
    assert evidence.run_digest == manifest.digest
    assert manifest.model_digest == result.model_digest
    assert manifest.semantic_revision == result.model_revision
    assert manifest.realization_digest == resolved.realization_digest
    assert manifest.output_digests == [snapshot.digest]
    assert manifest.adapter == result.adapter
    assert manifest.adapter_version == result.adapter_version

    mesh_document = json.loads(realized.canonical_bytes)
    coordinates = realized.coordinates
    triangles = realized.cells
    pressure = snapshot.values("vertex")
    np.testing.assert_array_equal(
        coordinates, np.asarray(mesh_document["vertices"], dtype=np.float64)
    )
    np.testing.assert_array_equal(
        triangles, np.asarray(mesh_document["cells"], dtype=np.uint32)
    )
    assert pressure.shape == (662,)
    assert np.isfinite(pressure).all()
    assert float(pressure.min()) == evidence.pressure_minimum
    assert float(pressure.max()) == evidence.pressure_maximum
    assert len(pressure) == 662
    assert pressure[-1] == pressure[-1]
    with pytest.raises(IndexError):
        pressure[-sys.maxsize]
    with pytest.raises(IndexError):
        pressure[len(pressure)]
    with pytest.raises(IndexError):
        pressure[sys.maxsize]

    for position, expected in PRESSURE_PROBES:
        squared_distance = np.square(
            coordinates - np.asarray(position, dtype=np.float64)
        ).sum(axis=1)
        index = int(np.argmin(squared_distance))
        assert squared_distance[index] <= plan.boundary_evaluation_allowance**2
        assert abs(pressure[index] - expected) <= PRESSURE_TOLERANCE

    assert abs(evidence.inlet_flux - EXPECTED_INLET_FLUX) <= FLUX_TOLERANCE
    assert abs(evidence.outlet_flux - EXPECTED_OUTLET_FLUX) <= FLUX_TOLERANCE
    assert evidence.net_flux == evidence.inlet_flux + evidence.outlet_flux
    assert abs(evidence.net_flux) <= 1.0e-8
    assert_vector_close(
        evidence.cylinder_force_on_fluid,
        EXPECTED_CYLINDER_REACTION,
        REACTION_TOLERANCE,
    )
    assert_vector_close(
        evidence.constrained_reaction,
        EXPECTED_GLOBAL_REACTION,
        REACTION_TOLERANCE,
    )
    assert_vector_close(
        evidence.integrated_body_force,
        EXPECTED_ZERO_FORCE,
        REACTION_TOLERANCE,
    )
    assert_vector_close(
        evidence.integrated_boundary_traction,
        EXPECTED_ZERO_FORCE,
        REACTION_TOLERANCE,
    )
    expected_closure = tuple(
        evidence.constrained_reaction[axis]
        + evidence.integrated_body_force[axis]
        + evidence.integrated_boundary_traction[axis]
        for axis in range(2)
    )
    assert evidence.momentum_closure == expected_closure
    assert all(abs(component) <= 1.0e-10 for component in evidence.momentum_closure)

    report = evidence.solve
    assert report.algorithm == "sparse-lu"
    assert report.preconditioner == "identity"
    assert report.reduction == "fast"
    assert report.relative_tolerance == 1.0e-6
    assert report.absolute_tolerance == 1.0e-13
    assert report.maximum_iterations == 10_000
    assert report.residual_target == RESIDUAL_TARGET
    assert 0.0 <= report.true_residual_norm <= report.residual_target
    assert math.isfinite(report.reported_residual_norm)
    continuity = evidence.continuity_residual_norm
    weak_bound = report.residual_target + 4096.0 * sys.float_info.epsilon * (
        1.0 + continuity + report.residual_target
    )
    assert math.isfinite(continuity) and 0.0 <= continuity <= weak_bound

    with pytest.raises(AttributeError):
        result.model_digest = "0" * 64
    with pytest.raises(AttributeError):
        evidence.run_digest = "0" * 64

    pretty_model = json.dumps(json.loads(model_bytes()), indent=2).encode()
    replay = solve(realized, model=pretty_model)
    assert replay.run_manifest().digest == manifest.digest
    assert replay.snapshots[0].digest == snapshot.digest
    np.testing.assert_array_equal(
        replay.snapshots[0].values("vertex"),
        pressure,
    )


def test_matrix_views_are_memoized_read_only_and_lifetime_safe() -> None:
    source = geometry()
    realized = mesh(source)
    result = solve(realized)
    snapshot = result.snapshots[0]
    coordinates = result.mesh(snapshot.field).coordinates
    triangles = result.mesh(snapshot.field).cells
    pressure = snapshot.values("vertex")
    support = snapshot.support_indices("vertex")

    assert result.field(snapshot.field) is snapshot
    assert result.mesh(snapshot.field) is realized
    assert coordinates is realized.coordinates
    assert triangles is realized.cells
    assert pressure is snapshot.values("vertex")
    assert support is snapshot.support_indices("vertex")
    assert coordinates.shape == (662, 2) and coordinates.dtype == np.float64
    assert triangles.shape == (1210, 3) and triangles.dtype == np.uint32
    np.testing.assert_array_equal(support, np.arange(662, dtype=np.uint32))
    for view in (coordinates, triangles, pressure, support):
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
    expected_support = support.copy()
    del result, snapshot, source, realized
    gc.collect()
    np.testing.assert_array_equal(coordinates, expected_coordinates)
    np.testing.assert_array_equal(triangles, expected_triangles)
    np.testing.assert_array_equal(pressure, expected_pressure)
    np.testing.assert_array_equal(support, expected_support)


def test_static_result_field_and_mesh_identity_fail_closed(
    accepted: tuple[Any, Any, Any],
) -> None:
    _source, realized, result = accepted
    snapshot = result.snapshots[0]
    current = replayed()
    accepted_field = snapshot.field
    assert current.field(accepted_field.id) == accepted_field

    absent_id = next(
        identifier
        for identifier in current.field_ids
        if identifier != accepted_field.id
    )
    absent = current.field(absent_id)
    with pytest.raises(KeyError):
        result.field(absent)
    with pytest.raises(KeyError):
        result.mesh(absent)

    foreign_artifact = replayed(parameter_value_variant(model_bytes()))
    same_id_foreign_model = foreign_artifact.field(accepted_field.id)
    assert same_id_foreign_model.id == accepted_field.id
    assert same_id_foreign_model.model_digest != accepted_field.model_digest

    foreign_declaration = eqiora.Field("foreign", initial=1.0)
    foreign_model = eqiora.Model.define(
        "foreign",
        foreign_declaration,
        eqiora.Relation(
            "hold",
            residual=eqiora.derivative(foreign_declaration),
        ),
    )
    different_id_foreign_model = foreign_model.field("foreign")
    for rejected in (same_id_foreign_model, different_id_foreign_model):
        with pytest.raises(ValueError, match="different exact Model"):
            result.field(rejected)
        with pytest.raises(ValueError, match="different exact Model"):
            result.mesh(rejected)

    for rejected in (object(), accepted_field.id):
        with pytest.raises(TypeError):
            result.field(rejected)  # type: ignore[arg-type]
        with pytest.raises(TypeError):
            result.mesh(rejected)  # type: ignore[arg-type]

    assert result.field(accepted_field) is snapshot
    assert result.mesh(accepted_field) is realized
    with pytest.raises(TypeError):
        eqiora.trajectory.FieldSnapshot()


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
    manifest = result.run_manifest()
    evidence = eqiora.fluid.steady_stokes_evidence(result)
    assert resolved.realization_digest == manifest.realization_digest
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
    report = evidence.solve
    assert resolved.solver_algorithm == report.algorithm
    assert resolved.preconditioner == report.preconditioner
    assert resolved.reduction == report.reduction
    assert resolved.relative_tolerance == report.relative_tolerance
    assert resolved.absolute_tolerance == report.absolute_tolerance
    assert resolved.maximum_iterations == report.maximum_iterations
    assert resolved.solver_backend == report.backend
    assert resolved.execution_adapter == report.adapter
    run = json.loads(manifest.to_json())
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
    repeated = eqiora.submit(replayed(), plan=resolved).result()
    assert repeated.run_manifest().digest == manifest.digest
    assert repeated.snapshots[0].digest == result.snapshots[0].digest


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
    assert isinstance(first, eqiora.Result)
    assert first.run_manifest().digest == expected.run_manifest().digest
    assert first.snapshots[0].digest == expected.snapshots[0].digest
    assert submitted.status == eqiora.RunStatus.Completed
    assert submitted.done
    assert submitted.cancellation is None
    assert submitted.model_digest == MODEL_DIGEST
    assert submitted.model_id == first.model_id
    assert submitted.model_revision == first.model_revision
    assert submitted.plan_key == first.plan_key
    assert submitted.adapter == first.adapter

    synchronous = eqiora.run(current, plan=resolved)
    assert isinstance(synchronous, eqiora.Result)
    assert synchronous.run_manifest().digest == expected.run_manifest().digest

    async def await_result() -> Any:
        return await eqiora.submit(current, plan=resolved)

    awaited = asyncio.run(await_result())
    assert isinstance(awaited, eqiora.Result)
    assert awaited.run_manifest().digest == expected.run_manifest().digest
    np.testing.assert_array_equal(
        awaited.snapshots[0].values("vertex"),
        expected.snapshots[0].values("vertex"),
    )
    assert awaited.run_manifest().to_json() == expected.run_manifest().to_json()


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
        assert completed.run_manifest().digest == accepted[2].run_manifest().digest
        assert completed.snapshots[0].digest == accepted[2].snapshots[0].digest
        assert completed.snapshots[0].values("vertex").shape == (662,)
    assert submitted.done


def test_demo_specific_result_and_solve_entry_point_are_absent(
    accepted: tuple[Any, Any, Any],
) -> None:
    result = accepted[2]
    native = sys.modules["eqiora._eqiora"]
    for removed in (REMOVED_ENTRY_POINT, REMOVED_RESULT_TYPE):
        assert removed not in eqiora.fluid.__all__
        assert not hasattr(eqiora.fluid, removed)
        assert removed not in dir(eqiora.fluid)
        assert not hasattr(eqiora, removed)
        assert not hasattr(native, removed)
    assert sorted(eqiora.fluid.__all__) == [
        "SteadyStokes",
        "SteadyStokesEvidence",
        "SteadyStokesPlan",
        "resolve",
        "steady_stokes_evidence",
    ]

    stub = (
        importlib.resources.files("eqiora")
        .joinpath("fluid.pyi")
        .read_text(encoding="utf-8")
    )
    assert REMOVED_ENTRY_POINT not in stub
    assert REMOVED_RESULT_TYPE not in stub
    assert "class SteadyStokes:" in stub
    assert "class SteadyStokesEvidence:" in stub
    assert "class SteadyStokesPlan:" in stub

    assert type(result) is eqiora.Result
    assert isinstance(
        eqiora.fluid.steady_stokes_evidence(result),
        eqiora.fluid.SteadyStokesEvidence,
    )
    pyplot = pytest.importorskip("matplotlib.pyplot")
    import eqiora.matplotlib as eqplot

    assert not hasattr(eqplot, "plot_pressure")
    figure = eqplot.plot_scalar_field(
        result,
        field=result.snapshots[0].field,
    )
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
        "plot",
        "animate",
        "save",
    ):
        assert not hasattr(result, unsupported)
    with pytest.raises(eqiora.CapabilityError):
        _ = result.trajectory


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
assert type(result) is eqiora.Result
assert result.model_digest == {MODEL_DIGEST!r}
assert result.model_id == current.model_id
assert result.model_revision == {SEMANTIC_REVISION!r}
assert result.fields == []
assert len(result.snapshots) == 1
snapshot = result.snapshots[0]
assert result.field(snapshot.field) is snapshot
assert snapshot.mesh_digest == {MESH_DIGEST!r}
assert snapshot.value_shape == ()
assert snapshot.associations == ("vertex",)
assert result.mesh(snapshot.field) is mesh
manifest = result.run_manifest()
evidence = eqiora.fluid.steady_stokes_evidence(result)
assert evidence.run_digest == manifest.digest
assert "numpy" not in sys.modules
coordinates = result.mesh(snapshot.field).coordinates
triangles = result.mesh(snapshot.field).cells
assert "numpy" in sys.modules
assert coordinates.shape == (662, 2)
assert triangles.shape == (1210, 3)
pressure = snapshot.values("vertex")
assert pressure.shape == (662,)
assert pressure[0] == snapshot.values("vertex")[0]
del result, snapshot, run, resolved, intent, current, geometry, mesh
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
        "662 1210 662",
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
    assert lines[0] == accepted[2].run_manifest().digest
    assert lines[1].startswith("LinearSolveSummary(")
    assert lines[2].startswith("pressure ") and lines[2].endswith(" Pa")
    assert lines[3].startswith("cylinder force on fluid ")
    assert lines[3].endswith(" N/m")
    assert lines[4].startswith("net flux ") and lines[4].endswith(" m^2/s")
