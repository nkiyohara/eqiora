from __future__ import annotations

import math
import asyncio
import gc

import numpy as np
import pytest

import eqiora


DECAY = """
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"""


def resolve_decay(model: eqiora.Model) -> tuple[eqiora.Plan, eqiora.FieldRef]:
    field = model.field(model.field_ids[0])
    plan = eqiora.resolve(
        model,
        temporal=eqiora.time.Tsitouras45(
            initial_step_s=0.01,
            relative_tolerance=1.0e-9,
            absolute_tolerances={field: 1.0e-11},
        ),
    )
    return plan, field


def test_model_first_no_mesh_decay_owns_exact_lineage_and_adaptive_series() -> None:
    model = eqiora.compile(source=DECAY, filename="decay.eqi")
    plan, field = resolve_decay(model)

    assert plan.model is model
    assert plan.model_digest == model.digest
    assert plan.fields == (field,)
    assert plan.mesh is None
    assert plan.mesh_digest is None
    assert plan.spatial is None
    assert isinstance(plan.capability, eqiora.time.OdePlanView)
    assert not hasattr(plan.capability, "scaling")
    assert plan.solve is None
    assert plan.execution.placement == "host-serial"
    assert plan.capability.backend == "eqiora.time.diffsol"
    assert plan.capability.backend_version == "0.16.2"

    initial = eqiora.State.initial(plan)
    assert initial.model is model
    assert initial.mesh is None
    assert initial.time_s == 0.0
    assert initial.field_refs == (field,)
    assert initial.value(field) == 1.0
    assert initial.source_kind == "initial"
    initial_bytes = initial.to_bytes()
    replayed_initial = eqiora.State.from_bytes(plan, initial_bytes)
    assert replayed_initial == initial
    assert replayed_initial.to_bytes() == initial_bytes
    assert replayed_initial.source_kind == "artifact"
    with pytest.raises(eqiora.ValidationError):
        eqiora.State.from_bytes(plan, initial_bytes + b"\n")

    result = eqiora.run(
        plan,
        state=initial,
        until_s=0.2,
        output_times_s=(0.1, 0.2),
    )
    series = result.series(field)
    assert series.field == field
    np.testing.assert_array_equal(series.time.numpy(), np.array([0.1, 0.2]))
    np.testing.assert_allclose(
        series.values.numpy(),
        np.array([math.exp(-0.1), math.exp(-0.2)]),
        rtol=2.0e-8,
        atol=2.0e-10,
    )
    with pytest.raises(TypeError):
        result.series("x")  # type: ignore[arg-type]


def test_replay_and_fresh_compile_use_the_same_resolver_without_fixed_identity() -> None:
    first = eqiora.compile(source=DECAY)
    replayed = eqiora.Model.from_bytes(first.to_bytes())
    second = eqiora.compile(source=DECAY)
    first_plan, _ = resolve_decay(first)
    replayed_plan, _ = resolve_decay(replayed)
    second_plan, _ = resolve_decay(second)

    assert replayed_plan.identity == first_plan.identity
    assert replayed_plan.model_digest == replayed.digest == first.digest
    assert second_plan.model_digest == second.digest
    assert second_plan.model_digest != first_plan.model_digest


def test_restart_is_a_new_adaptive_run_and_step_controls_are_rejected() -> None:
    model = eqiora.compile(source=DECAY)
    plan, field = resolve_decay(model)
    first = eqiora.run(
        plan,
        state=eqiora.State.initial(plan),
        until_s=0.1,
        output_times_s=(0.1,),
    )
    restart = eqiora.State.from_result(plan, first, time_s=0.1)
    second = eqiora.run(
        plan,
        state=restart,
        until_s=0.2,
        output_times_s=(0.2,),
    )
    assert second.series(field).values.numpy()[0] == pytest.approx(
        math.exp(-0.2), rel=2.0e-8, abs=2.0e-10
    )

    with pytest.raises(TypeError, match="steps/output_steps"):
        eqiora.submit(
            plan,
            state=restart,
            steps=1,
            output_steps=(1,),
        )


def test_async_run_and_array_ownership_use_the_same_common_result() -> None:
    async def exercise() -> tuple[eqiora.Result, np.ndarray]:
        model = eqiora.compile(source=DECAY)
        plan, field = resolve_decay(model)
        submitted = eqiora.submit(
            plan,
            state=eqiora.State.initial(plan),
            until_s=0.2,
            output_times_s=(0.1, 0.2),
        )
        assert submitted.cancel() is False
        result = await submitted
        assert result is submitted.result()
        values = result.series(field).values.numpy(copy=False)
        assert not values.flags.writeable
        return result, values

    result, retained = asyncio.run(exercise())
    del result
    gc.collect()
    np.testing.assert_allclose(
        retained,
        np.array([math.exp(-0.1), math.exp(-0.2)]),
        rtol=2.0e-8,
        atol=2.0e-10,
    )
