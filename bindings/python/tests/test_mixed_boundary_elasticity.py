"""Installed-wheel contract for the converged structural Plan, Run, and Result.

Every scientific, lineage, array, and evidence relation below is the one the
withdrawn `solve_mixed_boundary_elasticity` entry point already owned; only the
ownership path changes. No oracle, expected value, or tolerance is authored
here: `solid.mixed-boundary-elasticity-2d` and
`artifacts.generated-cartesian-q1-spatial-output` remain the authorities.
"""

from __future__ import annotations

import gc
import hashlib
import json
import subprocess
import sys
from importlib.resources import files
from pathlib import Path
from typing import Any

import numpy as np
import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "mixed_boundary_elasticity.py"
MODEL_RESOURCE = files(eqiora).joinpath(
    "examples",
    "mixed-boundary-elasticity.eqi",
)
MODEL_SHA256 = "dd3497c4b412a4171a7bfd18be5963074a093823c11ef2032907335f4779acb5"

CELLS_PER_AXIS = 16
RELATIVE_TOLERANCE = 1.0e-12
ABSOLUTE_TOLERANCE = 1.0e-14
MAXIMUM_ITERATIONS = 10_000
INTENT_ARGUMENTS: dict[str, Any] = {
    "cells_per_axis": CELLS_PER_AXIS,
    "relative_tolerance": RELATIVE_TOLERANCE,
    "absolute_tolerance": ABSOLUTE_TOLERANCE,
    "maximum_iterations": MAXIMUM_ITERATIONS,
}

