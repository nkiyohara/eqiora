from __future__ import annotations

import asyncio
import gc
import json
import sys
import threading
from itertools import product

import numpy as np
import pytest

import eqiora


POISSON = """
model native_poisson {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  representation scalar_space = continuum;

  field potential on interval as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 1;

  relation balance continuous on interval {
    -div(grad(potential)) - source_scale = 0;
  }
  relation lower_value continuous on lower_end { trace(potential) = 0; }
  relation upper_value continuous on upper_end { trace(potential) = 0; }
}
"""


def affine_poisson(dimension: int) -> str:
    axes = ("x", "y", "z")[:dimension]
    bounds = ", ".join(["0, 1"] * dimension)
    exact = " + ".join(
        f"{axis + 1} * inverse_length * coordinate({axis})"
        for axis in range(dimension)
    )
    domains: list[str] = []
    relations: list[str] = []
    for axis, name in enumerate(axes):
        for side in ("lower", "upper"):
            domains.append(
                f"  domain {name}_{side} = boundary(region, axis = {axis}, side = {side});"
            )
            relations.append(
                f"  relation {name}_{side}_value continuous on {name}_{side} "
                f"{{ trace(potential) - ({exact}) = 0; }}"
            )
    return "\n".join(
        [
            f"model affine_{dimension}d {{",
            f"  domain region = box({bounds});",
            *domains,
            "  representation scalar_space = continuum;",
            "  field potential on region as scalar_space: 1 = 0;",
            "  parameter inverse_length: 1 / m = 1;",
            "  parameter source_scale: 1 / m ^ 2 = 0;",
            "  relation balance continuous on region {",
            "    -div(grad(potential)) - source_scale = 0;",
            "  }",
            *relations,
            "}",
        ]
    )


def request(
    method: eqiora.ScalarEllipticMethod = eqiora.ScalarEllipticMethod.FiniteElement,
) -> eqiora.ScalarElliptic:
    return eqiora.ScalarElliptic(
        method=method,
        cells_per_axis=8,
        realization_revision=1,
    )


def test_preview_returns_deterministic_opaque_model_bound_realization() -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    selected = request()
    same_request = request()

    assert selected == same_request
    assert hash(selected) == hash(same_request)

    first = eqiora.preview_realization(model, selected)
    second = eqiora.preview_realization(model, selected)
    finite_volume = eqiora.preview_realization(
        model,
        request(eqiora.ScalarEllipticMethod.FiniteVolume),
    )

    assert first == second
    assert hash(first) == hash(second)
    assert first.to_json() == second.to_json()
    assert len(first.digest) == 64
    assert first.model_digest == model.digest
    assert first.method == eqiora.ScalarEllipticMethod.FiniteElement
    assert first.cells_per_axis == selected.cells_per_axis == 8
    assert first.realization_revision == selected.realization_revision == 1
    assert first.workers == 1
    assert first.cell_count == 8
    assert first.field_value_count == 9
    assert first.spatial_dimension == 1
    assert first.field_logical_shape == (9,)
    assert finite_volume != first
    assert finite_volume.field_value_count == 8
    assert json.loads(first.to_json())["schema"] == "eqiora.realization-envelope/v1"

    with pytest.raises(TypeError):
        eqiora.Realization()
    with pytest.raises(TypeError):
        eqiora.ScalarElliptic(
            method=eqiora.ScalarEllipticMethod.FiniteElement,
            cells_per_axis=8,
            workers=2,
        )


