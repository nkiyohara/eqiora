# Differentiation and framework adapters

## Framework-neutral accepted points

`eqiora.diff` binds one exact common Plan, ordered Parameter coordinate set,
and complete output Field:

```python
import numpy as np

program = eqiora.diff.compile(
    plan,
    inputs=(model.parameter("source"),),
    output=plan.capability.fields[0],
)

evaluation = program.evaluate(np.array([1.5], dtype=np.float64))
primal = evaluation.primal()
jvp = evaluation.jvp(np.array([1.0], dtype=np.float64))
vjp = evaluation.vjp(
    np.ones(program.output_shape, dtype=np.float64)
)
```

The program's Model and Plan identity is static. Each evaluation
owns an explicit complete numerical point and its paired accepted
linearization without mutating the Model or replacing the Plan.
Unselected Parameters remain frozen at their canonical Model values.
Retaining one evaluation while evaluating another point cannot retarget its
primal, JVP, or VJP.

The bounded Python path accepts the exact supplied rectangular 2D Cartesian
Mesh already owned by a common scalar Plan, with Q1 FEM or TPFA FVM and a
linear host-serial native `float64` solve. Native scalar realization remains
separately 1D--3D. Point values,
tangents, and cotangents accept exact rank-one CPU arrays through the ownership
contract described in
[Execution, diagnostics, and arrays](execution-and-arrays.md).

Multiple outputs, objective languages, persisted programs, GPU
adjoints, and higher-order differentiation remain separate capabilities.

## Native ordered batches

The batch API is in the current source tree, not the published `0.1.0a7`
wheel. From a checkout with the declared Rust toolchain available:

```console
uv venv --python 3.13 .venv
uv pip install --python .venv/bin/python .
uv run --no-project --python .venv/bin/python your_batch.py
```

Plan a batch without solving, then execute it once in native code:

```python
batch = program.map(np.array([[1.5], [0.5], [1.5]], dtype=np.float64))
print(batch.input_shape, batch.output_shape)
result = batch.execute()
if isinstance(result, eqiora.CompleteEvaluationMap):
    states = result.primal()  # read-only [3, output components]
    tangents = result.jvp(np.ones((3, 1), dtype=np.float64)).tangent
    cotangents = result.vjp(np.ones(batch.output_shape, dtype=np.float64))
else:
    print(result.stopped_index, result.statuses, result.diagnostics)
```

The trailing input axis follows `program.input_ids`; preceding axes are the
row-major point grid. Equal points remain separate occurrences in request
order. A rank-one input is one point, and zero extents preserve output metadata
without executing a solver. `batch.points` exposes the frozen complete inputs;
`batch.occurrence_coordinates(i)` maps a flat occurrence back to its grid.

Share selected coordinates explicitly across the whole grid. For a program
whose ordered inputs are `(source, diffusion, boundary_offset)`:

```python
batch = program.map(
    np.array([[3.0, 0.0], [1.0, 0.2], [3.0, 0.0]], dtype=np.float64),
    shared_inputs=(model.parameter("diffusion"),),
    shared=np.array([2.0], dtype=np.float64),
)
result = batch.execute()
if isinstance(result, eqiora.CompleteEvaluationMap):
    jvp = result.jvp(
        np.ones((3, 2), dtype=np.float64),
        shared=np.array([0.1], dtype=np.float64),
    )
    vjp = result.vjp(np.ones(batch.output_shape, dtype=np.float64))
    # Shared covectors sum over all point occurrences; mapped ones stay separate.
    print(vjp.shared_cotangents, vjp.mapped_cotangents)
```

Mapped coordinates are the remaining inputs in Program order, not the order
of a Python dictionary. Shared values follow `shared_inputs`, which must be
distinct references from the exact Program's Model. Sharing along only some
point axes is not admitted; no broadcasting or identity conversion is applied.

