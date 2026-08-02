from __future__ import annotations

import gc
import hashlib
import json
import subprocess
import sys
from importlib.resources import files
from pathlib import Path

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
# Names withdrawn from FixedReferenceFsiResult, whose sole installed-Python
# owner is now `result.trajectory`. Absence is the claim: no alias, no shim.
WITHDRAWN_RESULT_ACCESSORS = (
    "model_digest",
    "geometry_digest",
    "correspondence_digest",
    "mesh_digest",
    "realization_digest",
    "run_digest",
    "trajectory_digest",
    "state_digests",
    "coordinates",
    "cells",
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


@pytest.fixture(scope="module")
def accepted() -> tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult]:
    model = accepted_model()
    return model, eqiora.fsi.solve_fixed_reference_fsi(model)


def test_result_retains_complete_relational_lineage(
    accepted: tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult],
) -> None:
    model, result = accepted
    trajectory = result.trajectory
    assert isinstance(result, eqiora.fsi.FixedReferenceFsiResult)
    assert trajectory.model_digest == model.revision.digest
    assert result.semantic_revision == model.revision.number == 1
    assert result.realization_revision == 1
    assert result.case_ids == (
        "fsi.fixed-reference-monolithic-step-2d",
        "artifacts.fixed-reference-fsi-spatial-trajectory",
    )
    state_digests = tuple(state.digest for state in trajectory.states)
    assert len(state_digests) == 2
    assert all(len(digest) == 64 for digest in state_digests)
    assert len(trajectory.digest) == 64

    run = json.loads(result.run_manifest_json)
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
    accepted: tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult],
) -> None:
    _, result = accepted
    assert len(set(WITHDRAWN_RESULT_ACCESSORS)) == len(WITHDRAWN_RESULT_ACCESSORS) == 10
    for name in WITHDRAWN_RESULT_ACCESSORS:
        assert hasattr(result, name) is False
        with pytest.raises(AttributeError):
            getattr(result, name)
        assert not hasattr(eqiora.fsi.FixedReferenceFsiResult, name)
        assert name not in dir(result)


def test_partition_and_ordered_step_arrays_are_complete_and_immutable(
    accepted: tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult],
) -> None:
    _, result = accepted
    trajectory = result.trajectory
    assert trajectory.coordinates is trajectory.coordinates
    assert trajectory.cells is trajectory.cells
    assert result.fluid_cells is result.fluid_cells
    assert result.solid_cells is result.solid_cells
    assert result.interface_facets is result.interface_facets
    assert trajectory.coordinates.shape == (9, 2)
    assert trajectory.cells.shape == (8, 3)
    np.testing.assert_array_equal(trajectory.cells, EXPECTED_CELLS)
    np.testing.assert_array_equal(result.fluid_cells, [0, 1, 2, 3])
    np.testing.assert_array_equal(result.solid_cells, [4, 5, 6, 7])
    np.testing.assert_array_equal(result.interface_facets, [[1, 3], [3, 5]])

    assert tuple(step.ordinal for step in result.steps) == (1, 2)
    assert tuple(step.time_s for step in result.steps) == (0.05, 0.10)
    assert result.step(1) is result.steps[0]
    assert result.step(2) is result.steps[1]
    with pytest.raises(IndexError):
        result.step(0)
    with pytest.raises(IndexError):
        result.step(3)

    for step in result.steps:
        arrays = (
            step.velocity,
            step.bubble_velocity,
            step.pressure_vertices,
            step.pressure,
            step.displacement,
            step.interface_vertices,
            step.fluid_action,
            step.solid_action,
            step.action_imbalance,
        )
        assert step.velocity is step.velocity
        assert step.bubble_velocity is step.bubble_velocity
        assert step.pressure_vertices is step.pressure_vertices
        assert step.pressure is step.pressure
        assert step.displacement is step.displacement
        assert step.interface_vertices is step.interface_vertices
        assert step.fluid_action is step.fluid_action
        assert step.solid_action is step.solid_action
        assert step.action_imbalance is step.action_imbalance
        assert step.velocity.shape == (9, 2)
        assert step.bubble_velocity.shape == (4, 2)
        assert step.pressure_vertices.shape == (6,)
        assert step.pressure.shape == (6,)
        assert step.displacement.shape == (9, 2)
        assert step.interface_vertices.shape == (1,)
        assert step.fluid_action.shape == (1, 2)
        assert step.solid_action.shape == (1, 2)
        assert step.action_imbalance.shape == (1, 2)
        np.testing.assert_array_equal(step.pressure_vertices, [0, 1, 2, 3, 4, 5])
        np.testing.assert_array_equal(step.interface_vertices, [3])
        np.testing.assert_array_equal(step.displacement[[0, 2, 4]], 0.0)
        np.testing.assert_array_equal(
            step.fluid_action + step.solid_action,
            step.action_imbalance,
        )
        assert all(array.flags.c_contiguous for array in arrays)
        assert all(not array.flags.writeable for array in arrays)
        assert all(np.isfinite(array).all() for array in arrays)
        assert step.solve.algorithm == "minimum-residual"
        assert step.solve.preconditioner == "identity"
        assert step.solve.reduction == "reproducible"
        assert step.solve.true_residual_norm <= step.solve.residual_target
        assert step.assembly_packets > 0
        assert step.assembly_targets > 0
        assert np.isfinite(
            [
                step.energy_defect_j_per_m,
                step.numerical_residual_norm,
                step.continuity_residual_norm,
                step.kinematic_residual_norm,
                step.interface_velocity_jump_norm,
                step.interface_action_imbalance_n_per_m,
            ]
        ).all()

    assert not np.array_equal(
        result.steps[0].displacement,
        result.steps[1].displacement,
    )


