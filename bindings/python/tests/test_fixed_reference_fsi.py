"""Installed-wheel contract for the converged fixed-mesh monolithic FSI path.

The registered FSI and trajectory evidence remains the sole scientific oracle.
These tests move its existing observations from demo-owned DTOs onto an explicit
Plan, common Result/Trajectory, and typed evidence without changing a value.
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
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "fixed_reference_fsi.py"
MODEL_RESOURCE = files(eqiora).joinpath("examples", "fixed-reference-fsi.eqi")
MODEL_SHA256 = "f4da68623779af8795653468a57c1957cc3595ef2c3e6c8c9c76688b4778a362"
EXPECTED_CELLS = np.array(
    [
        [0, 1, 3],
        [0, 3, 2],
        [2, 3, 5],
        [2, 5, 4],
        [1, 6, 7],
        [1, 7, 3],
        [3, 7, 8],
        [3, 8, 5],
    ],
    dtype=np.uint32,
)
INTENT_ARGUMENTS: dict[str, Any] = {
    "time_step_s": 0.05,
    "steps": 2,
    "initial_velocity_m_per_s": (0.0, 0.0),
    "initial_free_interface_displacement_m": (0.02, 0.0),
    "length_scale_m": 2.0,
    "velocity_scale_m_per_s": 0.5,
    "pressure_scale_pa": 4.0,
    "relative_tolerance": 1.0e-11,
    "absolute_tolerance": 1.0e-13,
    "maximum_iterations": 20_000,
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
    "coupling_method",
    "geometry_motion",
    "mesh_kind",
    "fluid_velocity_space",
    "fluid_pressure_space",
    "solid_velocity_space",
    "solid_displacement_space",
    "time_integrator",
    *INTENT_ARGUMENTS,
    "solver_algorithm",
    "preconditioner",
    "reduction",
    "solver_backend",
    "execution_adapter",
    "workers",
    "canonical_bytes",
)
RETIRED_NAMES = (
    "FixedReferenceFsiStep",
    "FixedReferenceFsiResult",
    "solve_fixed_reference_fsi",
)
# Frozen dual-oracle support membership, wired verbatim from registered evidence.
EXPECTED_SUPPORT = {
    ("fluid_velocity", "vertex"): [0, 1, 2, 3, 4, 5],
    ("fluid_velocity", "cell"): [0, 1, 2, 3],
    ("fluid_pressure", "vertex"): [0, 1, 2, 3, 4, 5],
    ("solid_displacement", "vertex"): [1, 3, 5, 6, 7, 8],
    ("solid_velocity", "vertex"): [1, 3, 5, 6, 7, 8],
}


def accepted_model() -> eqiora.Model:
    source = MODEL_RESOURCE.read_text(encoding="utf-8")
    assert hashlib.sha256(source.encode()).hexdigest() == MODEL_SHA256
    return eqiora.compile(
        source,
        filename="fixed-reference-fsi.eqi",
    )


def foreign_model() -> eqiora.Model:
    source = MODEL_RESOURCE.read_text(encoding="utf-8").replace(
        "parameter fluid_density: kg / m ^ 3 = 2;",
        "parameter fluid_density: kg / m ^ 3 = 4;",
    )
    return eqiora.compile(source, filename="foreign-fsi.eqi")


def revised_model(model: eqiora.Model) -> eqiora.Model:
    return model.commit(model.preview_value_edit("fluid_density", 3.0))


def intent(**overrides: object) -> Any:
    arguments = dict(INTENT_ARGUMENTS)
    arguments.update(overrides)
    return eqiora.fsi.FixedMeshMonolithic(**arguments)


def resolve_plan(model: eqiora.Model, **overrides: object) -> Any:
    return eqiora.fsi.resolve(model, intent(**overrides))


def solve() -> tuple[eqiora.Model, Any, eqiora.Result, Any]:
    model = accepted_model()
    plan = resolve_plan(model)
    run = eqiora.submit(model, plan=plan)
    result = run.result()
    return model, plan, result, eqiora.fsi.fixed_mesh_monolithic_evidence(result)


@pytest.fixture(scope="module")
def accepted() -> tuple[eqiora.Model, Any, eqiora.Result, Any]:
    return solve()


def test_intent_is_keyword_only_immutable_and_has_no_hidden_defaults() -> None:
    requested = intent()
    assert isinstance(requested, eqiora.fsi.FixedMeshMonolithic)
    assert type(requested).__module__ == "eqiora._eqiora"
    for name, value in INTENT_ARGUMENTS.items():
        assert getattr(requested, name) == value
        with pytest.raises(AttributeError):
            setattr(requested, name, value)
    assert requested == intent()
    assert hash(requested) == hash(intent())
    assert requested != intent(steps=1)

    with pytest.raises(TypeError):
        eqiora.fsi.FixedMeshMonolithic()
    for omitted in INTENT_ARGUMENTS:
        incomplete = {
            name: value for name, value in INTENT_ARGUMENTS.items() if name != omitted
        }
        with pytest.raises(TypeError):
            eqiora.fsi.FixedMeshMonolithic(**incomplete)
    with pytest.raises(TypeError):
        eqiora.fsi.FixedMeshMonolithic(*INTENT_ARGUMENTS.values())


def test_resolved_plan_is_complete_inspectable_and_stable_before_execution() -> None:
    model = accepted_model()
    plan = resolve_plan(model)
    observed = {name: getattr(plan, name) for name in PLAN_PROPERTIES}

    assert isinstance(plan, eqiora.fsi.FixedMeshMonolithicPlan)
    assert plan.model_digest == model.digest
    assert plan.semantic_revision == model.revision.number == 1
    assert plan.spatial_dimension == 2
    for name, value in INTENT_ARGUMENTS.items():
        assert getattr(plan, name) == value
    for name in (
        "coupling_method",
        "geometry_motion",
        "mesh_kind",
        "fluid_velocity_space",
        "fluid_pressure_space",
        "solid_velocity_space",
        "solid_displacement_space",
        "time_integrator",
        "solver_algorithm",
        "preconditioner",
        "reduction",
        "solver_backend",
        "execution_adapter",
    ):
        assert isinstance(getattr(plan, name), str) and getattr(plan, name)
    assert plan.workers >= 1
    assert isinstance(plan.canonical_bytes, bytes) and plan.canonical_bytes
    for name in (
        "model_digest",
        "geometry_digest",
        "correspondence_digest",
        "mesh_digest",
        "realization_digest",
    ):
        assert len(getattr(plan, name)) == 64

    again = resolve_plan(model)
    assert again == plan
    assert hash(again) == hash(plan)
    assert again.canonical_bytes == plan.canonical_bytes
    result = eqiora.run(model, plan=plan)
    assert type(result) is eqiora.Result
    assert {name: getattr(plan, name) for name in PLAN_PROPERTIES} == observed
    assert result.trajectory.realization_digest == plan.realization_digest
    assert result.run_manifest().realization_digest == plan.realization_digest


def test_result_retains_complete_relational_lineage(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, plan, result, evidence = accepted
    trajectory = result.trajectory
    assert type(result) is eqiora.Result
    assert trajectory.model_digest == model.revision.digest
    assert result.model_revision == model.revision.number == 1
    assert plan.realization_revision == 1
    assert evidence.case_ids == (
        "fsi.fixed-reference-monolithic-step-2d",
        "artifacts.fixed-reference-fsi-spatial-trajectory",
    )
    state_digests = tuple(state.digest for state in trajectory.states)
    assert len(state_digests) == 2
    assert all(len(digest) == 64 for digest in state_digests)
    assert len(trajectory.digest) == 64

    manifest = result.run_manifest()
    assert result.run_manifest() is manifest
    assert evidence.run_digest == manifest.digest == trajectory.run_digest
    assert evidence.trajectory_digest == trajectory.digest
    run = json.loads(manifest.to_json())
    assert run["model_sha256"] == trajectory.model_digest
    assert run["realization_sha256"] == trajectory.realization_digest
    assert run["output_sha256"] == [trajectory.digest]
    assert len(trajectory.run_digest) == 64
    assert all(
        len(digest) == 64
        for digest in (
            trajectory.geometry_digest,
            trajectory.correspondence_digest,
            trajectory.mesh_digest,
            trajectory.realization_digest,
        )
    )


def test_withdrawn_result_accessors_are_absent_without_alias_or_shim(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    _, _, result, _ = accepted
    stub = files(eqiora).joinpath("fsi.pyi").read_text(encoding="utf-8")
    assert len(set(RETIRED_NAMES)) == len(RETIRED_NAMES) == 3
    for name in RETIRED_NAMES:
        assert not hasattr(eqiora.fsi, name)
        with pytest.raises(AttributeError):
            getattr(eqiora.fsi, name)
        assert name not in dir(eqiora.fsi)
        assert name not in eqiora.fsi.__all__
        assert name not in stub
    for name in (
        "fluid_cells",
        "solid_cells",
        "interface_facets",
        "states",
        "case_ids",
        "step",
        "steps",
    ):
        assert not hasattr(result, name)


def test_partition_and_ordered_step_arrays_are_complete_and_immutable(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, _, result, evidence = accepted
    trajectory = result.trajectory
    assert trajectory.coordinates is trajectory.coordinates
    assert trajectory.cells is trajectory.cells
    assert evidence.fluid_cells is evidence.fluid_cells
    assert evidence.solid_cells is evidence.solid_cells
    assert evidence.interface_facets is evidence.interface_facets
    assert trajectory.coordinates.shape == (9, 2)
    assert trajectory.cells.shape == (8, 3)
    np.testing.assert_array_equal(trajectory.cells, EXPECTED_CELLS)
    np.testing.assert_array_equal(evidence.fluid_cells, [0, 1, 2, 3])
    np.testing.assert_array_equal(evidence.solid_cells, [4, 5, 6, 7])
    np.testing.assert_array_equal(evidence.interface_facets, [[1, 3], [3, 5]])

    assert tuple(state.step for state in trajectory.states) == (1, 2)
    assert tuple(state.time_s for state in trajectory.states) == (0.05, 0.10)
    assert tuple(evidence.state(state) for state in trajectory.states) == evidence.states

    velocity = model.field("fluid_velocity")
    pressure = model.field("fluid_pressure")
    displacement = model.field("solid_displacement")
    displacements: list[np.ndarray] = []
    for state in trajectory.states:
        state_evidence = evidence.state(state)
        velocity_snapshot = state.field(velocity)
        pressure_snapshot = state.field(pressure)
        displacement_snapshot = state.field(displacement)
        velocity_vertices = velocity_snapshot.values("vertex")
        bubble_block = velocity_snapshot.values("cell")
        bubble_velocity = bubble_block[evidence.fluid_cells]
        pressure_vertices = pressure_snapshot.support_indices("vertex")
        pressure_block = pressure_snapshot.values("vertex")
        pressure_values = pressure_block[pressure_vertices]
        displacement_values = displacement_snapshot.values("vertex")
        displacements.append(displacement_values)
        arrays = (
            velocity_vertices,
            bubble_block,
            pressure_vertices,
            pressure_block,
            displacement_values,
            state_evidence.interface_vertices,
            state_evidence.fluid_action,
            state_evidence.solid_action,
            state_evidence.action_imbalance,
        )
        assert velocity_snapshot.values("vertex") is velocity_vertices
        assert pressure_snapshot.support_indices("vertex") is pressure_vertices
        assert displacement_snapshot.values("vertex") is displacement_values
        assert state_evidence.interface_vertices is state_evidence.interface_vertices
        assert state_evidence.fluid_action is state_evidence.fluid_action
        assert state_evidence.solid_action is state_evidence.solid_action
        assert state_evidence.action_imbalance is state_evidence.action_imbalance
        assert velocity_vertices.shape == (9, 2)
        assert bubble_block.shape == (8, 2)
        assert bubble_velocity.shape == (4, 2)
        assert pressure_vertices.shape == (6,)
        assert pressure_block.shape == (9,)
        assert pressure_values.shape == (6,)
        assert displacement_values.shape == (9, 2)
        assert state_evidence.interface_vertices.shape == (1,)
        assert state_evidence.fluid_action.shape == (1, 2)
        assert state_evidence.solid_action.shape == (1, 2)
        assert state_evidence.action_imbalance.shape == (1, 2)
        np.testing.assert_array_equal(pressure_vertices, [0, 1, 2, 3, 4, 5])
        np.testing.assert_array_equal(state_evidence.interface_vertices, [3])
        np.testing.assert_array_equal(displacement_values[[0, 2, 4]], 0.0)
        np.testing.assert_array_equal(
            state_evidence.fluid_action + state_evidence.solid_action,
            state_evidence.action_imbalance,
        )
        assert all(array.flags.c_contiguous for array in arrays)
        assert all(not array.flags.writeable for array in arrays)
        assert all(np.isfinite(array).all() for array in arrays)
        assert state_evidence.solve.algorithm == "minimum-residual"
        assert state_evidence.solve.preconditioner == "identity"
        assert state_evidence.solve.reduction == "reproducible"
        assert (
            state_evidence.solve.true_residual_norm
            <= state_evidence.solve.residual_target
        )
        assert state_evidence.assembly_packets > 0
        assert state_evidence.assembly_targets > 0
        assert np.isfinite(
            [
                state_evidence.previous_kinetic_energy_j_per_m,
                state_evidence.next_kinetic_energy_j_per_m,
                state_evidence.previous_elastic_energy_j_per_m,
                state_evidence.next_elastic_energy_j_per_m,
                state_evidence.kinetic_increment_j_per_m,
                state_evidence.elastic_increment_j_per_m,
                state_evidence.viscous_dissipation_j_per_m,
                state_evidence.energy_defect_j_per_m,
                state_evidence.numerical_residual_norm,
                state_evidence.continuity_residual_norm,
                state_evidence.kinematic_residual_norm,
                state_evidence.interface_velocity_jump_norm,
                state_evidence.interface_action_imbalance_n_per_m,
            ]
        ).all()

    assert not np.array_equal(displacements[0], displacements[1])


def test_general_trajectory_projects_exact_replayed_fields(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, _, result, evidence = accepted
    trajectory = result.trajectory
    assert isinstance(trajectory, eqiora.trajectory.Trajectory)
    assert result.trajectory is trajectory
    assert trajectory.model_digest == model.revision.digest
    assert all(
        len(digest) == 64
        for digest in (
            trajectory.digest,
            trajectory.geometry_digest,
            trajectory.correspondence_digest,
            trajectory.mesh_digest,
            trajectory.realization_digest,
            trajectory.run_digest,
        )
    )
    assert trajectory.dimension == 2
    assert trajectory.coordinates is result.trajectory.coordinates
    assert trajectory.cells is result.trajectory.cells
    assert tuple(state.step for state in trajectory.states) == (1, 2)
    assert tuple(state.time_s for state in trajectory.states) == (0.05, 0.10)
    assert tuple(state.digest for state in trajectory.states) == (
        trajectory.state(1).digest,
        trajectory.state(2).digest,
    )
    assert trajectory.state(1) is trajectory.states[0]
    assert trajectory.state(2) is trajectory.states[1]
    with pytest.raises(IndexError):
        trajectory.state(0)

    velocity = model.field("fluid_velocity")
    pressure = model.field("fluid_pressure")
    displacement = model.field("solid_displacement")
    solid_velocity = model.field("solid_velocity")
    expected_fields = (velocity, pressure, displacement, solid_velocity)
    accepted_field_order: tuple[eqiora.FieldRef, ...] | None = None
    for state in trajectory.states:
        state_fields = tuple(snapshot.field for snapshot in state.fields)
        assert set(state_fields) == set(expected_fields)
        assert tuple(field.id for field in state_fields) == tuple(
            sorted(field.id for field in state_fields)
        )
        if accepted_field_order is None:
            accepted_field_order = state_fields
        else:
            assert state_fields == accepted_field_order
        assert state.field(velocity) is next(
            snapshot for snapshot in state.fields if snapshot.field == velocity
        )
        assert state.field(pressure) is next(
            snapshot for snapshot in state.fields if snapshot.field == pressure
        )

        velocity_snapshot = state.field(velocity)
        assert isinstance(velocity_snapshot, eqiora.trajectory.FieldSnapshot)
        assert velocity_snapshot.value_shape == (2,)
        assert velocity_snapshot.dimension == (0, 1, -1, 0, 0, 0, 0)
        assert velocity_snapshot.frame == "spatial-cartesian"
        assert velocity_snapshot.associations == ("vertex", "cell")
        assert tuple(role for role, _ in velocity_snapshot.block_digests) == (
            "vertex",
            "cell",
        )
        assert velocity_snapshot.values("vertex") is velocity_snapshot.values("vertex")
        assert velocity_snapshot.values("vertex").shape == (9, 2)
        velocity_cell_block = velocity_snapshot.values("cell")
        assert velocity_cell_block.shape == (8, 2)
        inactive_velocity_cells = velocity_cell_block[evidence.solid_cells]
        np.testing.assert_array_equal(inactive_velocity_cells, 0.0)
        assert not np.signbit(inactive_velocity_cells).any()

        pressure_snapshot = state.field(pressure)
        assert pressure_snapshot.value_shape == ()
        assert pressure_snapshot.dimension == (1, -1, -2, 0, 0, 0, 0)
        assert pressure_snapshot.frame == "invariant"
        assert pressure_snapshot.associations == ("vertex",)
        pressure_block = pressure_snapshot.values("vertex")
        assert pressure_block.shape == (9,)
        np.testing.assert_array_equal(
            pressure_snapshot.support_indices("vertex"),
            [0, 1, 2, 3, 4, 5],
        )
        inactive_pressure_vertices = pressure_block[[6, 7, 8]]
        np.testing.assert_array_equal(inactive_pressure_vertices, 0.0)
        assert not np.signbit(inactive_pressure_vertices).any()

        displacement_snapshot = state.field(displacement)
        assert displacement_snapshot.values("vertex").shape == (9, 2)
        assert state.field(solid_velocity).associations == ("vertex",)


def test_field_support_indices_expose_frozen_membership_without_disturbing_replay(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, _, result, evidence = accepted
    trajectory = result.trajectory
    trajectory_digest_before = trajectory.digest
    field_names = sorted({name for name, _ in EXPECTED_SUPPORT})
    fields = {name: model.field(name) for name in field_names}
    supports: dict[tuple[int, str, str], np.ndarray] = {}
    for state in trajectory.states:
        snapshots = {name: state.field(field) for name, field in fields.items()}
        values_before = {
            name: snapshot.values(snapshot.associations[0])
            for name, snapshot in snapshots.items()
        }
        for (name, association), expected in EXPECTED_SUPPORT.items():
            snapshot = snapshots[name]
            support = snapshot.support_indices(association)
            assert support is snapshot.support_indices(association)
            assert support.dtype == np.uint32
            assert support.ndim == 1
            np.testing.assert_array_equal(support, expected)
            np.testing.assert_array_equal(support, np.unique(support))
            bound = (
                trajectory.cells if association == "cell" else trajectory.coordinates
            )
            assert int(support.max()) < len(bound)
            assert support.flags.writeable is False
            with pytest.raises(ValueError):
                support.flat[0] = support.flat[0]
            with pytest.raises(ValueError):
                support.setflags(write=True)
            supports[(state.step, name, association)] = support
        for name, snapshot in snapshots.items():
            declared = set(snapshot.associations)
            for absent in ({"vertex", "cell"} - declared) | {"unknown-association"}:
                with pytest.raises(KeyError):
                    snapshot.support_indices(absent)
        for name, snapshot in snapshots.items():
            assert state.field(fields[name]) is snapshot
            association = snapshot.associations[0]
            assert snapshot.values(association) is values_before[name]
        assert snapshots["fluid_velocity"].values("vertex").shape == (9, 2)
        assert snapshots["solid_displacement"].values("vertex").shape == (9, 2)
    assert result.trajectory is trajectory
    assert trajectory.digest == trajectory_digest_before

    for key, association in EXPECTED_SUPPORT:
        np.testing.assert_array_equal(
            supports[(1, key, association)],
            supports[(2, key, association)],
        )
    for state_step in (1, 2):
        np.testing.assert_array_equal(
            supports[(state_step, "fluid_velocity", "vertex")],
            supports[(state_step, "fluid_pressure", "vertex")],
        )
        np.testing.assert_array_equal(
            supports[(state_step, "solid_displacement", "vertex")],
            supports[(state_step, "solid_velocity", "vertex")],
        )

    fluid_vertices = supports[(1, "fluid_velocity", "vertex")]
    solid_vertices = supports[(1, "solid_displacement", "vertex")]
    np.testing.assert_array_equal(
        np.intersect1d(fluid_vertices, solid_vertices),
        [1, 3, 5],
    )
    np.testing.assert_array_equal(
        np.union1d(fluid_vertices, solid_vertices),
        [0, 1, 2, 3, 4, 5, 6, 7, 8],
    )

    np.testing.assert_array_equal(
        supports[(1, "fluid_pressure", "vertex")],
        trajectory.state(1)
        .field(model.field("fluid_pressure"))
        .support_indices("vertex"),
    )
    np.testing.assert_array_equal(
        supports[(1, "fluid_velocity", "cell")],
        evidence.fluid_cells,
    )
    np.testing.assert_array_equal(
        np.unique(evidence.interface_facets),
        np.intersect1d(fluid_vertices, solid_vertices),
    )


def test_general_trajectory_rejects_foreign_fields_and_mutation(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, _, result, _ = accepted
    state = result.trajectory.state(1)
    with pytest.raises(KeyError):
        state.field(model.field("fluid_load_potential"))

    source = MODEL_RESOURCE.read_text(encoding="utf-8").replace(
        "model Main {",
        "model IndependentMain {",
    )
    independent = eqiora.compile(source, filename="independent-fixed-reference-fsi.eqi")
    assert model.structurally_equivalent(independent)
    assert model.digest != independent.digest
    with pytest.raises(ValueError, match="different exact Model"):
        state.field(independent.field("fluid_velocity"))

    arrays = (
        result.trajectory.coordinates,
        result.trajectory.cells,
        state.field(model.field("fluid_velocity")).values("vertex"),
        state.field(model.field("fluid_velocity")).values("cell"),
    )
    for array in arrays:
        assert array.flags.writeable is False
        with pytest.raises(ValueError):
            array.flat[0] = array.flat[0]
        with pytest.raises(ValueError):
            array.setflags(write=True)
        assert np.asarray(array).view().flags.writeable is False
    with pytest.raises(KeyError):
        state.field(model.field("fluid_pressure")).values("cell")


def test_independent_compilations_share_meaning_without_sharing_storage() -> None:
    first_model = accepted_model()
    second_model = accepted_model()
    assert first_model is not second_model
    assert first_model.structurally_equivalent(second_model)
    assert first_model.structural_fingerprint == second_model.structural_fingerprint

    first = eqiora.run(first_model, plan=resolve_plan(first_model))
    second = eqiora.run(second_model, plan=resolve_plan(second_model))
    assert first is not second
    for name, association in (
        ("fluid_velocity", "vertex"),
        ("fluid_pressure", "vertex"),
        ("solid_displacement", "vertex"),
    ):
        for ordinal in (1, 2):
            left = first.trajectory.state(ordinal).field(first_model.field(name))
            right = second.trajectory.state(ordinal).field(second_model.field(name))
            left_values = left.values(association)
            right_values = right.values(association)
            np.testing.assert_array_equal(left_values, right_values)
            assert not np.shares_memory(left_values, right_values)


def test_array_owners_survive_result_and_step_deletion() -> None:
    model = accepted_model()
    result = eqiora.run(model, plan=resolve_plan(model))
    trajectory = result.trajectory
    state = trajectory.state(2)
    evidence = eqiora.fsi.fixed_mesh_monolithic_evidence(result)
    state_evidence = evidence.state(state)
    arrays = (
        trajectory.coordinates,
        trajectory.cells,
        state.field(model.field("fluid_velocity")).values("vertex"),
        state.field(model.field("fluid_pressure")).values("vertex"),
        state.field(model.field("solid_displacement")).values("vertex"),
        state_evidence.fluid_action,
    )
    del state_evidence
    del evidence
    del state
    del trajectory
    del result
    del model
    gc.collect()
    assert all(array.size > 0 and not array.flags.writeable for array in arrays)


def test_foreign_current_meaning_is_rejected_before_execution() -> None:
    foreign = foreign_model()
    with pytest.raises(eqiora.ValidationError) as caught:
        resolve_plan(foreign)
    assert any(diagnostic.code == "EQ0807" for diagnostic in caught.value.diagnostics)


def test_plan_is_bound_to_one_exact_model_before_worker_creation(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, plan, _, _ = accepted
    for foreign in (accepted_model(), revised_model(model), foreign_model()):
        assert foreign.digest != model.digest
        for entry_point in (eqiora.submit, eqiora.run):
            with pytest.raises(eqiora.ValidationError) as caught:
                entry_point(foreign, plan=plan)
            assert any(
                diagnostic.code == "EQ0807"
                for diagnostic in caught.value.diagnostics
            )


def test_run_result_and_occurrence_projections_are_memoized(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, plan, _, _ = accepted
    run = eqiora.submit(model, plan=plan)
    result = run.result()
    assert run.result() is result
    assert result.trajectory is result.trajectory
    evidence = eqiora.fsi.fixed_mesh_monolithic_evidence(result)
    assert eqiora.fsi.fixed_mesh_monolithic_evidence(result) is evidence
    for state in result.trajectory.states:
        assert evidence.state(state) is evidence.state(state)


def test_trajectory_and_fsi_evidence_reject_unrelated_common_results() -> None:
    state = eqiora.Field("x", initial=1.0)
    model = eqiora.Model.define(
        "hold",
        state,
        eqiora.Relation("hold", residual=eqiora.derivative(state)),
    )
    unrelated = eqiora.run(model, end_time=0.1, max_step=0.1)
    with pytest.raises(eqiora.CapabilityError):
        _ = unrelated.trajectory
    with pytest.raises(eqiora.CapabilityError):
        eqiora.fsi.fixed_mesh_monolithic_evidence(unrelated)
    for wrong_type in (object(), unrelated.run_manifest()):
        with pytest.raises(TypeError):
            eqiora.fsi.fixed_mesh_monolithic_evidence(wrong_type)


def test_evidence_state_lookup_is_bound_to_exact_occurrence_and_state(
    accepted: tuple[eqiora.Model, Any, eqiora.Result, Any],
) -> None:
    model, plan, result, evidence = accepted
    states = result.trajectory.states
    assert tuple(item.state_digest for item in evidence.states) == tuple(
        state.digest for state in states
    )
    assert tuple(evidence.state(state) for state in states) == evidence.states

    repeated = eqiora.run(model, plan=plan)
    for foreign_state in repeated.trajectory.states:
        with pytest.raises(ValueError, match="different exact|occurrence|trajectory"):
            evidence.state(foreign_state)
    other_model = accepted_model()
    other = eqiora.run(other_model, plan=resolve_plan(other_model))
    with pytest.raises(ValueError, match="different exact|occurrence|trajectory"):
        evidence.state(other.trajectory.state(1))
    for wrong_type in (1, object(), result.trajectory):
        with pytest.raises(TypeError):
            evidence.state(wrong_type)


def test_unsupported_intent_values_reject_during_resolution() -> None:
    model = accepted_model()
    for unsupported in (
        {"time_step_s": 0.1},
        {"steps": 1},
        {"initial_velocity_m_per_s": (1.0, 0.0)},
        {"initial_free_interface_displacement_m": (0.01, 0.0)},
        {"length_scale_m": 1.0},
        {"velocity_scale_m_per_s": 1.0},
        {"pressure_scale_pa": 2.0},
        {"relative_tolerance": 1.0e-10},
        {"absolute_tolerance": 1.0e-12},
        {"maximum_iterations": 10_000},
    ):
        with pytest.raises(eqiora.CapabilityError):
            resolve_plan(model, **unsupported)


def test_numpy_import_is_lazy_until_projection(tmp_path: Path) -> None:
    script = tmp_path / "lazy_fsi_projection.py"
    script.write_text(
        """
