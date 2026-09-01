# Eqiora

Eqiora is a typed mathematical modeling and execution system backed by one
canonical Rust implementation. Its Python SDK provides immutable native
declarations, synchronous and awaitable execution, explicit NumPy/DLPack
ownership, bounded first-order PyTorch and JAX adapters, and an optional
Matplotlib Result adapter plus a bounded optional Notebook viewer without
reimplementing model meaning in Python or JavaScript.

> **Alpha — `0.1.0a6`.** The supported boundary is intentionally narrow.
> Consult the [capability matrix](https://eqiora.org/capabilities/) before
> relying on a method, backend, or platform.

## Install

Eqiora `0.1.0a6` supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64:

```console
python -m pip install eqiora==0.1.0a6
```

Automatic exact-cylinder meshing requires Gmsh 4.15.2. The conventional Linux
installation is:

```console
sudo apt-get install libglu1-mesa
python -m pip install "eqiora[gmsh]==0.1.0a6"
```

The Gmsh extra is separate so the base `manylinux_2_17` package keeps its
compatibility floor; the current Gmsh wheel has a newer Linux floor.

Optional first-order framework adapters are explicit:

```console
python -m pip install "eqiora[torch]==0.1.0a6"
python -m pip install "eqiora[jax]==0.1.0a6"
python -m pip install "eqiora[matplotlib]==0.1.0a6"
python -m pip install "eqiora[viewer]==0.1.0a6"
```

The exact-cylinder pressure example combines the mesher and plot adapter:
`python -m pip install "eqiora[gmsh,matplotlib]==0.1.0a6"`.

The base package imports none of these optional libraries. The viewer extra
pins `anywidget==0.11.0`; its JavaScript and CSS are already carried inside the
Eqiora wheel, so the host does not fetch renderer assets at display time. The PyTorch extra
declares `torch>=2.13,<2.14`; this release verifies exactly PyTorch 2.13.0. It
also verifies the exact JAX/JAXLIB 0.11.0 pair and Matplotlib 3.11.1 on
CPython 3.13. The JAX extra requires Python 3.12 or newer.

The exact-cylinder example composes the Geometry → Gmsh Mesh → root Plan
workflow and displays its caller-owned Matplotlib Figure. The bounded viewer is
a separate read-only presentation surface and adds no rich display semantics
to `Trajectory`.

## Geometry to evidence

The first complete application keeps reusable equations in Eqiora source and
the one concrete shape in Python. It compiles both into one ordinary `Model`,
then resolves an inspectable mesh and numerical policies before execution:

```python
from importlib.resources import files

import eqiora

graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
geometry = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:4],
    "cylinder": circle.boundaries[0],
})
mesh_request = eqiora.meshing.GmshMesher(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
mesh = eqiora.meshing.generate(mesh_plan)

model = eqiora.compile(
    path=files(eqiora).joinpath("examples", "steady-flow-past-cylinder.eqi"),
    geometry=geometry,
    parameters={
        "dynamic_viscosity": 1.0e-3,
        "zero_pressure": 0.0,
        "inlet_speed": 0.3,
        "channel_height": geometry.bounds[1][1] - geometry.bounds[1][0],
    },
)
linear = eqiora.solve.Linear(
    relative_tolerance=1e-6,
    absolute_tolerance=1e-13,
    maximum_iterations=10_000,
)
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.MiniP1(),
    solve=linear,
    scaling=None,
)
result = eqiora.run(plan)
evidence = eqiora.fluid.steady_stokes_evidence(result)

print(result.plan_key)
print(evidence.solve)
print("pressure", evidence.pressure_minimum, evidence.pressure_maximum, "Pa")
print("cylinder force on fluid", evidence.cylinder_force_on_fluid, "N/m")
print("net flux", evidence.net_flux, "m^2/s")
```

The exact Geometry and Model remain distinct from meshing and execution plans.
The common `Result` retains their Geometry, Model, Mesh, Plan, Field, and
observation lineage rather than returning an unowned array. This is one verified 2D
steady-Stokes case, not general CFD; its precise boundary and the optional
pressure plot are described in
[Modeling and realization](https://eqiora.org/python/modeling/#exact-cylinder-steady-stokes-result).

One explicit locked Model Package can also be checked through the installed
Python distribution:

```python
from pathlib import Path

resolution_bytes = Path("resolution.canonical.json").read_bytes()
report = eqiora.check_package_conformance(
    "package-store",
    resolution_bytes,
    entry_model="Main",
    profile="eqiora.package.structural-conformance-v1",
)
```

The immutable in-process report states structural compatibility and exact
package-compilation and current Model identity only. A deliberately false
scientific claim in package documentation can still pass: the operation does
not prove physics, well-posedness, realizability, numerical accuracy,
convergence, performance, or execution support. It runs no package code or
tests and creates no registry, installation, publishing, trust, badge,
attestation, durable report wire, scientific-evidence decision, or Studio
workflow. The precise boundary is documented under
[Modeling and realization](https://eqiora.org/python/modeling/#check-one-exact-package-structurally).

The accepted exact-cylinder path uses one planar GeometryGraph as the sole
shape authority:

```python
graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
geometry = graph.build(
    fluid,
    named_topology={
        "fluid": fluid.region,
        "inlet": rectangle.boundaries[0],
        "outlet": rectangle.boundaries[1],
        "walls": rectangle.boundaries[2:],
        "cylinder": circle.boundaries[0],
    },
)
request = eqiora.meshing.GmshMesher(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
plan = eqiora.meshing.resolve(geometry, request)
mesh = eqiora.meshing.generate(plan)
print(geometry.digest, mesh.digest)
print(mesh.selection_entity_count("cylinder"))
```

The same `GeometryGraph` also owns the admitted typed solid operations through
`graph.rectangle_extrusion(...)` and
`graph.circular_through_cut(solid, ...)`. All dimensions and tolerances are
coherent-SI metres. Solid-operation and resulting planar Geometry identities
remain distinct. The planar result reproduces the accepted exact value
byte-for-byte; depth and CAD tolerances cannot leak into its independently
classified 2D meaning. This is not a generic Sketch, section, or Python Boolean
implementation. Its matching `resolve` call derives a complete immutable plan
without invoking the provider. `generate` invokes exact Gmsh 4.15.2 for that
call, then admits the MSH 4.1 linear triangles through Rust-owned quality and
source-correspondence checks. Missing, wrong-version, failed, or invalid Gmsh
output rejects without falling back to the retired spoke reference mesh.
The returned value retains exact source and correspondence identity within the
live process; durable generated-realization replay, geometry-backed
Model binding, solve, Result, and visualization are separate capabilities.

The accepted exact-cylinder Result can be presented as one bounded pressure
still:

```python
import eqiora.matplotlib as eqplot

# `result` is the common Result returned by the accepted fluid solve.
pressure = result.snapshots[0]
figure = eqplot.plot_scalar_field(result, field=pressure.field)
figure.savefig("exact-cylinder-pressure.png")
```

The adapter selects an exact Model-bound Field from the accepted Result rather
than accepting raw arrays. It uses the Result's paired Mesh connectivity,
vertex-associated P1 pressure, and Rust-owned full pressure range in pascals.
This slice does not claim arbitrary fields, vectors, animation,
media-publication, or visual validation.

The same accepted objects can be composed in the optional interactive
Notebook viewer without converting them to renderer-shaped dictionaries:

```python
# `geometry`, `mesh`, and `result` come from one accepted Plan/Run workflow.
pressure = result.output(plan.capability.pressure)
view = eqiora.View().add(geometry).add(mesh).add(pressure)
view.show()  # Display through the configured anywidget host.

# Release the widget, browser resources, and retained accepted objects when done.
view.close()
```

The current V0--V3 boundary is planar Geometry, 2D triangle/quadrilateral Mesh,
exact edge/face named selections, and scalar vertex/cell `FieldOutput`. Orbit, pan, zoom,
reset, surface/edge visibility, selection colour/isolation, and exact accepted
cell or nearest-vertex inspection are presentation operations only. The field
must belong to the exact Mesh in the same `View`; foreign owners fail closed.
Without the viewer extra or a rich host, `View` retains a deterministic text
representation. Vector/tensor fields, trajectories, animation, 3D, contours,
streamlines, derived science, Studio, and Cloud integration are not part of
this slice; vertex selections are reported unavailable rather than guessed.

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

The accepted fixed-reference FSI workflow uses the root common lifecycle. It
authors the adjacent Geometry and Mesh in Python, compiles the equations-only
Component, resolves exact Domain-scoped MINI/P1 and P1 policies with typed time,
solve, and scaling policies, then initializes four exact Fields:

```console
python examples/python/fixed_reference_fsi.py
```

The common immutable `Result` exposes the ordered fields and lineage through its
`Trajectory`, while `eqiora.fsi.evidence(result)` owns the accepted
partition and FSI-specific solver/acceptance observations. The optional still
uses only the general trajectory field adapters. This is one verified
fixed-reference monolithic case, not general FSI, ALE or moving-mesh support, a
Python time loop, or an animation surface.

## Structured diagnostics

Failures expose stable categories and structured diagnostics:

```python
try:
    eqiora.run(
        plan,
        state=eqiora.State.initial(plan),
        until_s=-1.0,
        output_times_s=(-1.0,),
    )
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
async def simulate(plan):
    run = eqiora.submit(
        plan,
        state=eqiora.State.initial(plan),
        until_s=10.0,
        output_times_s=(10.0,),
    )
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
example constructs the Geometry, Mesh, Model, and matching common Plan before
compiling the differentiable program:

```python
import numpy as np

graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(rectangle, named_topology={
    "square": rectangle.region,
    "x_lower": rectangle.boundaries[0],
    "x_upper": rectangle.boundaries[1],
    "y_lower": rectangle.boundaries[2],
    "y_upper": rectangle.boundaries[3],
})
mesh_provider = eqiora.meshing.CartesianMesher(cells=(4, 4))
mesh_plan = eqiora.meshing.resolve(geometry, mesh_provider)
mesh = eqiora.meshing.generate(mesh_plan)

model = eqiora.compile(
    source="""
    public component DifferentiatedPoisson {
      public support square: volume(ambient_dimension = 2);
      public support x_lower: boundary(parent = square);
      public support x_upper: boundary(parent = square);
      public support y_lower: boundary(parent = square);
      public support y_upper: boundary(parent = square);
      representation scalar_space = continuum;
      field potential on square as scalar_space: 1 = 0;
      public parameter diffusion: 1;
      public parameter wave_number: 1 / m;
      public parameter source_scale: 1 / m ^ 2;
      public parameter boundary_offset: 1;
      relation balance continuous on square {
        -div(diffusion * grad(potential))
          - source_scale * math.sin(wave_number * coordinate(0))
            * math.sin(wave_number * coordinate(1)) = 0;
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
    """,
    geometry=geometry,
    parameters={
        "diffusion": 1.0,
        "wave_number": np.pi,
        "source_scale": 2.0 * np.pi**2,
        "boundary_offset": 0.0,
    },
)
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=eqiora.fem.Q1(),
    solve=eqiora.solve.Linear(
        relative_tolerance=1.0e-10,
        absolute_tolerance=1.0e-12,
        maximum_iterations=10_000,
    ),
)
program = eqiora.diff.compile(
    plan,
    inputs=(
        model.parameter("source_scale"),
        model.parameter("diffusion"),
        model.parameter("boundary_offset"),
    ),
    output=plan.capability.field,
)
point = np.array([19.739208802178716, 1.0, 0.0], dtype=np.float64)
evaluation = program.evaluate(point)
values = evaluation.primal().output.numpy(copy=False)
```

The current Python path is host-CPU rank-one `float64` over an exact supplied
rectangular 2D Cartesian Mesh, using scalar-elliptic Q1 FEM or TPFA FVM.

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

`0.1.0a6` is an alpha prerelease. Public Python names and serialized contracts
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
