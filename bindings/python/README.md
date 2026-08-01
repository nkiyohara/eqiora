# Eqiora

Eqiora is a typed mathematical modeling and execution system backed by one
canonical Rust implementation. Its Python SDK provides immutable native
declarations, synchronous and awaitable execution, explicit NumPy/DLPack
ownership, bounded first-order PyTorch and JAX adapters, and an optional
Matplotlib Result adapter without reimplementing model meaning in Python.

> **Alpha — `0.1.0a1`.** The supported boundary is intentionally narrow.
> Consult the [capability matrix](https://eqiora.org/capabilities/) before
> relying on a method, backend, or platform.

## Install

Eqiora `0.1.0a1` supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64:

```console
python -m pip install eqiora==0.1.0a1
```

Optional first-order framework adapters are explicit:

```console
python -m pip install "eqiora[torch]==0.1.0a1"
python -m pip install "eqiora[jax]==0.1.0a1"
python -m pip install "eqiora[matplotlib]==0.1.0a1"
```

The base package imports none of these optional libraries. The PyTorch extra
declares `torch>=2.13,<2.14`; this release verifies exactly PyTorch 2.13.0. It
also verifies the exact JAX/JAXLIB 0.11.0 pair and Matplotlib 3.11.1 on
CPython 3.13. The JAX extra requires Python 3.12 or newer.

## Five-minute model and run

Build a decay relation from frozen native declarations and execute it through
the shared native lifecycle:

```python
import eqiora

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
model = eqiora.Model.define("decay", state, rate, decay)

result = eqiora.run(model, end_time=1.0, max_step=0.01)
time = result["state"].time.numpy(copy=False)
values = result["state"].values.numpy(copy=False)

print(eqiora.__version__)
print(model.digest)
print(time[-1], values[-1])
```

`Field`, `Parameter`, `Relation`, and `Model` are immutable handles over
Rust-owned meaning. A relation declares a residual equal to zero; validation,
typed lowering, atomic commit, execution, and artifact identity remain in
Rust. Spatial authoring and the bounded FEM/FVM realization path are described
in [Modeling and realization](https://eqiora.org/python/modeling/).

The accepted exact-cylinder path now begins at the shared authored-CAD graph:

```python
graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
).circular_through_cut(
    center=(0.2, 0.2),
    radius=0.05,
    boolean_tolerance=1e-10,
)
geometry = graph.planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
mesh = eqiora.meshing.circular_hole_chordal(
    geometry,
    max_boundary_error=1e-4,
    required_minimum_mean_ratio=1e-5,
    max_segments=50,
)
print(geometry.digest, mesh.mesh_digest)
print(mesh.selection_entity_count("cylinder"))
```

The graph and its exact planar section have distinct identities. The section
reproduces the accepted direct convenience value byte-for-byte; depth and CAD
tolerances cannot leak into its independently classified 2D meaning. This is
not a generic section or Python Boolean implementation. Its matching meshing
operation is one Rust-owned, error-controlled chordal reference path, not a
generic or production mesher. The returned wrapper binds its exact source only
within the live process; durable generated-realization replay, geometry-backed
Model binding, solve, Result, and visualization are separate capabilities.

The accepted exact-cylinder Result can be presented as one bounded pressure
still:

```python
import eqiora.matplotlib as eqplot

# `result` is the accepted exact-cylinder Result returned by the fluid solve.
figure = eqplot.plot_pressure(result)
figure.savefig("exact-cylinder-pressure.png")
```

The adapter accepts the complete accepted Result rather than raw arrays. It
uses the Result's explicit triangle connectivity, vertex-associated P1
pressure, and full pressure range in pascals. It is not a generic Field plot,
animation, media-publication, or visual-validation claim.

The accepted mixed-boundary structural workflow is likewise an ordinary
Python file:

```console
python examples/python/mixed_boundary_elasticity.py \
  --displacement-png mixed-boundary-displacement.png --scale 1
```

It compiles the packaged source through the single current Model API, executes
the shared Rust application result, and renders original and scaled-deformed
canonical Q1 edges. It is one bounded verified case, not a general structural
solver or deformation viewer.

The accepted fixed-reference FSI workflow follows the same rule and performs
both coupled time steps inside the shared Rust application service:

```console
python examples/python/fixed_reference_fsi.py \
  --fsi-png fixed-reference-fsi.png --step 2 --displacement-scale 12
```

The immutable result exposes the accepted partition, ordered coupled fields,
solver/acceptance evidence, and complete Model-to-trajectory-to-Run lineage.
The optional still uses only those result-owned values. This is one verified
fixed-reference monolithic case, not a general FSI API, ALE or moving-mesh
solver, Python time loop, or animation surface.

## Structured diagnostics

Failures expose stable categories and structured diagnostics:

```python
try:
    eqiora.run(model, end_time=-1.0, max_step=0.01)
except eqiora.EqioraError as error:
    print(error.category)
    for diagnostic in error.diagnostics:
        print(diagnostic.code, diagnostic.severity, diagnostic.message)
```

Validation, compatibility, capability, execution, cancellation, and internal
failures have distinct subclasses. Ordinary Python call-shape errors remain
`TypeError`.

## NumPy ownership and copies

Eqiora `Array` values own dense, rank-one CPU `float64` storage:

```python
array = result["state"].values
view = array.numpy(copy=False)
writable = array.numpy(copy=True)

assert not view.flags.writeable
assert writable.flags.writeable
```

`copy=False` and `copy=None` return the same lifetime-safe, read-only NumPy
projection. If that contract cannot be honored, Eqiora fails instead of
copying silently. `copy=True` returns an independent writable allocation.
DLPack exports are fresh versioned CPU snapshots, not aliases of immutable
result evidence. The complete contract is in
[Execution, diagnostics, and arrays](https://eqiora.org/python/execution-and-arrays/).

## Await, progress, and cancellation

`run(...)`, `submit(...).result()`, and `await submit(...)` share one native
state machine and one materialized result:

```python
async def simulate(model):
    run = eqiora.submit(model, end_time=10.0, max_step=0.001)
    try:
        print(run.status, run.progress)
        return await run
    finally:
        if not run.done:
            run.cancel()
```

Cancelling the surrounding asyncio task or dropping a `Run` does not
implicitly cancel native work. Call `run.cancel()` explicitly. Cancellation
is cooperative at accepted execution boundaries and never publishes a
partial result.

## PyTorch and JAX

Both optional adapters consume the same accepted, opaque
`DifferentiableProgram`. They do not define a second model. This complete
example constructs the spatial model and its matching realization before
compiling the differentiable program:

```python
import numpy as np

model = eqiora.compile(
    """
    model differentiated_poisson {
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
      relation x_lower_value continuous on x_lower {
        trace(potential) - boundary_offset = 0;
      }
      relation x_upper_value continuous on x_upper {
        trace(potential) - boundary_offset = 0;
      }
      relation y_lower_value continuous on y_lower {
        trace(potential) - boundary_offset = 0;
      }
      relation y_upper_value continuous on y_upper {
        trace(potential) - boundary_offset = 0;
      }
    }
    """
)
realization = eqiora.preview_realization(
    model,
    eqiora.ScalarElliptic(
        method=eqiora.ScalarEllipticMethod.FiniteElement,
        cells_per_axis=4,
    ),
)
program = eqiora.diff.compile(
    model,
    realization,
    inputs=(
        model.parameter("source_scale"),
        model.parameter("diffusion"),
        model.parameter("boundary_offset"),
    ),
    output=model.field("potential"),
)
point = np.array([19.739208802178716, 1.0, 0.0], dtype=np.float64)
evaluation = program.evaluate(point)
values = evaluation.primal().output.numpy(copy=False)
```

The current path is host-CPU, rank-one `float64`, generated-Cartesian scalar
elliptic Q1 FEM or TPFA FVM.

PyTorch uses Eqiora's accepted VJP in backward:

```python
import torch
import eqiora.torch as eqtorch

torch_program = eqtorch.bind(program)
theta = torch.tensor(point, dtype=torch.float64, requires_grad=True)
state = torch_program(theta)
state.square().sum().backward()
```

JAX uses typed native CPU FFI for primal, JVP, and VJP:

```python
import jax
import jax.numpy as jnp
import eqiora.jax as eqjax

jax.config.update("jax_enable_x64", True)
jax_program = eqjax.bind(program)
theta = jnp.array(point, dtype=jnp.float64)
gradient = jax.grad(lambda point: jnp.sum(jax_program(point) ** 2))(theta)
```

Device transfer is never hidden. GPU execution, output sharding, higher-order
differentiation, export/serialization, and general transformation support are
not claimed. See
[Differentiation and framework adapters](https://eqiora.org/python/differentiation/).

## Compatibility and limitations

`0.1.0a1` is an alpha prerelease. Public Python names and serialized contracts
change only deliberately and are documented in release notes, but breaking
changes may occur before 1.0. Corrections to a published artifact use a new
version; an existing release is never overwritten.

This distribution does not support macOS, Windows, free-threaded CPython, GPU
wheels, bundled MPI, or arbitrary user-defined native operators. It is not a
complete physics library or a safety-certified engineering tool.

## Links

- [Documentation](https://eqiora.org)
- [Python guide](https://eqiora.org/python/)
- [API index](https://eqiora.org/api/)
- [Source](https://github.com/nkiyohara/eqiora)
- [Issue tracker](https://github.com/nkiyohara/eqiora/issues)
- [Security policy](https://github.com/nkiyohara/eqiora/security/policy)
- [Apache-2.0 license](https://github.com/nkiyohara/eqiora/blob/main/LICENSE)
