# Differentiation and framework adapters

## Framework-neutral accepted points

`eqiora.diff` binds one exact common Plan, ordered Parameter coordinate set,
and complete output Field:

```python
import numpy as np

program = eqiora.diff.compile(
    plan,
    inputs=(model.parameter("source"),),
    output=plan.capability.field,
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

Multiple outputs, objective languages, batching, persisted programs, GPU
adjoints, and higher-order differentiation remain separate capabilities.

## PyTorch

Install the optional adapter and bind outside the compiled function:

```console
python -m pip install "eqiora[torch]"
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
python -m pip install "eqiora[jax]"
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