def test_general_trajectory_projects_exact_replayed_fields(
    accepted: tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult],
) -> None:
    model, result = accepted
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
    for state, step in zip(trajectory.states, result.steps, strict=True):
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
        np.testing.assert_array_equal(
            velocity_snapshot.values("vertex"),
            step.velocity,
        )
        velocity_cell_block = velocity_snapshot.values("cell")
        assert velocity_cell_block.shape == (8, 2)
        np.testing.assert_array_equal(
            velocity_cell_block[result.fluid_cells],
            step.bubble_velocity,
        )
        inactive_velocity_cells = velocity_cell_block[result.solid_cells]
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
            pressure_block[step.pressure_vertices],
            step.pressure,
        )
        inactive_pressure_vertices = pressure_block[[6, 7, 8]]
        np.testing.assert_array_equal(inactive_pressure_vertices, 0.0)
        assert not np.signbit(inactive_pressure_vertices).any()

        displacement_snapshot = state.field(displacement)
        np.testing.assert_array_equal(
            displacement_snapshot.values("vertex"),
            step.displacement,
        )
        assert state.field(solid_velocity).associations == ("vertex",)


def test_field_support_indices_expose_frozen_membership_without_disturbing_replay(
    accepted: tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult],
) -> None:
    model, result = accepted
    trajectory = result.trajectory
    trajectory_digest_before = trajectory.digest
    field_names = sorted({name for name, _ in EXPECTED_SUPPORT})
    fields = {name: model.field(name) for name in field_names}
    supports: dict[tuple[int, str, str], np.ndarray] = {}
    for state, step in zip(trajectory.states, result.steps, strict=True):
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
        np.testing.assert_array_equal(
            snapshots["fluid_velocity"].values("vertex"),
            step.velocity,
        )
        np.testing.assert_array_equal(
            snapshots["solid_displacement"].values("vertex"),
            step.displacement,
        )
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
        result.steps[0].pressure_vertices,
    )
    np.testing.assert_array_equal(
        supports[(1, "fluid_velocity", "cell")],
        result.fluid_cells,
    )
    np.testing.assert_array_equal(
        np.unique(result.interface_facets),
        np.intersect1d(fluid_vertices, solid_vertices),
    )


def test_general_trajectory_rejects_foreign_fields_and_mutation(
    accepted: tuple[eqiora.Model, eqiora.fsi.FixedReferenceFsiResult],
) -> None:
    model, result = accepted
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

    first = eqiora.fsi.solve_fixed_reference_fsi(first_model)
    second = eqiora.fsi.solve_fixed_reference_fsi(second_model)
    assert first is not second
    for left, right in zip(first.steps, second.steps, strict=True):
        np.testing.assert_array_equal(left.velocity, right.velocity)
        np.testing.assert_array_equal(left.pressure, right.pressure)
        np.testing.assert_array_equal(left.displacement, right.displacement)
        assert not np.shares_memory(left.velocity, right.velocity)
        assert not np.shares_memory(left.pressure, right.pressure)


def test_array_owners_survive_result_and_step_deletion() -> None:
    result = eqiora.fsi.solve_fixed_reference_fsi(accepted_model())
    trajectory = result.trajectory
    step = result.step(2)
    arrays = (
        trajectory.coordinates,
        trajectory.cells,
        step.velocity,
        step.pressure,
        step.displacement,
    )
    del step
    del trajectory
    del result
    gc.collect()
    assert all(array.size > 0 and not array.flags.writeable for array in arrays)


def test_foreign_current_meaning_is_rejected_before_execution() -> None:
    source = MODEL_RESOURCE.read_text(encoding="utf-8").replace(
        "parameter fluid_density: kg / m ^ 3 = 2;",
        "parameter fluid_density: kg / m ^ 3 = 4;",
    )
    foreign = eqiora.compile(
        source,
        filename="foreign-fsi.eqi",
    )
    with pytest.raises(eqiora.ValidationError) as caught:
        eqiora.fsi.solve_fixed_reference_fsi(foreign)
    assert any(diagnostic.code == "EQ0807" for diagnostic in caught.value.diagnostics)


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
result = eqiora.fsi.solve_fixed_reference_fsi(model)
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