import sys
from importlib.resources import files
import eqiora

assert "numpy" not in sys.modules
source = files(eqiora).joinpath("examples", "fixed-reference-fsi.eqi").read_text()
model = eqiora.compile(
    source,
    filename="fixed-reference-fsi.eqi",
)
intent = eqiora.fsi.FixedMeshMonolithic(
    time_step_s=0.05,
    steps=2,
    initial_velocity_m_per_s=(0.0, 0.0),
    initial_free_interface_displacement_m=(0.02, 0.0),
    length_scale_m=2.0,
    velocity_scale_m_per_s=0.5,
    pressure_scale_pa=4.0,
    relative_tolerance=1.0e-11,
    absolute_tolerance=1.0e-13,
    maximum_iterations=20000,
)
plan = eqiora.fsi.resolve(model, intent)
result = eqiora.submit(model, plan=plan).result()
assert "numpy" not in sys.modules
trajectory = result.trajectory
assert "numpy" not in sys.modules
_ = trajectory.coordinates
assert "numpy" in sys.modules
""",
        encoding="utf-8",
    )
    completed = subprocess.run(
        [sys.executable, "-I", str(script)],
        cwd=tmp_path,
        check=False,
        capture_output=True,
        text=True,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_checked_in_python_demo_runs_with_packaged_model_resource() -> None:
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
    assert all(len(line) == 64 for line in lines[:2])
    assert lines[2].startswith("step 1 at 0.05 s LinearSolveSummary(")
    assert lines[3].startswith("step 2 at 0.1 s LinearSolveSummary(")