JVP and VJP reuse accepted native linearizations. Optional `seed_shape` and
`point_axes` place point axes inside a nested product grid. For point shape
`(2, 4)`, `seed_shape=(3,)` and `point_axes=(0, 2)` mean `(2, 3, 4)`;
mapped tangent and output-cotangent arrays append their coordinate extent.
Shared tangents use `seed_shape + (shared_count,)`. Products retain the Plan,
axis metadata and per-member evidence. Rank is bounded to 32 point/seed axes;
`retained_bytes_limit` and `numerical_bytes_limit` bound native retained
numerical storage, not process peak memory.

NumPy, Eqiora Array and CPU DLPack inputs use the existing exact `float64`
ownership rules. NumPy/DLPack tensors must be aligned, native-endian and
C-contiguous. Inputs are copied before execution releases the GIL, so later
mutations cannot retarget a Plan or product. Eqiora Array is rank one; use its
read-only NumPy view and explicit reshape when a point grid is desired.

`eqiora.EvaluationMapCancellation()` can be passed to `batch.execute` and
cancelled from another Python thread. Native execution polls it only between
occurrences. A failed or cancelled prefix exposes accepted `member(i)` values
and exact status/diagnostics, but has no complete `primal`, `jvp` or `vjp`.
Importing and using batches requires neither JAX nor PyTorch.

## PyTorch

Install the optional adapter and bind outside the compiled function:

```console
uv venv --python 3.13 .venv
uv pip install --python .venv/bin/python ".[torch]"
```

```python
import torch
import eqiora.torch as eqtorch

torch_program = eqtorch.bind(program)
theta = torch.tensor(
    [1.5],
    dtype=torch.float64,
    requires_grad=True,
)
state = torch_program(theta)
state.square().sum().backward()

compiled_objective = torch.compile(
    lambda point: torch_program(point).square().sum(),
    fullgraph=True,
)
```

The current adapter declares PyTorch `>=2.13,<2.14` and verifies 2.13.0. It
registers a functional project-namespaced custom operator, a metadata-only fake
implementation, and a first-order autograd rule whose backward invokes
Eqiora's accepted VJP through a second custom operator.

Inputs are exact rank-one contiguous CPU:0 `float64` tensors. The adapter
mutates no input and returns a fresh versioned DLPack snapshot rather than an
alias of native evidence. Static programs are retained process-locally because
autograd and compiled graphs may outlive a temporary wrapper; mutable
evaluations and derivatives are not cached.

The current adapter supports in-process `torch.compile(fullgraph=True)` with
first-order gradients. Double backward, `vmap`, AMP, CUDA, `torch.export`, and
AOT packaging are not yet supported.

## JAX

The optional JAX adapter uses native typed FFI:

```console
uv venv --python 3.13 .venv
uv pip install --python .venv/bin/python ".[jax]"
```

```python
import jax
import jax.numpy as jnp
import eqiora.jax as eqjax

jax.config.update("jax_enable_x64", True)
jax_program = eqjax.bind(program)
theta = jnp.array([1.5], dtype=jnp.float64)
direction = jnp.array([0.25], dtype=jnp.float64)

state = jax.jit(jax_program)(theta)
_, tangent = jax.jvp(
    jax_program,
    (theta,),
    (direction,),
)
gradient = jax.grad(
    lambda point: jnp.sum(jax_program(point) ** 2)
)(theta)
```

This first slice requires Python 3.12 or newer and the exact JAX/JAXLIB 0.11.0
pair. Separate primal, JVP, and VJP typed FFI targets keep compiled numerical
execution free of Python host callbacks and do not differentiate solver
iterations.

Only the numerical Parameter point is traced. Program identity, shapes, dtype,
layout, and host-CPU placement are static. Inputs are ordinary unsharded
rank-one host-CPU `float64` arrays. Direct or explicitly compiled input
sharding, `pmap`, `vmap`, higher-order transformations, explicit output
sharding, accelerators, export, serialization, multiprocessing, and
performance claims remain outside this slice.

Importing base `eqiora` imports neither optional framework.
