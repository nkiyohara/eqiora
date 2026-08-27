# Eqiora

Eqiora is a typed mathematical modeling and execution system backed by one
canonical Rust implementation. Its Python SDK provides immutable native
declarations, synchronous and awaitable execution, explicit NumPy/DLPack
ownership, bounded first-order PyTorch and JAX adapters, and an optional
Matplotlib Result adapter without reimplementing model meaning in Python.

> **Alpha — `0.1.0a3`.** The supported boundary is intentionally narrow.
> Consult the [capability matrix](https://eqiora.org/capabilities/) before
> relying on a method, backend, or platform.

## Install

Eqiora `0.1.0a3` supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64:

```console
python -m pip install eqiora==0.1.0a3
```

Automatic exact-cylinder meshing requires Gmsh 4.15.2. The conventional Linux
installation is:

```console
sudo apt-get install libglu1-mesa
python -m pip install "eqiora[gmsh]==0.1.0a3"
```

The Gmsh extra is separate so the base `manylinux_2_17` package keeps its
compatibility floor; the current Gmsh wheel has a newer Linux floor.

Optional first-order framework adapters are explicit:

```console
python -m pip install "eqiora[torch]==0.1.0a3"
python -m pip install "eqiora[jax]==0.1.0a3"
python -m pip install "eqiora[matplotlib]==0.1.0a3"
python -m pip install "eqiora[notebook]==0.1.0a3"
```

The exact-cylinder pressure example combines the mesher and plot adapter:
`python -m pip install "eqiora[gmsh,matplotlib]==0.1.0a3"`.

The base package imports none of these optional libraries. The PyTorch extra
declares `torch>=2.13,<2.14`; this release verifies exactly PyTorch 2.13.0. It
also verifies the exact JAX/JAXLIB 0.11.0 pair and Matplotlib 3.11.1 on
CPython 3.13. The JAX extra requires Python 3.12 or newer.

The exact `notebook` extra installs anywidget 0.11.0 and keeps the complete
private Three.js frontend inside the Eqiora wheel. In the verified Linux
x86-64 CPython 3.13 profile, a bare exact accepted 50-chord circular-hole
`Mesh` renders interactively in JupyterLab 4.6.2 and marimo 0.23.16. The same
private runtime includes an unverified product view for the accepted
fixed-reference FSI `Trajectory`, with stored-state previous/next, playback,
speed, time, and scalar-Field metadata without interpolation or Python
writeback. Focused adapter and frontend tests cover that bounded Trajectory
view; it adds no registered host-support claim. Other meshes, trajectories,
fields, and hosts retain deterministic text; this does not add Mesh selection,
field display, saved widget state, a public viewer API, or Studio coupling.

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
mesh_request = eqiora.meshing.MeshRequest(
    eqiora.meshing.GmshMesher(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
)
mesh_plan = eqiora.meshing.resolve(geometry, mesh_request)
mesh = eqiora.meshing.generate(geometry, plan=mesh_plan)

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
The common `Result` retains their Mesh, Realization, Run, Field, and evidence
lineage rather than returning an unowned array. This is one verified 2D
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

The accepted exact-cylinder path now begins with explicit native-owned sketch
composition:

```python
base_sketch = eqiora.geometry.CadAuthoredSketch.rectangle_xy(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    modeling_tolerance=1e-10,
)
base = base_sketch.extrude_positive_z(depth=1.0)
cut_sketch = eqiora.geometry.CadAuthoredSketch.circle_on_face(
    base.face_handle("end-cap"),
    center=(0.2, 0.2),
    radius=0.05,
)
graph = base.through_cut(cut_sketch, boolean_tolerance=1e-10)
geometry = graph.planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
request = eqiora.meshing.MeshRequest(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
plan = eqiora.meshing.resolve(geometry, request)
mesh = eqiora.meshing.generate(geometry, plan=plan)
print(geometry.digest, mesh.digest)
print(mesh.selection_entity_count("cylinder"))
```

The sketch wrappers retain native values and all dimensions and tolerances are
coherent-SI metres. Existing `CadAuthoredGraph.rectangle_extrusion` and
`graph.circular_through_cut` calls remain supported and reproduce the same
canonical graph. The graph and its exact planar section have distinct
identities. The section reproduces the accepted exact planar value
byte-for-byte; depth and CAD tolerances cannot leak into its independently
classified 2D meaning. This is not a generic Sketch, section, or Python Boolean
implementation. Its matching meshing operation invokes exact Gmsh 4.15.2,
then admits the MSH 4.1 linear triangles through Rust-owned quality and
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

The accepted fixed-reference FSI workflow follows the same rule. It resolves a
fully explicit, immutable `FixedMeshMonolithic` intent before submitting the
ordinary Run; both coupled time steps execute inside the shared Rust application
service:

```console
python examples/python/fixed_reference_fsi.py \
  --fsi-png fixed-reference-fsi.png --step 2 --displacement-scale 12
```

The common immutable `Result` exposes the ordered fields and lineage through its
`Trajectory`, while `fixed_mesh_monolithic_evidence(result)` owns the accepted
partition and FSI-specific solver/acceptance observations. The optional still
uses only the general trajectory field adapters. This is one verified
fixed-reference monolithic case, not general FSI, ALE or moving-mesh support, a
Python time loop, or an animation surface.

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
    source="""
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

`0.1.0a3` is an alpha prerelease. Public Python names and serialized contracts
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
