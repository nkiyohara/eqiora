# Modeling and realization

## Native declarations

Python declarations are immutable inputs to the same typed draft, validation,
transaction, and canonical artifact path used by other Eqiora clients.
Python does not implement a second model semantics.

```python
import eqiora

x = eqiora.Field("x", initial=1.0)
rate = eqiora.Parameter(
    "rate",
    value=1.0,
    dimension=eqiora.Dimension(time=-1),
)
flow = eqiora.Relation(
    "flow",
    residual=eqiora.derivative(x) + rate * x,
)
model = eqiora.Model.define("decay", x, rate, flow)
```

A relation receives an explicit zero-valued residual. Symbolic equality and
Python truth testing are not modeling syntax. Declarations and expressions
are frozen; validation and artifact creation happen atomically in Rust.

## Exact geometry authoring

The first geometry constructor is intentionally one closed analytic family,
not a Python Boolean algebra:

```python
import eqiora

geometry = eqiora.geometry.RectangleWithCircularHole(
    bounds=((0.0, 2.2), (0.0, 0.41)),
    circle_center=(0.2, 0.2),
    circle_radius=0.05,
    tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)

assert geometry.selection_dimension("fluid") == 2
assert geometry.selection_dimension("cylinder") == 1
print(geometry.digest)
```

Rust owns validation, canonical ordering, bytes, and identity. The circle
remains centre-and-radius geometry; chord count, mesh size, and approximation
tolerance cannot enter this constructor. Generic primitives and Boolean
operations, multiple holes, selection handles, Model binding, solve, Result,
and visualization remain separate slices.

## Bounded chordal reference mesh

The matching meshing operation is an explicit Realization choice rather than a
method on exact geometry:

```python
mesh = eqiora.meshing.circular_hole_chordal(
    geometry,
    max_boundary_error=1e-4,
    required_minimum_mean_ratio=1e-5,
    max_segments=50,
)

assert mesh.source_digest == geometry.digest
assert mesh.circle_segments == 50
assert mesh.selection_entity_count("cylinder") == 50
print(mesh.mesh_digest)
```

Rust retains the exact source, chooses and measures the chordal approximation,
accepts the affine-triangle mesh, and derives realized named selections through
the geometry-to-mesh correspondence. `mesh_canonical_json` and `mesh_digest`
identify only the accepted inner simplicial mesh. The returned object is a
same-process owner, not a durable source-to-mesh artifact.

This bounded operation supports the one rectangle-with-circular-hole family
and fixed-phase reference topology. It is not a generic `Mesh` or
`MeshRequest`, production mesher, curved-element path, external import,
cross-process generated-realization proof, Model binding, solve, Result, or
visualization surface.

## Exact-cylinder steady Stokes result

The first fluid application consumes the accepted geometry-bound Model v7
artifact explicitly and returns one immutable result:

```python
from importlib.resources import files

model_v7 = (
    files(eqiora)
    .joinpath("examples", "steady-flow-past-cylinder.model-v7.json")
    .read_bytes()
)
result = eqiora.fluid.solve_exact_cylinder_stokes(
    model_v7=model_v7,
    geometry=geometry,
    mesh=mesh,
)

print(result.solve)
print(result.pressure_minimum, result.pressure_maximum)
print(result.cylinder_force_on_fluid, result.net_flux)
```

Studio and Python use the same Rust composition for Model replay, exact-source
binding, field-wise Realization, solve, pressure snapshot, and Run provenance.
`result.pressure` is the existing immutable rank-one `Array`;
`result.coordinates` and `result.triangles` lazily publish read-only NumPy
matrices in matching mesh order.

This operation admits only the checked exact-cylinder Model, geometry, mesh,
scale profile, and SparseLU policy. Supplying the Model bytes is explicit
artifact consumption; the wheel ships an exact copy of that one canonical
artifact so the documented script needs no repository-local runtime input.
This is not a general Model catalog or Python fluid authoring. Velocity
projection, drag/lift, solver selection, transient flow, and FSI remain
separate slices. The runnable file is
[`examples/python/exact_cylinder_stokes.py`](../../examples/python/exact_cylinder_stokes.py).