@pytest.mark.parametrize(
    ("method", "location", "value_count"),
    [
        (
            eqiora.ScalarEllipticMethod.FiniteElement,
            eqiora.ScalarFieldLocation.Vertex,
            9,
        ),
        (
            eqiora.ScalarEllipticMethod.FiniteVolume,
            eqiora.ScalarFieldLocation.CellCenter,
            8,
        ),
    ],
)
def test_run_replays_exact_realization_and_publishes_bounded_evidence(
    method: eqiora.ScalarEllipticMethod,
    location: eqiora.ScalarFieldLocation,
    value_count: int,
) -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    accepted = eqiora.preview_realization(model, request(method))

    result = eqiora.run(model, realization=accepted)

    assert isinstance(result, eqiora.ScalarEllipticResult)
    assert result.realization == accepted
    assert result.elapsed_seconds >= 0.0
    assert result.field.location == location
    assert result.field.value_count == value_count
    assert result.field.value_count == result.realization.field_value_count
    assert result.field.spatial_dimension == result.realization.spatial_dimension == 1
    assert result.field.logical_shape == result.realization.field_logical_shape
    assert result.field.minimum >= 0.0
    assert result.field.maximum > result.field.minimum
    assert isinstance(result.values, eqiora.Array)
    assert result.values.ownership == "owned"
    assert result.values.origin_copy_occurred is False
    assert result.values.shape == (result.field.value_count,)
    values = result.values.numpy(copy=False)
    assert values is result.values.numpy(copy=False)
    assert values.shape == (value_count,)
    assert values.dtype == np.float64
    assert not values.flags.writeable
    assert np.isfinite(values).all()
    assert float(values.min()) == result.field.minimum
    assert float(values.max()) == result.field.maximum
    assert result.balance.relative_imbalance < 1.0e-12
    assert result.solve.true_residual_norm <= result.solve.residual_target
    assert result.solve.reason in (
        eqiora.ConvergenceReason.InitialResidualSatisfied,
        eqiora.ConvergenceReason.ResidualToleranceSatisfied,
    )
    assert len(result.output_fingerprint) == 64

    manifest = result.run_manifest
    replay = eqiora.RunManifest.from_json(
        manifest.to_json(),
        realization=accepted,
    )
    assert replay == manifest
    assert hash(replay) == hash(manifest)
    assert replay.digest == manifest.digest
    assert replay.model_digest == model.digest
    assert replay.realization_digest == accepted.digest
    assert replay.semantic_revision == 1
    assert replay.workers == 1
    assert replay.reduction == "reproducible"
    assert replay.adapter == result.solve.adapter
    assert replay.solver_backend == result.solve.backend
    assert replay.output_digests == []


@pytest.mark.parametrize("dimension", [1, 2, 3])
@pytest.mark.parametrize(
    "method",
    [
        eqiora.ScalarEllipticMethod.FiniteElement,
        eqiora.ScalarEllipticMethod.FiniteVolume,
    ],
)
def test_runtime_dimensional_field_shape_and_canonical_order_are_explicit(
    dimension: int,
    method: eqiora.ScalarEllipticMethod,
) -> None:
    cells = 4 if dimension == 1 else 2
    model = eqiora.compile(affine_poisson(dimension))
    accepted = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(method=method, cells_per_axis=cells),
    )
    result = eqiora.run(model, realization=accepted)
    grid = (
        np.linspace(0.0, 1.0, cells + 1)
        if method == eqiora.ScalarEllipticMethod.FiniteElement
        else (np.arange(cells, dtype=np.float64) + 0.5) / cells
    )
    expected = np.array(
        [
            sum((axis + 1) * coordinate for axis, coordinate in enumerate(point))
            for point in product(grid, repeat=dimension)
        ]
    )

    assert accepted.spatial_dimension == dimension
    assert accepted.field_logical_shape == (len(grid),) * dimension
    assert result.field.spatial_dimension == dimension
    assert result.field.logical_shape == accepted.field_logical_shape
    assert result.field.value_count == expected.size
    np.testing.assert_allclose(
        result.values.numpy(copy=False), expected, rtol=0.0, atol=2.0e-14
    )


def test_complete_field_values_outlive_the_result_owner() -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    accepted = eqiora.preview_realization(model, request())
    result = eqiora.run(model, realization=accepted)
    values = result.values.numpy(copy=False)
    expected = values.copy()

    del result
    del accepted
    del model
    gc.collect()

    np.testing.assert_array_equal(values, expected)
    assert not values.flags.writeable


