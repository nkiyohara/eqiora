from __future__ import annotations

import asyncio
import gc
import subprocess
import sys
import threading
import time

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

OVERDETERMINED = """
model overdetermined {
  field x: 1 = 1;
  relation first continuous { x = 0; }
  relation second continuous { x = 0; }
}
"""


def test_sync_and_submitted_runs_share_one_result_contract() -> None:
    model = eqiora.compile(DECAY)
    synchronous = eqiora.run(model, end_time=0.2, max_step=0.1)
    submitted = eqiora.submit(model, end_time=0.2, max_step=0.1)
    first = submitted.result()
    second = submitted.result()

    assert first is second
    assert submitted.status == eqiora.RunStatus.Completed
    assert submitted.done
    assert submitted.history == (
        eqiora.RunStatus.Created,
        eqiora.RunStatus.Validating,
        eqiora.RunStatus.Queued,
        eqiora.RunStatus.Running,
        eqiora.RunStatus.Completed,
    )
    assert submitted.cancellation is None
    assert submitted.cancel() is False
    assert submitted.model_id == model.model_id == first.model_id
    assert submitted.model_digest == model.digest == first.model_digest
    assert submitted.model_revision == model.revision.number == first.model_revision
    assert submitted.plan_key == first.plan_key
    assert submitted.adapter == first.adapter == "eqiora.reference"
    assert first.elapsed_seconds >= 0.0
    np.testing.assert_array_equal(
        first["x"].time.numpy(), synchronous["x"].time.numpy()
    )
    np.testing.assert_allclose(
        first["x"].values.numpy(), synchronous["x"].values.numpy(), rtol=0.0, atol=0.0
    )

    retained = first["x"].values.numpy()
    del first, second, submitted, synchronous, model
    gc.collect()
    np.testing.assert_allclose(retained, [1.0, 1.0 / 1.1, 1.0 / 1.1**2])


def test_result_wait_releases_the_gil() -> None:
    model = eqiora.compile(DECAY)
    submitted = eqiora.submit(model, end_time=0.2, max_step=2.0e-6)
    ready = threading.Event()
    observed_while_running: list[bool] = []

    def advance_python() -> None:
        ready.set()
        time.sleep(0.02)
        observed_while_running.append(not submitted.done)

    observer = threading.Thread(target=advance_python)
    observer.start()
    assert ready.wait(timeout=1.0)
    assert not submitted.done, "the GIL probe completed before its wait began"
    submitted.result()
    observer.join(timeout=2.0)

    assert not observer.is_alive()
    assert observed_while_running == [True], (
        "the observer could not inspect a nonterminal Run while result() waited"
    )


def test_cancellation_is_typed_and_never_publishes_a_partial_result() -> None:
    model = eqiora.compile(DECAY)
    submitted = eqiora.submit(model, end_time=1.0, max_step=1.0e-6)
    assert submitted.cancel() is True
    assert submitted.cancel() is False

    with pytest.raises(eqiora.CancellationError) as captured:
        submitted.result()

    assert captured.value.category == "cancellation"
    assert captured.value.diagnostics[0].code == "EQ0506"
    assert submitted.status == eqiora.RunStatus.Cancelled
    assert submitted.done
    assert submitted.cancellation is not None
    assert submitted.cancellation.plan_key == submitted.plan_key
    assert submitted.cancellation.progress == submitted.progress
    assert submitted.cancellation.elapsed_seconds >= 0.0
    assert submitted.history[-2:] == (
        eqiora.RunStatus.Cancelling,
        eqiora.RunStatus.Cancelled,
    )


def test_failure_and_zero_interval_have_unambiguous_terminal_authority() -> None:
    invalid = eqiora.compile(OVERDETERMINED)
    failed = eqiora.submit(invalid, end_time=0.1, max_step=0.1)
    with pytest.raises(eqiora.ExecutionError) as captured:
        failed.result()
    assert captured.value.diagnostics[0].code == "EQ0503"
    assert failed.status == eqiora.RunStatus.Failed
    assert failed.done
    assert failed.cancel() is False

    zero = eqiora.submit(eqiora.compile(DECAY), end_time=0.0, max_step=0.1)
    zero.result()
    assert zero.status == eqiora.RunStatus.Completed
    assert zero.progress is None


def test_await_and_task_cancellation_do_not_redefine_native_cancellation() -> None:
    async def exercise() -> None:
        completed = eqiora.submit(eqiora.compile(DECAY), end_time=0.2, max_step=0.1)
        result = await completed
        assert result is completed.result()

        live = eqiora.submit(eqiora.compile(DECAY), end_time=1.0, max_step=1.0e-6)
        task = asyncio.ensure_future(live)
        await asyncio.sleep(0)
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
        assert live.status not in (
            eqiora.RunStatus.Cancelling,
            eqiora.RunStatus.Cancelled,
        )
        assert live.cancel() is True
        with pytest.raises(eqiora.CancellationError):
            live.result()

    asyncio.run(exercise())


def test_dropping_a_live_handle_does_not_block_interpreter_exit() -> None:
    program = f"""
import eqiora
model = eqiora.compile({DECAY!r})
eqiora.submit(model, end_time=10.0, max_step=1.0e-7)
"""
    completed = subprocess.run(
        [sys.executable, "-c", program],
        check=False,
        capture_output=True,
        text=True,
        timeout=10.0,
    )
    assert completed.returncode == 0, completed.stderr