## Exact-cylinder pressure still

Install the optional Matplotlib adapter and ask the same runnable file to save
the accepted pressure field:

```console
python -m pip install 'eqiora[matplotlib]'
python examples/python/exact_cylinder_stokes.py \
  --pressure-png exact-cylinder-pressure.png
```

The equivalent composition API is:

```python
import eqiora.matplotlib as eqplot

figure = eqplot.plot_pressure(result)
figure.savefig("exact-cylinder-pressure.png")
```

The adapter accepts only the complete
`CircularHoleSteadyStokesResult`. It sends the Result's co-indexed P1 pressure,
coordinates, and explicit accepted triangle connectivity to Matplotlib and
uses the Result's pressure extrema in pascals. Gouraud shading is presentation
interpolation of the accepted vertex coefficients, not a new scientific
field.

Matplotlib remains optional and is not imported by base `eqiora`. This bounded
still does not claim raw-array or general Field plotting, contours, velocity,
interactive behavior, animation, deterministic image bytes, media admission,
or validation from visual similarity.

## Mixed-boundary structural demo

The installed package also carries the accepted exact-v4 mixed-boundary
elasticity source. Python compiles it explicitly and passes the immutable
Model into the same Rust-owned application result used by Studio:

```python
from importlib.resources import files

source = (
    files(eqiora)
    .joinpath("examples", "mixed-boundary-elasticity.eqi")
    .read_text()
)
model = eqiora.compatibility.compile_exact(
    source,
    filename="mixed-boundary-elasticity.eqi",
    codec=eqiora.compatibility.ExactModelCodec.V4,
)
result = eqiora.solid.solve_mixed_boundary_elasticity(model)
```

`result.coordinates`, `result.cells`, and `result.displacement` are
memoized, read-only NumPy matrices in one canonical Q1 order. Model,
Realization, Run, solver, assembly, reaction, and body-force evidence remains
owned by Rust. Stress, strain, traction recovery, analytic error, other
meshes, and general structural solving are not implied.

The optional still displays original and explicitly scaled deformed edges:

```python
figure = eqplot.plot_displacement(result, scale=1.0)
figure.savefig("mixed-boundary-displacement.png")
```

The complete runnable workflow is
[`examples/python/mixed_boundary_elasticity.py`](../../examples/python/mixed_boundary_elasticity.py).

## Conserving connections

Scalar conserving connections use nominal physical-domain identity:

```python
voltage = eqiora.Dimension(mass=1, length=2, time=-3, current=-1)
current = eqiora.Dimension(current=1)
electrical = eqiora.PhysicalDomain(
    "electrical",
    across_dimension=voltage,
    through_dimension=current,
)
left = eqiora.ConservingPort("left", domain=electrical)
right = eqiora.ConservingPort("right", domain=electrical)
component = eqiora.Relation(
    "component",
    residuals=(eqiora.across(left), eqiora.through(right)),
)
physical_model = eqiora.Model.define(
    "physical_pair",
    electrical,
    left,
    right,
    component,
    eqiora.connect(left, right),
)
```

Equal names and dimensions do not make separately constructed domains
interchangeable.

## Spatial declarations

Domain, boundary, Representation, Field support, and Relation support are
exact frozen handles. Python does not infer support from names or reproduce
the Semantic Kernel's dimensional and spatial checks.

