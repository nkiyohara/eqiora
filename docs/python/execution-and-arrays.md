# Execution, diagnostics, and arrays

## Model to Plan to Run

The `.eqi` `Model` owns equations, coefficients, fields, supports, and boundary
laws. Python supplies only typed numerical choices. Resolution binds those
choices to one exact Model and Mesh before execution:

```python
model = eqiora.compile(source, filename="poisson.eqi")
mesh = eqiora.meshing.Cartesian(cells_per_axis=16)

plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.Q1(),
    solve=eqiora.solve.Linear(),
)
result = eqiora.run(plan)
```

Use `eqiora.fvm.CellCenteredTpfa()` for the other currently admitted spatial
policy. `Plan` owns the exact Model and Mesh and publishes every effective
space, quadrature, solver/backend, reduction, provider, and host placement
choice; `run(plan)` therefore does not ask for the Model again. An unsupported
request or policy, or an incompatible operator, fails during `resolve` without
a fallback. The Cartesian request deliberately repeats no Domain bounds;
`resolve` obtains the effective bounds from the current `.eqi` Model.

This first common slice is intentionally closed to generated Cartesian scalar
elliptic Q1 FEM and cell-centred orthogonal TPFA FVM with the existing
host-serial reference linear solve. It is not a generic method registry or a
promise that arbitrary policies compose. The former `ScalarElliptic` /
`preview_realization` form remains temporary alpha compatibility while other
proved consumers migrate to the common lifecycle.

## One run lifecycle

Blocking and awaitable execution use the same native worker, state machine,
and once-materialized result:

```python
run = eqiora.submit(model, end_time=1.0, max_step=0.01)
print(run.status, run.progress)
result = run.result()

# The blocking convenience uses the same lifecycle.
same_kind_of_result = eqiora.run(
    model,
    end_time=1.0,
    max_step=0.01,
)
```

`RunStatus` records a finite accepted history from creation through one
terminal state. `progress` is an execution-family-specific coalesced snapshot,
not a percentage or event log. Repeated `result()` calls return the same
immutable Python result object.

Awaiting does not introduce another native runtime:

```python
async def simulate(model):
    run = eqiora.submit(model, end_time=1.0, max_step=0.01)
    try:
        return await run
    finally:
        if not run.done:
            run.cancel()
```

Cancelling the surrounding asyncio task and dropping a Run do not implicitly
cancel native work. Call `run.cancel()` explicitly. Cancellation is
cooperative at accepted execution boundaries, publishes typed cancellation
evidence, and never exposes a partial result. A request after the last
cancellable boundary may still complete.

Long native waits release the ordinary CPython GIL only after inputs are
owned. Solver iterations do not call Python. Free-threaded Python and
subinterpreter shutdown remain separate capabilities.

## Structured failures

Eqiora model and execution failures derive from `EqioraError`. Stable
subclasses distinguish validation, compatibility, capability, execution,
cancellation, and internal failures. Every Eqiora-raised error retains
structured diagnostics:

```python
try:
    result = eqiora.run(model, end_time=-1.0, max_step=0.01)
except eqiora.EqioraError as error:
    print(error.category)
    for diagnostic in error.diagnostics:
        print(diagnostic.code, diagnostic.severity, diagnostic.message)
        print(diagnostic.graph_path, diagnostic.source_span)
```

Python call-shape mistakes remain ordinary `TypeError` rather than fabricated
model diagnostics. Guarded native boundaries sanitize unwinding Rust panics as
`InternalError`; process abort and memory exhaustion are not recoverable
claims.

## NumPy ownership

An `Array` owns a dense, native-endian, rank-one CPU `float64` allocation.
Inspecting descriptors does not import NumPy.

```python
array = result["x"].values
view = array.numpy(copy=False)  # `None` has the same meaning
writable = array.numpy(copy=True)

assert not view.flags.writeable
assert writable.flags.writeable
```

The first no-copy projection transfers the native allocation once into an
opaque owner. The resulting C-contiguous NumPy array is irreversibly read-only
and remains alive independently of the Result and Array handles. `copy=True`
returns an independent writable allocation.

## DLPack

Eqiora exports an independent versioned CPU snapshot:

```python
import numpy as np

snapshot = np.from_dlpack(array)
```

The snapshot never aliases immutable result evidence. Legacy capsule requests,
non-CPU transfers, non-`None` streams, and `copy=False` fail closed because
consumer enforcement of DLPack's advisory read-only flag is not universal.

Differentiable-program inputs may arrive from a complete CPU:0 DLPack
producer. Eqiora requests a no-transfer view, validates dtype, rank, length,
byte order, alignment, and contiguity, then makes one documented owned staging
copy before native execution. This is not a zero-copy execution-input claim.
GPU streams, sparse/distributed arrays, and general Run inputs remain separate
contracts.
