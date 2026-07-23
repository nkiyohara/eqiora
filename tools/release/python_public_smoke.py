#!/usr/bin/env python3
"""Run public quick starts against an installed Eqiora distribution."""

from __future__ import annotations

import argparse
import asyncio
import importlib.metadata
import sys


POISSON = """
model release_smoke_poisson {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  domain x_upper = boundary(square, axis = 0, side = upper);
  domain y_lower = boundary(square, axis = 1, side = lower);
  domain y_upper = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;
  field potential on square as scalar_space: 1 = 0;
  parameter diffusion: 1 = 1;
  parameter wave_number: 1 / m = 3.141592653589793;
  parameter source_scale: 1 / m ^ 2 = 19.739208802178716;
  parameter boundary_offset: 1 = 0;
  relation balance continuous on square {
    -div(diffusion * grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) - boundary_offset = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) - boundary_offset = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) - boundary_offset = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) - boundary_offset = 0; }
}
"""


def decay_model(eqiora):
    state = eqiora.Field("state", initial=1.0)
    rate = eqiora.Parameter(
        "rate",
        value=1.0,
        dimension=eqiora.Dimension(time=-1),
    )
    decay = eqiora.Relation(
        "decay",
        residual=eqiora.derivative(state) + rate * state,
    )
    return eqiora.Model.define("decay", state, rate, decay)


def differentiable_program(eqiora):
    model = eqiora.compile(POISSON)
    realization = eqiora.preview_realization(
        model,
        eqiora.ScalarElliptic(
            method=eqiora.ScalarEllipticMethod.FiniteElement,
            cells_per_axis=4,
        ),
    )
    return eqiora.diff.compile(
        model,
        realization,
        inputs=(
            model.parameter("source_scale"),
            model.parameter("diffusion"),
            model.parameter("boundary_offset"),
        ),
        output=model.field("potential"),
    )


async def await_result(eqiora, model):
    run = eqiora.submit(model, end_time=0.2, max_step=0.01)
    result = await run
    assert run.done
    progress = run.progress
    assert isinstance(progress, eqiora.RunProgress)
    assert 0.0 <= progress.model_time <= progress.end_time == 0.2
    assert 0 <= progress.accepted_steps <= progress.maximum_steps
    return result


def base_smoke(expected_version: str) -> None:
    before = set(sys.modules)
    import eqiora

    assert importlib.metadata.version("eqiora") == expected_version
    assert eqiora.__version__ == expected_version
    assert not ({"torch", "jax", "jaxlib"} & (set(sys.modules) - before))

    model = decay_model(eqiora)
    result = eqiora.run(model, end_time=1.0, max_step=0.01)
    time = result["state"].time.numpy(copy=False)
    values = result["state"].values.numpy(copy=False)
    assert time.shape == values.shape
    assert time[-1] == 1.0
    assert 0.0 < values[-1] < 1.0
    assert not values.flags.writeable
    owned = result["state"].values.numpy(copy=True)
    assert owned.flags.writeable
    assert owned.base is None

    awaited = asyncio.run(await_result(eqiora, model))
    assert awaited["state"].values.numpy(copy=False)[-1] > 0.0

    try:
        eqiora.run(model, end_time=-1.0, max_step=0.01)
    except eqiora.EqioraError as error:
        assert error.category
        assert error.diagnostics
        assert all(diagnostic.code for diagnostic in error.diagnostics)
    else:
        raise AssertionError("invalid execution should expose structured diagnostics")


def torch_smoke(expected_version: str) -> None:
    base_smoke(expected_version)
    import eqiora
    import eqiora.torch as eqtorch
    import torch

    solve = eqtorch.bind(differentiable_program(eqiora))
    point = torch.tensor([17.0, 1.2, 0.1], dtype=torch.float64, requires_grad=True)
    output = solve(point)
    output.square().sum().backward()
    assert output.device.type == "cpu"
    assert point.grad is not None
    assert torch.isfinite(point.grad).all()


def jax_smoke(expected_version: str) -> None:
    base_smoke(expected_version)
    import eqiora
    import eqiora.jax as eqjax
    import jax
    import jax.numpy as jnp

    jax.config.update("jax_enable_x64", True)
    solve = eqjax.bind(differentiable_program(eqiora))
    point = jnp.array([17.0, 1.2, 0.1], dtype=jnp.float64)
    output = jax.jit(solve)(point)
    gradient = jax.grad(lambda value: jnp.sum(solve(value) ** 2))(point)
    output.block_until_ready()
    gradient.block_until_ready()
    assert output.devices() == {jax.devices("cpu")[0]}
    assert bool(jnp.all(jnp.isfinite(gradient)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-version", required=True)
    parser.add_argument("--profile", choices=("base", "torch", "jax"), required=True)
    arguments = parser.parse_args()
    {"base": base_smoke, "torch": torch_smoke, "jax": jax_smoke}[
        arguments.profile
    ](arguments.expected_version)
    print(f"installed Eqiora {arguments.expected_version} {arguments.profile} smoke passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