```python
interval = eqiora.Domain.box("interval", (0.0, 1.0))
lower = interval.boundary(
    "lower",
    axis=0,
    side=eqiora.BoundarySide.Lower,
)
upper = interval.boundary(
    "upper",
    axis=0,
    side=eqiora.BoundarySide.Upper,
)
space = eqiora.Representation.continuum("scalar_space")
potential = eqiora.Field(
    "potential",
    domain=interval,
    representation=space,
)
source = eqiora.Parameter(
    "source",
    value=1.0,
    dimension=eqiora.Dimension(length=-2),
)
model = eqiora.Model.define(
    "poisson",
    interval,
    lower,
    upper,
    space,
    potential,
    source,
    eqiora.Relation(
        "balance",
        domain=interval,
        residual=-eqiora.div(eqiora.grad(potential)) - source,
    ),
    eqiora.Relation(
        "lower_value",
        domain=lower,
        residual=eqiora.trace(potential),
    ),
    eqiora.Relation(
        "upper_value",
        domain=upper,
        residual=eqiora.trace(potential),
    ),
)
```

`grad`, `div`, and `trace` are a closed adapter vocabulary over the shared
draft. Shape, frame, dimension, support, and residual validity remain Kernel
decisions.

## Bounded scalar-elliptic realization

The accepted spatial model can enter a small typed realization surface without
exposing the internal graph-shaped Realization IR:

```python
request = eqiora.ScalarElliptic(
    method=eqiora.ScalarEllipticMethod.FiniteElement,
    cells_per_axis=32,
)
realization = eqiora.preview_realization(model, request)
result = eqiora.run(model, realization=realization)

assert result.realization == realization
print(result.field.logical_shape)
print(result.balance.relative_imbalance)
print(result.solve.true_residual_norm)
values = result.values.numpy(copy=False)
```

`ScalarElliptic` is an unbound request. `preview_realization` resolves it
against one exact model and the host-serial capability profile before
numerical allocation. The resulting Realization is immutable and model-bound;
foreign models and mismatched persisted run manifests fail closed.

The current path supports generated Cartesian scalar elliptic systems in one
through three dimensions. Q1 finite elements return complete vertex values,
including eliminated essential-boundary values. TPFA finite volumes return
primary cell-centred values. Both use canonical row-major flattening with the
last physical axis varying fastest.

## Exact revisions and compatibility

`Model` owns one immutable canonical artifact. Previewing an edit never
mutates it, and committing a valid edit returns a child:

```python
base = model
edit = base.preview_value_edit("source", 2.0)
child = base.commit(edit)

assert base.revision != child.revision
assert base.digest == edit.base_digest
```

Commit checks the edit's exact base digest and graph revision atomically.
Stale or foreign plans produce no partial child. Ordinary authoring and edits
retain the current artifact profile; historical codecs are explicit:

```python
legacy = eqiora.compatibility.compile_exact(
    "model legacy { field x: 1 = 1; }",
    codec=eqiora.compatibility.ExactModelCodec.V2,
)
replayed = eqiora.compatibility.replay_exact(
    legacy.to_json(),
    codec=eqiora.compatibility.ExactModelCodec.V2,
)
```

Exact operations never sniff, fall back, or silently migrate a generation.

Independent definitions allocate fresh canonical occurrence identities, so
exact equality and digest equality are intentionally stronger than structural
comparison:

```python
source_model = eqiora.compile(
    """
    model decay {
      field x: 1 = 1;
      parameter rate: 1 / s = 1;
      relation flow continuous {
        derivative(x) + rate * x = 0;
      }
    }
    """
)
x = eqiora.Field("x", initial=1.0)
rate = eqiora.Parameter(
    "rate",
    value=1.0,
    dimension=eqiora.Dimension(time=-1),
)
native_model = eqiora.Model.define(
    "decay",
    x,
    rate,
    eqiora.Relation(
        "flow",
        residual=eqiora.derivative(x) + rate * x,
    ),
)

assert source_model != native_model
assert source_model.digest != native_model.digest
assert source_model.structurally_equivalent(native_model)
assert (
    source_model.structural_fingerprint
    == native_model.structural_fingerprint
)
```

The structural fingerprint omits names, formatting, source spans, occurrence
IDs, package provenance, and artifact codec. It is comparison evidence, not a
replacement for exact identity in execution, replay, provenance, or edits.
