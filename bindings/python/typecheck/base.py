from typing import assert_type

import numpy as np
import numpy.typing as npt

import eqiora
from eqiora.fsi import FixedReferenceFsiResult, FixedReferenceFsiStep
from eqiora.meshing import Mesh
from eqiora.solid import (
    LinearElasticity,
    LinearElasticityEvidence,
    LinearElasticityPlan,
)
from eqiora.trajectory import FieldSnapshot, Trajectory, TrajectoryState


def check_native_modeling() -> None:
    length = eqiora.Dimension(length=1)
    domain = eqiora.Domain.box("rod", (0.0, 1.0))
    continuum = eqiora.Representation.continuum("temperature")
    temperature = eqiora.Field(
        "temperature",
        domain=domain,
        representation=continuum,
        dimension=length,
    )
    conductivity = eqiora.Parameter("conductivity", value=1.0)
    balance = eqiora.Relation(
        "balance",
        domain=domain,
        residual=eqiora.div(conductivity * eqiora.grad(temperature)),
    )

    assert_type(
        eqiora.Model.define(
            "thermal",
            domain,
            continuum,
            temperature,
            conductivity,
            balance,
        ),
        eqiora.Model,
    )


def check_execution(
    model: eqiora.Model,
    realization: eqiora.Realization,
    array: eqiora.Array,
) -> None:
    assert_type(
        eqiora.run(model, end_time=1.0, max_step=0.1),
        eqiora.Result,
    )
    assert_type(
        eqiora.run(model, end_time=1.0, max_step=0.1, realization=None),
        eqiora.Result,
    )
    assert_type(
        eqiora.run(model, realization=realization),
        eqiora.ScalarEllipticResult,
    )

    temporal = eqiora.submit(model, end_time=1.0, max_step=0.1)
    spatial = eqiora.submit(model, realization=realization)
    assert_type(temporal, eqiora.Run[eqiora.Result])
    assert_type(spatial, eqiora.Run[eqiora.ScalarEllipticResult])
    assert_type(temporal.result(), eqiora.Result)
    assert_type(spatial.result(), eqiora.ScalarEllipticResult)
    assert_type(array.numpy(copy=False), npt.NDArray[np.float64])

    eqiora.run(model)  # type: ignore[call-overload]
    eqiora.run(  # type: ignore[call-overload]
        model,
        end_time=1.0,
        max_step=0.1,
        realization=realization,
    )
    eqiora.Run[eqiora.Result](object())  # type: ignore[arg-type]


class _InvalidModelSubclass(eqiora.Model):  # type: ignore[misc]
    pass


def check_structural_result(model: eqiora.Model) -> None:
    # Accepted reference tuple; its native authority is
    # `crates/eqiora-api/src/elasticity.rs::require_supported_intent`.
    intent = LinearElasticity(
        cells_per_axis=16,
        relative_tolerance=1.0e-12,
        absolute_tolerance=1.0e-14,
        maximum_iterations=10_000,
    )
    plan = eqiora.solid.resolve(model, intent)
    assert_type(plan, LinearElasticityPlan)
    assert_type(plan.discretization_method, str)
    assert_type(plan.mesh_kind, str)
    assert_type(plan.mesh_policy, str)
    assert_type(plan.field_space, str)
    assert_type(plan.quadrature, str)
    assert_type(plan.quadrature_points_per_axis, int)
    assert_type(plan.scalar_type, str)
    assert_type(plan.vector_layout, str)
    assert_type(plan.coefficient_association, str)
    assert_type(eqiora.submit(model, plan=plan), eqiora.Run[eqiora.Result])
    result = eqiora.run(model, plan=plan)
    assert_type(result, eqiora.Result)

    displacement = model.field("displacement")
    snapshot = result.field(displacement)
    mesh = result.mesh(displacement)
    assert_type(snapshot, FieldSnapshot)
    assert_type(mesh, Mesh)
    assert_type(snapshot.values("vertex"), npt.NDArray[np.float64])
    assert_type(snapshot.support_indices("vertex"), npt.NDArray[np.uint32])
    assert_type(mesh.coordinates, npt.NDArray[np.float64])
    assert_type(mesh.cells, npt.NDArray[np.uint32])
    assert_type(
        eqiora.solid.linear_elasticity_evidence(result),
        LinearElasticityEvidence,
    )

    LinearElasticity(cells_per_axis=16)  # type: ignore[call-arg]


def check_fsi_result(model: eqiora.Model) -> None:
    result = eqiora.fsi.solve_fixed_reference_fsi(model)
    assert_type(result, FixedReferenceFsiResult)
    assert_type(result.trajectory, Trajectory)
    assert_type(result.trajectory.coordinates, npt.NDArray[np.float64])
    assert_type(result.trajectory.cells, npt.NDArray[np.uint32])
    assert_type(result.trajectory.states, tuple[TrajectoryState, ...])
    assert_type(result.trajectory.state(1).fields, tuple[FieldSnapshot, ...])
    assert_type(
        result.trajectory.state(1).field(model.field(model.field_ids[0])),
        FieldSnapshot,
    )
    assert_type(
        result.trajectory.state(1)
        .field(model.field(model.field_ids[0]))
        .support_indices("vertex"),
        npt.NDArray[np.uint32],
    )
    assert_type(result.steps, tuple[FixedReferenceFsiStep, FixedReferenceFsiStep])
    assert_type(result.step(1).pressure, npt.NDArray[np.float64])