# The smallest set parallel to the accepted `SteadyStokesPlan` that publishes
# every value the withdrawn entry point kept hidden. The typed literals below
# make the Q1/generated-Cartesian choice inspectable without requiring a caller
# to decode the canonical Realization artifact.
PLAN_PROPERTIES = (
    "model_digest",
    "semantic_revision",
    "geometry_digest",
    "correspondence_digest",
    "mesh_digest",
    "realization_digest",
    "realization_revision",
    "spatial_dimension",
    "cells_per_axis",
    "discretization_method",
    "mesh_kind",
    "mesh_policy",
    "field_space",
    "quadrature",
    "quadrature_points_per_axis",
    "scalar_type",
    "vector_layout",
    "coefficient_association",
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

SEMANTIC_REVISION = 1
REALIZATION_REVISION = 1
SPATIAL_DIMENSION = 2
DISCRETIZATION_METHOD = "continuous-galerkin"
MESH_KIND = "generated-cartesian"
MESH_POLICY = "generated-uniform"
FIELD_SPACE = "continuous-lagrange-1"
QUADRATURE = "gauss-legendre"
QUADRATURE_POINTS_PER_AXIS = 2
SCALAR_TYPE = "f64"
VECTOR_LAYOUT = "replicated"
COEFFICIENT_ASSOCIATION = "vertex"
SOLVER_ALGORITHM = "conjugate-gradient"
PRECONDITIONER = "identity"
REDUCTION = "reproducible"

VERTEX_COUNT = 289
CELL_COUNT = 256
DISPLACEMENT_DIMENSION = (0, 1, 0, 0, 0, 0, 0)
EXACT_BOUNDS = ((0.0, 1.0), (0.0, 1.0))
RUN_FIELDS = (
    "schema",
    "encoding",
    "model_sha256",
    "semantic_revision",
    "realization_sha256",
    "execution",
    "output_sha256",
)

ORDINARY_NAMES = (
    "LinearElasticity",
    "LinearElasticityEvidence",
    "LinearElasticityPlan",
    "linear_elasticity_evidence",
    "resolve",
)
# Retained for exactly one subsequent prerelease by the pre-1.0 compatibility
# rule in docs/development/python-release-policy.md. Their deletion condition
# is the next prerelease boundary; they own no implementation and no evidence.
COMPATIBILITY_NAMES = (
    "MixedBoundaryElasticityResult",
    "solve_mixed_boundary_elasticity",
)
WITHHELD_QUANTITIES = (
    "stress",
    "strain",
    "traction",
    "exact_solution",
    "error_norm",
    "convergence_order",
)


def accepted_model() -> eqiora.Model:
    """Compile the packaged byte-exact source into one fresh exact Model."""

    source = MODEL_RESOURCE.read_text(encoding="utf-8")
    assert hashlib.sha256(source.encode()).hexdigest() == MODEL_SHA256
    return eqiora.compile(
        source,
        filename="mixed-boundary-elasticity.eqi",
    )


def foreign_model() -> eqiora.Model:
    """Compile a Model whose physical meaning differs from the accepted case."""

    source = MODEL_RESOURCE.read_text(encoding="utf-8").replace(
        "parameter mu: kg / (m * s ^ 2) = 3;",
        "parameter mu: kg / (m * s ^ 2) = 4;",
    )
    return eqiora.compile(source, filename="foreign-elasticity.eqi")


def revised_model(model: eqiora.Model) -> eqiora.Model:
    """Commit a value edit: every semantic field id kept, a new exact Model.

    Independent compilation allocates fresh semantic ids, so it alone cannot
    show that an id string never admits a `FieldRef`. This carrier keeps every
    id and changes only the exact Model artifact. It is never resolved, never
    submitted, and never rendered, so no scientific value depends on the edit.
    """

    return model.commit(model.preview_value_edit("mu", 4.0))


def intent(**overrides: object) -> Any:
    arguments: dict[str, Any] = dict(INTENT_ARGUMENTS)
    arguments.update(overrides)
    return eqiora.solid.LinearElasticity(**arguments)


def resolve_plan(model: eqiora.Model, **overrides: object) -> Any:
    return eqiora.solid.resolve(model, intent(**overrides))


def solve() -> tuple[eqiora.Model, Any, eqiora.Result]:
    model = accepted_model()
    plan = resolve_plan(model)
    return model, plan, eqiora.submit(model, plan=plan).result()


@pytest.fixture(scope="module")
def accepted() -> tuple[eqiora.Model, Any, eqiora.Result]:
    return solve()


def displacement_of(model: eqiora.Model) -> eqiora.FieldRef:
    return model.field("displacement")


def assert_digest(value: str) -> None:
    assert len(value) == 64
    assert all(character in "0123456789abcdef" for character in value)


def assert_error(
    operation: Any,
    exception: type[eqiora.EqioraError],
    *,
    category: str,
    code: str | None = None,
) -> None:
    with pytest.raises(exception) as caught:
        operation()
    error = caught.value
    assert error.category == category
    assert error.diagnostics
    assert all(diagnostic.severity == "error" for diagnostic in error.diagnostics)
    if code is not None:
        assert any(diagnostic.code == code for diagnostic in error.diagnostics)


def test_linear_elasticity_intent_is_keyword_only_without_hidden_defaults() -> None:
    requested = intent()
    assert type(requested).__module__ == "eqiora._eqiora"
    assert type(requested).__name__ == "LinearElasticity"
    assert isinstance(requested, eqiora.solid.LinearElasticity)

    for name, value in INTENT_ARGUMENTS.items():
        assert getattr(requested, name) == value
        with pytest.raises(AttributeError):
            setattr(requested, name, value)
    for name in ("cells_per_axis", "maximum_iterations"):
        assert isinstance(getattr(requested, name), int)
        assert not isinstance(getattr(requested, name), bool)

    assert requested == intent()
    assert hash(requested) == hash(intent())
    assert requested != intent(cells_per_axis=8)

    with pytest.raises(TypeError):
        eqiora.solid.LinearElasticity()
    for omitted in INTENT_ARGUMENTS:
        incomplete = {
            name: value for name, value in INTENT_ARGUMENTS.items() if name != omitted
        }
        with pytest.raises(TypeError):
            eqiora.solid.LinearElasticity(**incomplete)
    with pytest.raises(TypeError):
        eqiora.solid.LinearElasticity(*INTENT_ARGUMENTS.values())
    with pytest.raises(TypeError):
        intent(workers=2)
    for wrong_type in (
        {"cells_per_axis": 16.0},
        {"maximum_iterations": 1.5},
        {"relative_tolerance": "1e-12"},
    ):
        with pytest.raises(TypeError):
            intent(**wrong_type)

    for rejected in (
        {"cells_per_axis": 0},
        {"cells_per_axis": -16},
        {"relative_tolerance": 0.0},
        {"relative_tolerance": -RELATIVE_TOLERANCE},
        {"relative_tolerance": float("nan")},
        {"relative_tolerance": float("inf")},
        {"absolute_tolerance": 0.0},
        {"absolute_tolerance": -ABSOLUTE_TOLERANCE},
        {"absolute_tolerance": float("nan")},
        {"absolute_tolerance": float("inf")},
        {"maximum_iterations": 0},
        {"maximum_iterations": -1},
    ):
        assert_error(
            lambda rejected=rejected: intent(**rejected),
            eqiora.ValidationError,
            category="validation",
        )


def test_resolved_plan_publishes_every_effective_value_before_submission() -> None:
    # This exact Plan has not crossed either execution entry point. Keeping this
    # fixture local prevents a completed module-scoped Run from proving values
    # that an implementation populated only while submitting it.
    model = accepted_model()
    plan = resolve_plan(model)
    assert type(plan).__module__ == "eqiora._eqiora"
    assert type(plan).__name__ == "LinearElasticityPlan"
    assert isinstance(plan, eqiora.solid.LinearElasticityPlan)

    pre_execution: dict[str, object] = {}
    for name in PLAN_PROPERTIES:
        value = getattr(plan, name)
        pre_execution[name] = value
        with pytest.raises(AttributeError):
            setattr(plan, name, value)

    assert plan.model_digest == model.digest
    assert plan.semantic_revision == model.revision.number == SEMANTIC_REVISION
    assert plan.realization_revision == REALIZATION_REVISION
    assert plan.spatial_dimension == SPATIAL_DIMENSION
    assert plan.cells_per_axis == CELLS_PER_AXIS
    assert plan.discretization_method == DISCRETIZATION_METHOD
    assert plan.mesh_kind == MESH_KIND
    assert plan.mesh_policy == MESH_POLICY
    assert plan.field_space == FIELD_SPACE
    assert plan.quadrature == QUADRATURE
    assert plan.quadrature_points_per_axis == QUADRATURE_POINTS_PER_AXIS
    assert plan.scalar_type == SCALAR_TYPE
    assert plan.vector_layout == VECTOR_LAYOUT
    assert plan.coefficient_association == COEFFICIENT_ASSOCIATION
    assert plan.solver_algorithm == SOLVER_ALGORITHM
    assert plan.preconditioner == PRECONDITIONER
    assert plan.reduction == REDUCTION
    assert plan.relative_tolerance == RELATIVE_TOLERANCE
    assert plan.absolute_tolerance == ABSOLUTE_TOLERANCE
    assert plan.maximum_iterations == MAXIMUM_ITERATIONS

    identities = (
        plan.model_digest,
        plan.geometry_digest,
        plan.correspondence_digest,
        plan.mesh_digest,
        plan.realization_digest,
    )
    for identity in identities:
        assert_digest(identity)
    assert len(set(identities)) == len(identities)

    envelope = plan.canonical_bytes
    assert isinstance(envelope, bytes) and envelope
    document = json.loads(envelope)
    assert document["schema"].startswith("eqiora.realization-envelope/")
    assert document["encoding"] == "eqiora.canonical-json/v1"
    assert document["model_sha256"] == plan.model_digest
    assert document["semantic_revision"] == plan.semantic_revision
    assert document["source"]["realization_revision"] == plan.realization_revision
    assert (
        hashlib.sha256(document["schema"].encode() + b"\0" + envelope).hexdigest()
        == plan.realization_digest
    )

    # Only after every resolved fact and exact identity is observable does this
    # Plan cross the execution boundary.
    result = eqiora.submit(model, plan=plan).result()
    assert type(result) is eqiora.Result
    assert {name: getattr(plan, name) for name in PLAN_PROPERTIES} == pre_execution
    displacement = displacement_of(model)
    snapshot = result.field(displacement)
    mesh = result.mesh(displacement)
    assert plan.mesh_digest == snapshot.mesh_digest == mesh.digest
    assert plan.correspondence_digest == mesh.correspondence_digest
    assert (
        plan.geometry_digest
        == mesh.source_digest
        == mesh.realized_geometry_digest
    )
    assert plan.realization_digest == mesh.realization_digest
    assert mesh.cells.shape == (CELL_COUNT, 4)
    assert mesh.coordinates.shape == (VERTEX_COUNT, SPATIAL_DIMENSION)

    manifest = result.run_manifest()
    evidence = eqiora.solid.linear_elasticity_evidence(result)
    report = evidence.solve
    assert (
        plan.realization_digest
        == mesh.realization_digest
        == manifest.realization_digest
    )
    assert plan.solver_algorithm == report.algorithm
    assert plan.preconditioner == report.preconditioner
    assert plan.reduction == report.reduction == manifest.reduction
    assert plan.relative_tolerance == report.relative_tolerance
    assert plan.absolute_tolerance == report.absolute_tolerance
    assert plan.maximum_iterations == report.maximum_iterations
    assert plan.solver_backend == report.backend == manifest.solver_backend
    assert plan.execution_adapter == report.adapter == manifest.adapter
    assert plan.execution_adapter == result.adapter
    assert plan.workers == manifest.workers >= 1

    # Resolving one exact Model twice is deterministic, and resubmission
    # reproduces byte-identical Run and output identity.
    again = eqiora.solid.resolve(model, intent())
    assert again == plan
    assert hash(again) == hash(plan)
    assert again.canonical_bytes == envelope
    del again
    gc.collect()
    assert plan.canonical_bytes == envelope

    # A fresh compilation of the byte-exact source is structurally equivalent
    # but is a different exact Model, so it resolves to a different Plan. The
    # resolved Plan still outlives the Python lifetime of that Model.
    independent = resolve_plan(accepted_model())
    gc.collect()
    assert independent != plan
    assert independent.model_digest != plan.model_digest
    assert independent.realization_digest != plan.realization_digest
    assert independent.canonical_bytes != envelope

    repeated = eqiora.run(model, plan=plan)
    assert isinstance(repeated, eqiora.Result)
    assert repeated.run_manifest().digest == manifest.digest
    assert repeated.field(displacement).digest == snapshot.digest
    assert repeated.plan_key == result.plan_key


def test_result_retains_complete_relational_lineage_and_execution(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    model, plan, result = accepted
    assert type(result) is eqiora.Result
    assert result.model_id == model.model_id
    assert result.model_digest == model.digest
    assert result.model_revision == model.revision.number == SEMANTIC_REVISION
    assert result.plan_key
    assert plan.realization_digest in result.plan_key
    assert result.adapter_version
    assert np.isfinite(result.elapsed_seconds) and result.elapsed_seconds > 0.0

    # One static vector output; the temporal Series surface stays empty.
    assert result.fields == []
    assert len(result) == 0
    with pytest.raises(KeyError):
        result["displacement"]
    assert len(result.snapshots) == 1

    snapshot = result.field(displacement_of(model))
    evidence = eqiora.solid.linear_elasticity_evidence(result)
    assert type(evidence).__module__ == "eqiora._eqiora"
    assert type(evidence).__name__ == "LinearElasticityEvidence"
    assert isinstance(evidence, eqiora.solid.LinearElasticityEvidence)
    assert eqiora.solid.linear_elasticity_evidence(result) is evidence
    with pytest.raises(TypeError):
        eqiora.solid.LinearElasticityEvidence()
    with pytest.raises(AttributeError):
        evidence.run_digest = "0" * 64

    manifest = result.run_manifest()
    assert result.run_manifest() is manifest
    assert evidence.run_digest == manifest.digest
    assert manifest.model_digest == result.model_digest
    assert manifest.semantic_revision == result.model_revision
    assert manifest.realization_digest == plan.realization_digest
    assert manifest.output_digests == [snapshot.digest]

    run_bytes = manifest.to_json()
    assert isinstance(run_bytes, bytes)
    run = json.loads(run_bytes)
    assert tuple(run) == RUN_FIELDS
    assert run["schema"] == "eqiora.run-manifest/v2"
    assert run["model_sha256"] == result.model_digest
    assert run["semantic_revision"] == result.model_revision
    assert run["realization_sha256"] == plan.realization_digest
    assert run["output_sha256"] == [snapshot.digest]
    assert run["execution"]["adapter"] == plan.execution_adapter
    assert run["execution"]["solver_backend"] == plan.solver_backend
    assert run["execution"]["topology"]["workers"] == plan.workers
    assert (
        hashlib.sha256(run["schema"].encode() + b"\0" + run_bytes).hexdigest()
        == manifest.digest
    )
    for identity in (evidence.run_digest, snapshot.digest, plan.realization_digest):
        assert_digest(identity)

    report = evidence.solve
    assert report.algorithm == SOLVER_ALGORITHM
    assert report.preconditioner == PRECONDITIONER
    assert report.reduction == REDUCTION
    assert report.relative_tolerance == RELATIVE_TOLERANCE
    assert report.absolute_tolerance == ABSOLUTE_TOLERANCE
    assert report.maximum_iterations == MAXIMUM_ITERATIONS
    assert report.true_residual_norm <= report.residual_target
    assert evidence.assembly_packets > 0
    assert evidence.assembly_targets > 0
    assert len(evidence.constrained_reaction) == SPATIAL_DIMENSION
    assert len(evidence.integrated_body_force) == SPATIAL_DIMENSION
    assert np.isfinite(evidence.constrained_reaction).all()
    assert np.isfinite(evidence.integrated_body_force).all()
    assert evidence.exact_bounds == EXACT_BOUNDS


def test_q1_snapshot_and_mesh_are_complete_coindexed_and_immutable(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    model, _plan, result = accepted
    displacement = displacement_of(model)
    snapshot = result.field(displacement)
    mesh = result.mesh(displacement)

    assert result.field(displacement) is snapshot
    assert result.mesh(displacement) is mesh
    assert result.snapshots[0] is snapshot
    assert snapshot.field == displacement
    assert snapshot.field.model_digest == result.model_digest
    assert snapshot.value_shape == (SPATIAL_DIMENSION,)
    assert snapshot.frame == "spatial-cartesian"
    assert snapshot.dimension == DISPLACEMENT_DIMENSION
    assert snapshot.associations == ("vertex",)
    assert snapshot.mesh_digest == mesh.digest
    assert snapshot.support_domain_id

    coefficients = snapshot.values("vertex")
    support = snapshot.support_indices("vertex")
    coordinates = mesh.coordinates
    cells = mesh.cells

    assert coefficients is snapshot.values("vertex")
    assert support is snapshot.support_indices("vertex")
    assert coordinates is mesh.coordinates
    assert cells is mesh.cells
    assert coefficients.shape == (VERTEX_COUNT, SPATIAL_DIMENSION)
    assert coordinates.shape == (VERTEX_COUNT, SPATIAL_DIMENSION)
    assert cells.shape == (CELL_COUNT, 4)
    assert coefficients.dtype == np.float64
    assert coordinates.dtype == np.float64
    assert cells.dtype == np.uint32
    np.testing.assert_array_equal(support, np.arange(VERTEX_COUNT, dtype=np.uint32))
    assert mesh.dimension == SPATIAL_DIMENSION
    assert mesh.vertex_count == VERTEX_COUNT
    assert mesh.cell_count == CELL_COUNT

    for view in (coefficients, support, coordinates, cells):
        assert view.flags.c_contiguous and view.flags.aligned
        assert not view.flags.writeable
        with pytest.raises(ValueError):
            view.setflags(write=True)
        with pytest.raises(ValueError):
            view.flat[0] = 0

    assert np.isfinite(coefficients).all()
    assert np.isfinite(coordinates).all()
    assert cells.max() < coordinates.shape[0]
    assert all(len(set(cell)) == 4 for cell in cells.tolist())
    evidence = eqiora.solid.linear_elasticity_evidence(result)
    for axis, (lower, upper) in enumerate(evidence.exact_bounds):
        assert float(coordinates[:, axis].min()) == lower
        assert float(coordinates[:, axis].max()) == upper


def test_array_owners_survive_result_release_and_solves_do_not_share_storage() -> None:
    first_model, _, first = solve()
    second_model, _, second = solve()
    first_displacement = displacement_of(first_model)
    second_displacement = displacement_of(second_model)
    first_arrays = (
        first.mesh(first_displacement).coordinates,
        first.mesh(first_displacement).cells,
        first.field(first_displacement).values("vertex"),
        first.field(first_displacement).support_indices("vertex"),
    )
    second_arrays = (
        second.mesh(second_displacement).coordinates,
        second.mesh(second_displacement).cells,
        second.field(second_displacement).values("vertex"),
        second.field(second_displacement).support_indices("vertex"),
    )

    for left, right in zip(first_arrays, second_arrays, strict=True):
        np.testing.assert_array_equal(left, right)
        assert not np.shares_memory(left, right)

    expected = tuple(array.copy() for array in first_arrays)
    del first, first_model, first_displacement
    gc.collect()
    for array, reference in zip(first_arrays, expected, strict=True):
        assert array.size > 0 and not array.flags.writeable
        np.testing.assert_array_equal(array, reference)


def test_foreign_model_and_foreign_exact_plan_are_rejected_before_execution(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    model, plan, _result = accepted

    assert_error(
        lambda: resolve_plan(foreign_model()),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )
    assert_error(
        lambda: eqiora.submit(foreign_model(), plan=plan),
        eqiora.ValidationError,
        category="validation",
        code="EQ0807",
    )

    # An independent compilation of the byte-exact source is structurally
    # equivalent but is a different exact Model, so it cannot submit this Plan.
    independent = accepted_model()
    assert model.structurally_equivalent(independent)
    assert model.digest != independent.digest
    for entry_point in (eqiora.submit, eqiora.run):
        assert_error(
            lambda entry_point=entry_point: entry_point(independent, plan=plan),
            eqiora.ValidationError,
            category="validation",
            code="EQ0807",
        )

    with pytest.raises(TypeError):
        eqiora.solid.resolve(object(), intent())
    with pytest.raises(TypeError):
        eqiora.solid.resolve(model, object())
    with pytest.raises(TypeError):
        eqiora.submit(model, plan=object())
    with pytest.raises(TypeError):
        eqiora.submit(model, plan=plan, end_time=1.0, max_step=0.1)
    with pytest.raises(TypeError):
        eqiora.run(model, plan=plan, end_time=1.0, max_step=0.1)


def test_unsupported_intent_tuple_is_refused_during_resolution(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    model = accepted[0]
    for unsupported in (
        {"cells_per_axis": 8},
        {"cells_per_axis": 32},
        {"relative_tolerance": 1.0e-11},
        {"absolute_tolerance": 1.0e-13},
        {"maximum_iterations": 9_999},
    ):
        assert_error(
            lambda unsupported=unsupported: resolve_plan(model, **unsupported),
            eqiora.CapabilityError,
            category="capability",
        )


def test_cross_physics_evidence_selection_rejects(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    result = accepted[2]
    state = eqiora.Field("x", initial=1.0)
    reference_model = eqiora.Model.define(
        "hold",
        state,
        eqiora.Relation("hold", residual=eqiora.derivative(state)),
    )
    reference = eqiora.run(reference_model, end_time=0.1, max_step=0.1)

    assert_error(
        lambda: eqiora.fluid.steady_stokes_evidence(result),
        eqiora.CapabilityError,
        category="capability",
    )
    assert_error(
        lambda: eqiora.solid.linear_elasticity_evidence(reference),
        eqiora.CapabilityError,
        category="capability",
    )
    for rejected in (object(), result.run_manifest()):
        with pytest.raises(TypeError):
            eqiora.solid.linear_elasticity_evidence(rejected)


def test_result_field_and_mesh_enforce_exact_field_identity(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    model, _plan, result = accepted
    displacement = displacement_of(model)
    snapshot = result.field(displacement)

    absent = model.field("load_potential")
    assert absent != displacement
    with pytest.raises(KeyError):
        result.field(absent)
    with pytest.raises(KeyError):
        result.mesh(absent)

    revised = revised_model(model)
    same_id_foreign_model = revised.field("displacement")
    assert same_id_foreign_model.id == displacement.id
    assert same_id_foreign_model.model_digest != displacement.model_digest
    different_id_foreign_model = accepted_model().field("displacement")
    assert different_id_foreign_model.id != displacement.id
    for rejected in (same_id_foreign_model, different_id_foreign_model):
        with pytest.raises(ValueError, match="different exact Model"):
            result.field(rejected)
        with pytest.raises(ValueError, match="different exact Model"):
            result.mesh(rejected)

    for wrong_type in (object(), displacement.id):
        with pytest.raises(TypeError):
            result.field(wrong_type)
        with pytest.raises(TypeError):
            result.mesh(wrong_type)

    assert result.field(displacement) is snapshot
    with pytest.raises(TypeError):
        eqiora.trajectory.FieldSnapshot()


def test_surface_does_not_claim_uncomputed_structural_quantities(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    _model, plan, result = accepted
    evidence = eqiora.solid.linear_elasticity_evidence(result)
    for owner in (result, plan, evidence):
        assert set(dir(owner)).isdisjoint(WITHHELD_QUANTITIES)
    assert not hasattr(eqiora.solid, "solve")


def test_numpy_import_is_lazy_until_snapshot_projection(tmp_path: Path) -> None:
    program = f"""
import sys
from importlib.resources import files
import eqiora

assert "numpy" not in sys.modules
source = files(eqiora).joinpath(
    "examples", "mixed-boundary-elasticity.eqi"
).read_text()
model = eqiora.compile(
    source,
    filename="mixed-boundary-elasticity.eqi",
)
intent = eqiora.solid.LinearElasticity(
    cells_per_axis={CELLS_PER_AXIS!r},
    relative_tolerance={RELATIVE_TOLERANCE!r},
    absolute_tolerance={ABSOLUTE_TOLERANCE!r},
    maximum_iterations={MAXIMUM_ITERATIONS!r},
)
plan = eqiora.solid.resolve(model, intent)
assert "numpy" not in sys.modules
result = eqiora.submit(model, plan=plan).result()
displacement = model.field("displacement")
snapshot = result.field(displacement)
evidence = eqiora.solid.linear_elasticity_evidence(result)
assert evidence.run_digest == result.run_manifest().digest
assert snapshot.value_shape == ({SPATIAL_DIMENSION!r},)
assert "numpy" not in sys.modules
assert result.mesh(displacement).coordinates.shape == (
    {VERTEX_COUNT!r},
    {SPATIAL_DIMENSION!r},
)
assert "numpy" in sys.modules
assert snapshot.values("vertex").shape == ({VERTEX_COUNT!r}, {SPATIAL_DIMENSION!r})
"""
    completed = subprocess.run(
        [sys.executable, "-I", "-c", program],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_converged_names_are_exported_and_predecessors_are_deprecated_shims(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    model, _plan, result = accepted
    assert set(eqiora.solid.__all__) == set(ORDINARY_NAMES) | set(COMPATIBILITY_NAMES)
    assert sorted(eqiora.solid.__all__) == list(eqiora.solid.__all__)
    for name in ORDINARY_NAMES:
        assert hasattr(eqiora.solid, name)

    # The compatibility type name is resolved lazily so ordinary import of the
    # solid namespace stays quiet, while both attribute access and the retained
    # import spelling tell callers which common type replaces it.
    with pytest.warns(DeprecationWarning, match="eqiora.Result"):
        compatibility_result = getattr(
            eqiora.solid,
            "MixedBoundaryElasticityResult",
        )
    imported: dict[str, object] = {}
    with pytest.warns(DeprecationWarning, match="eqiora.Result"):
        exec(
            "from eqiora.solid import MixedBoundaryElasticityResult",
            imported,
        )
    assert compatibility_result is eqiora.Result
    assert imported["MixedBoundaryElasticityResult"] is compatibility_result

    stub = files(eqiora).joinpath("solid.pyi").read_text(encoding="utf-8")
    for declaration in (
        "class LinearElasticity:",
        "class LinearElasticityEvidence:",
        "class LinearElasticityPlan:",
        "def resolve(",
        "def linear_elasticity_evidence(",
    ):
        assert declaration in stub
    for retained in COMPATIBILITY_NAMES:
        assert retained in stub
    package_stub = files(eqiora).joinpath("__init__.pyi").read_text(encoding="utf-8")
    assert "solid.LinearElasticityPlan" in package_stub

    # One delegation path per predecessor name. They own no type and no
    # independent lineage: the retained result name is only a projection of the
    # common `eqiora.Result`, and the shim replays the accepted Run exactly.
    with pytest.warns(DeprecationWarning, match="eqiora.solid.resolve"):
        delegated = eqiora.solid.solve_mixed_boundary_elasticity(model)
    assert type(delegated) is eqiora.Result
    assert isinstance(delegated, compatibility_result)
    with pytest.raises(TypeError):
        compatibility_result()
    assert delegated.run_manifest().digest == result.run_manifest().digest
    assert delegated.model_id == result.model_id
    assert delegated.model_digest == result.model_digest
    assert delegated.model_revision == result.model_revision
    assert delegated.plan_key == result.plan_key
    for withdrawn in ("run_digest", "case_id"):
        assert not hasattr(delegated, withdrawn)


def test_checked_in_python_demo_runs_with_packaged_model_resource(
    accepted: tuple[eqiora.Model, Any, eqiora.Result],
) -> None:
    if not PYTHON_DEMO.is_file():
        pytest.skip("consumer tree does not carry the checked-in Python example")

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
    assert len(lines) == 4
    assert lines[0] == accepted[2].run_manifest().digest
    assert lines[1].startswith("LinearSolveSummary(")
    assert lines[2].startswith("constrained reaction ") and lines[2].endswith(" N")
    assert lines[3].startswith("integrated body force ") and lines[3].endswith(" N")