def test_sync_and_await_share_one_scalar_elliptic_run_lifecycle() -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    accepted = eqiora.preview_realization(model, request())

    submitted = eqiora.submit(model, realization=accepted)
    synchronous = submitted.result()

    assert submitted.result() is synchronous
    assert submitted.status == eqiora.RunStatus.Completed
    assert submitted.done
    assert submitted.progress == eqiora.ScalarEllipticRunProgress.SolutionAccepted
    assert submitted.cancellation is None
    assert submitted.model_digest == model.digest
    assert submitted.plan_key == accepted.digest
    assert submitted.adapter == synchronous.run_manifest.adapter
    assert isinstance(synchronous, eqiora.ScalarEllipticResult)

    async def await_result() -> eqiora.ScalarEllipticResult:
        return await eqiora.submit(model, realization=accepted)

    awaited = asyncio.run(await_result())
    np.testing.assert_array_equal(
        awaited.values.numpy(copy=False), synchronous.values.numpy(copy=False)
    )
    assert awaited.output_fingerprint == synchronous.output_fingerprint
    assert awaited.run_manifest == synchronous.run_manifest


def test_scalar_elliptic_cancellation_is_typed_and_publishes_no_result() -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    accepted = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(
            method=eqiora.ScalarEllipticMethod.FiniteElement,
            cells_per_axis=50_000,
        ),
    )
    submitted = eqiora.submit(model, realization=accepted)

    assert submitted.cancel()
    assert not submitted.cancel()
    with pytest.raises(eqiora.CancellationError) as caught:
        submitted.result()

    assert caught.value.diagnostics[0].code == "EQ0506"
    assert submitted.status == eqiora.RunStatus.Cancelled
    assert submitted.done
    assert isinstance(submitted.cancellation, eqiora.ScalarEllipticRunCancellation)
    assert submitted.progress == submitted.cancellation.progress
    assert submitted.cancellation.plan_key == accepted.digest
    assert submitted.cancellation.progress in (
        eqiora.ScalarEllipticRunProgress.PlanReplayed,
        eqiora.ScalarEllipticRunProgress.SystemFinalized,
        eqiora.ScalarEllipticRunProgress.SolutionAccepted,
    )
    assert not submitted.cancel()


def test_run_forms_are_exclusive_and_the_native_entry_point_stays_private() -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    accepted = eqiora.preview_realization(model, request())

    with pytest.raises(TypeError, match="requires either realization"):
        eqiora.run(model)
    with pytest.raises(TypeError, match="requires either realization"):
        eqiora.run(model, end_time=1.0)
    with pytest.raises(TypeError, match="realization alone"):
        eqiora.run(
            model,
            realization=accepted,
            end_time=1.0,
            max_step=0.1,
        )
    with pytest.raises(TypeError, match="requires either realization"):
        eqiora.submit(model)
    with pytest.raises(TypeError, match="realization alone"):
        eqiora.submit(
            model,
            realization=accepted,
            end_time=1.0,
            max_step=0.1,
        )

    assert "run_realization" not in eqiora.__all__
    assert not hasattr(eqiora, "run_realization")


def test_scalar_elliptic_run_releases_the_gil() -> None:
    model = eqiora.compile(POISSON, filename="poisson.eqi")
    accepted = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(
            method=eqiora.ScalarEllipticMethod.FiniteElement,
            cells_per_axis=2048,
        ),
    )
    observer_ready = threading.Event()
    observer_start = threading.Event()
    observer_ran = threading.Event()

    def observe() -> None:
        observer_ready.set()
        assert observer_start.wait(timeout=2.0)
        observer_ran.set()

    observer = threading.Thread(target=observe)
    observer.start()
    assert observer_ready.wait(timeout=2.0)
    previous_switch_interval = sys.getswitchinterval()
    sys.setswitchinterval(1.0)
    try:
        observer_start.set()
        eqiora.run(model, realization=accepted)
        observed_before_return = observer_ran.is_set()
    finally:
        sys.setswitchinterval(previous_switch_interval)
    observer.join(timeout=2.0)

    assert not observer.is_alive()
    assert observed_before_return, "the spatial native run retained the GIL"


def test_an_accepted_realization_cannot_run_against_a_foreign_model() -> None:
    original = eqiora.compile(POISSON, filename="original.eqi")
    accepted = eqiora.preview_realization(original, request())
    foreign = eqiora.compile(
        POISSON.replace(
            "source_scale: 1 / m ^ 2 = 1",
            "source_scale: 1 / m ^ 2 = 2",
        )
    )

    with pytest.raises(eqiora.ValidationError) as caught:
        eqiora.run(foreign, realization=accepted)
    assert caught.value.diagnostics[0].code == "EQ0807"
