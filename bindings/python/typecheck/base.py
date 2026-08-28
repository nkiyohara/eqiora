from typing import assert_type

import numpy as np
import numpy.typing as npt

import eqiora
from eqiora.fsi import (
    FsiEvidence,
    FsiStateEvidence,
)
from eqiora.meshing import Mesh
from eqiora.solid import LinearElasticityEvidence
from eqiora.trajectory import FieldSnapshot, Trajectory, State


def check_language_source() -> None:
    source = eqiora.lang.Source()
    component = source.component("Poisson")
    volume = component.volume("volume", dimensions=2)
    value = component.field("value", on=volume, unit=eqiora.lang.units.m)
    component.relation("balance", on=volume, residual=eqiora.lang.div(value))
    assert_type(source.to_eqi(), str)
    assert_type(eqiora.compile(source=source), eqiora.Model)


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
    plan: eqiora.Plan,
    state: eqiora.State,
    array: eqiora.Array,
) -> None:
    assert_type(eqiora.run(plan), eqiora.Result)
    transient = eqiora.submit(
        plan, state=state, until_s=1.0, output_times_s=(1.0,)
    )
    assert_type(transient, eqiora.Run[eqiora.Result])
    assert_type(transient.result(), eqiora.Result)
    assert_type(array.numpy(copy=False), npt.NDArray[np.float64])

    eqiora.Run[eqiora.Result](object())  # type: ignore[arg-type]


class _InvalidModelSubclass(eqiora.Model):  # type: ignore[misc]
    pass


def check_structural_result(plan: eqiora.Plan, result: eqiora.Result) -> None:
    assert_type(plan, eqiora.Plan)
    assert_type(plan.mesh_kind, str | None)
    assert_type(plan.space, str | None)
    assert_type(
        eqiora.solid.linear_elasticity_evidence(result),
        LinearElasticityEvidence,
    )


def check_fsi_result(plan: eqiora.Plan, state: eqiora.State) -> None:
    assert_type(plan, eqiora.Plan)
    assert_type(plan.mesh_kind, str | None)
    assert_type(plan.velocity_space, str | None)
    assert_type(plan.pressure_space, str | None)
    assert_type(plan.temporal, eqiora.time.BackwardEuler | eqiora.time.Tsitouras45 | None)
    assert_type(
        eqiora.submit(plan, state=state, steps=2, output_steps=(1, 2)),
        eqiora.Run[eqiora.Result],
    )
    result = eqiora.run(plan, state=state, steps=2, output_steps=(1, 2))
    assert_type(result, eqiora.Result)
    assert_type(result.trajectory, Trajectory)
    assert_type(result.trajectory.coordinates, npt.NDArray[np.float64])
    assert_type(result.trajectory.cells, npt.NDArray[np.uint32])
    assert_type(result.trajectory.states, tuple[State, ...])
    assert_type(result.trajectory.state(1).fields, tuple[FieldSnapshot, ...])
    assert_type(
        result.trajectory.state(1).field(plan.fields[0]),
        FieldSnapshot,
    )
    assert_type(
        result.trajectory.state(1)
        .field(plan.fields[0])
        .support_indices("vertex"),
        npt.NDArray[np.uint32],
    )
    evidence = eqiora.fsi.evidence(result)
    assert_type(evidence, FsiEvidence)
    assert_type(
        evidence.state(result.trajectory.state(1)),
        FsiStateEvidence,
    )
    assert_type(evidence.fluid_cells, npt.NDArray[np.uint32])
    assert_type(evidence.solid_cells, npt.NDArray[np.uint32])
    assert_type(evidence.interface_facets, npt.NDArray[np.uint32])
    assert_type(
        evidence.states,
        tuple[FsiStateEvidence, ...],
    )
    assert_type(
        evidence.state(result.trajectory.state(1)).fluid_action,
        npt.NDArray[np.float64],
    )
