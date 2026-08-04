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

## Compile one exact locked Model Package

Python can project an existing content-addressed Model Package into the same
ordinary immutable `Model` without reimplementing package semantics:

```python
from pathlib import Path

import eqiora

store_root = Path("package-store")
resolution = Path("resolution.canonical.json").read_bytes()
model = eqiora.compile_package(
    store_root,
    resolution,
    entry_model="Main",
)

print(model.digest)
print(model.package_compilation_digest)
```

The caller selects one explicit store directory, supplies the exact bytes from
`ResolutionRecordV1.canonical_json()`, and names one bare root-local Model.
Rust opens the capability-rooted store, verifies the complete locked closure,
compiles it through the shared package/compiler path, and returns the existing
`Model` type. Human-formatted, reordered, newline-terminated, duplicate-key, or
store-mismatched resolution bytes fail closed.

`package_compilation_digest` is read-only lineage for the accepted call. It is
absent on source-compiled, natively defined, replayed, and edited Models; it is
also deliberately excluded from Model JSON, equality, hashing, revision, and
structural comparison. This surface does not discover stores or lock files,
author or install packages, access registries or networks, select imported
Model roots, execute the Model, or add a Studio package workflow.

## Authored CAD to exact geometry

The first accepted path projects one closed authored-CAD history into its exact
transverse Geometry. Python names the two native-owned sketch inputs and does
not implement their operations:

```python
import eqiora

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

assert geometry.selection_dimension("fluid") == 2
assert geometry.selection_dimension("cylinder") == 1
print(geometry.digest)
```

Rust owns validation, graph binding, operation order, canonical ordering,
bytes, and both distinct identities. The sketch wrapper owns a native value,
so dropping its source graph or face-handle wrapper does not invalidate it.
Every coordinate, radius, depth, and CAD tolerance is a coherent-SI metre.
The existing `CadAuthoredGraph.rectangle_extrusion` and
`graph.circular_through_cut` signatures remain supported and produce the same
canonical graphs. The 3D graph retains its explicit depth and CAD tolerances;
none enter the derived 2D Geometry, whose classification tolerance is supplied
separately. The circle remains centre-and-radius geometry, so chord count,
mesh size, and approximation tolerance cannot enter it. A general Sketch,
arbitrary planes or profiles, operation DAGs, general Booleans or sections,
multiple holes, Model binding, solve, Result, Studio, and visualization remain
separate slices. Installed Python exposes the common `Geometry` projection
only through the accepted authored graph; it does not publish a demo-shaped
constructor.

## Bounded chordal reference mesh

The matching meshing operation is an explicit Realization choice rather than a
method on exact geometry:

```python
request = eqiora.meshing.MeshRequest(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
plan = eqiora.meshing.resolve(geometry, request)
mesh = eqiora.meshing.generate(geometry, plan=plan)

assert mesh.source_digest == geometry.digest
assert plan.boundary_facets == 50
assert mesh.selection_entity_count("cylinder") == 50
print(mesh.digest)
```

Rust retains the exact source, chooses and measures the chordal approximation,
accepts the affine-triangle mesh, and derives realized named selections through
the geometry-to-mesh correspondence. `canonical_bytes` and `digest` identify
only the accepted inner simplicial mesh. The returned object retains the
source, correspondence, and realization identities in the live process.

This bounded operation supports the one rectangle-with-circular-hole family
and fixed-phase reference topology. Its common ownership types do not claim a
production mesher, curved-element path, external import,
cross-process generated-realization proof, Model binding, solve, Result, or
visualization surface.

## Exact-cylinder steady Stokes result

The first fluid application consumes the accepted geometry-bound current Model
artifact explicitly and returns one immutable result:

```python
from importlib.resources import files

model_bytes = (
    files(eqiora)
    .joinpath("examples", "steady-flow-past-cylinder.model.json")
    .read_bytes()
)
model = eqiora.replay(model_bytes)
intent = eqiora.fluid.SteadyStokes(
    length_scale_m=0.41,
    velocity_scale_m_per_s=0.3,
    pressure_scale_pa=0.001 * 0.3 / 0.41,
    relative_tolerance=1e-6,
    absolute_tolerance=1e-13,
    maximum_iterations=10_000,
)
plan = eqiora.fluid.resolve(model, intent, mesh=mesh)
result = eqiora.run(model, plan=plan)

pressure = result.snapshots[0]
evidence = eqiora.fluid.steady_stokes_evidence(result)
print(result.run_manifest().digest, pressure.digest)
print(evidence.solve)
print(evidence.pressure_minimum, evidence.pressure_maximum)
print(evidence.cylinder_force_on_fluid, evidence.net_flux)
```

Studio and Python use the same Rust resolved Plan for Model replay, exact-source
binding, field-wise Realization, solve, pressure snapshot, and Run provenance.
The Plan exposes the exact spaces, scales, solver tuple, backend, placement,
and existing Realization bytes before a worker starts.
The common `Result` exposes one immutable pressure `FieldSnapshot`, selected by
its exact Model-bound `FieldRef`; `result.mesh(field)` returns the paired common
`Mesh`. Snapshot values and Mesh coordinates/connectivity lazily publish
read-only NumPy views in matching mesh order. Solver- and physics-specific
observations remain available through
`eqiora.fluid.steady_stokes_evidence(result)`.

This operation admits only the checked exact-cylinder Model and mesh plus the
frozen scale and SparseLU intent. Replaying the Model bytes is explicit
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

pressure = result.snapshots[0]
figure = eqplot.plot_scalar_field(result, field=pressure.field)
figure.savefig("exact-cylinder-pressure.png")
```

The adapter selects an exact Field from the common `Result`. It sends the
co-indexed P1 pressure, paired Mesh coordinates, and explicit accepted triangle
connectivity to Matplotlib and uses the Rust-owned pressure extrema in pascals.
Gouraud shading is presentation interpolation of accepted vertex coefficients,
not a new scientific field.

Matplotlib remains optional and is not imported by base `eqiora`. This bounded
still does not claim raw-array or arbitrary Field plotting, contours, velocity,
interactive behavior, animation, deterministic image bytes, media admission,
or validation from visual similarity.

## Mixed-boundary structural result

The installed package also carries the accepted mixed-boundary elasticity
source. Python compiles it through the current `Model` path, resolves an
explicit linear-elasticity intent before execution, and submits the resulting
model-bound Plan through the ordinary Run path:

```python
from importlib.resources import files

source = (
    files(eqiora)
    .joinpath("examples", "mixed-boundary-elasticity.eqi")
    .read_text()
)
model = eqiora.compile(
    source,
    filename="mixed-boundary-elasticity.eqi",
)
intent = eqiora.solid.LinearElasticity(
    cells_per_axis=16,
    relative_tolerance=1.0e-12,
    absolute_tolerance=1.0e-14,
    maximum_iterations=10_000,
)
plan = eqiora.solid.resolve(model, intent)
run = eqiora.submit(model, plan=plan)
result = run.result()

displacement = model.field("displacement")
snapshot = result.field(displacement)
mesh = result.mesh(displacement)
evidence = eqiora.solid.linear_elasticity_evidence(result)
```

`LinearElasticity` is keyword-only and has no hidden defaults. The resolved
`LinearElasticityPlan` exposes the effective generated-Cartesian Q1
Realization, solver, backend, execution, and worker choices before a worker
starts. Resolution admits only this already verified tuple and rejects other
values instead of silently falling back.

The common `Result` owns one immutable vector `FieldSnapshot` selected by the
caller's exact Model-bound `FieldRef`; `result.mesh(displacement)` returns its
paired exact generated-Cartesian `Mesh`. Snapshot values, Mesh coordinates,
and Q1 connectivity lazily publish memoized, read-only NumPy views in one
co-indexed canonical order. The typed `LinearElasticityEvidence` keeps the
Run digest, reference-CG solve summary, assembly counts, constrained reaction,
integrated body force, and exact bounds outside the common result transport.
Model, Realization, Geometry, correspondence, Mesh, Snapshot, and Run identity
remain Rust-owned and relationally exact. Stress, strain, traction recovery,
analytic error, other meshes, and general structural solving are not implied.

The optional still displays original and explicitly scaled deformed edges:

```python
import eqiora.matplotlib as eqplot

figure = eqplot.plot_deformed_field(
    result,
    field=displacement,
    scale=1.0,
)
figure.savefig("mixed-boundary-displacement.png")
```

For one subsequent prerelease, `MixedBoundaryElasticityResult`,
`solve_mixed_boundary_elasticity`, and `plot_displacement` remain only as
warning-emitting compatibility shims that delegate to this path. They own no
result storage, execution, lineage, plotting implementation, or evidence and
are not the ordinary API.

The complete runnable workflow is
[`examples/python/mixed_boundary_elasticity.py`](../../examples/python/mixed_boundary_elasticity.py).

## Fixed-mesh monolithic FSI result

The installed package carries the accepted fixed-reference two-body FSI source.
Python compiles it through the current Model path and resolves mandatory,
explicit fixed-mesh monolithic intent before execution. The shared Rust
application service owns the fixed mesh, coupled Realization, both consecutive
monolithic steps, spatial states, trajectory, and final Run:

```python
source = (
    files(eqiora)
    .joinpath("examples", "fixed-reference-fsi.eqi")
    .read_text()
)
model = eqiora.compile(
    source,
    filename="fixed-reference-fsi.eqi",
)
intent = eqiora.fsi.FixedMeshMonolithic(
    time_step_s=0.05,
    steps=2,
    initial_velocity_m_per_s=(0.0, 0.0),
    initial_free_interface_displacement_m=(0.02, 0.0),
    length_scale_m=2.0,
    velocity_scale_m_per_s=0.5,
    pressure_scale_pa=4.0,
    relative_tolerance=1.0e-11,
    absolute_tolerance=1.0e-13,
    maximum_iterations=20_000,
)
plan = eqiora.fsi.resolve(model, intent)
run = eqiora.submit(model, plan=plan)
result = run.result()
trajectory = result.trajectory
evidence = eqiora.fsi.fixed_mesh_monolithic_evidence(result)

assert tuple(state.step for state in trajectory.states) == (1, 2)
assert not trajectory.coordinates.flags.writeable
for state in trajectory.states:
    state_evidence = evidence.state(state)
    print(state.step, state.time_s, state_evidence.solve)
```

`FixedMeshMonolithic` is keyword-only, immutable, and has no hidden numerical
defaults. Its initial state explicitly applies zero velocity everywhere and
the accepted displacement only at the free interface midpoint. The
model-bound `FixedMeshMonolithicPlan` exposes the admitted fixed-reference
geometry policy, affine-triangle spaces, backward-Euler time policy, monolithic
coupling, scales, symmetric-indefinite solver, tolerances, backend, execution
adapter, worker count, and state count before a worker starts. Resolution
rejects every unsupported value and foreign Model meaning rather than falling
back.

The common `Result` returns the common `Trajectory`, which is the sole Python
owner of the exact Model, Geometry, correspondence, Mesh, Realization, Run,
ordered-state, and trajectory identities as well as the fixed reference
coordinates, connectivity, and spatial fields. `FixedMeshMonolithicEvidence`
owns the exhaustive fluid/solid/interface partition. Its exact
`TrajectoryState` lookup returns the corresponding action, energy, residual,
solve, and assembly observations without turning the state into a
physics-specific property bag.

The optional Matplotlib adapters select an exact Model-bound Field from an
already accepted trajectory state. They restrict both values and topology to
the Field's accepted support; deformation additionally requires a
spatial-cartesian vector with the SI dimension of length:

```python
pressure_figure = eqplot.plot_scalar_field(
    trajectory,
    step=2,
    field=model.field("fluid_pressure"),
)
pressure_figure.savefig("fixed-reference-pressure.png")

deformed_figure = eqplot.plot_deformed_field(
    trajectory,
    step=2,
    field=model.field("solid_displacement"),
    scale=12,
)
deformed_figure.savefig("fixed-reference-deformed.png")
```

The complete runnable workflow is
[`examples/python/fixed_reference_fsi.py`](../../examples/python/fixed_reference_fsi.py).
It is one immutable fixed-reference 2D, affine-triangle, host-serial, two-step
composition. It does not expose a general coupling graph, Python time loop,
ALE or remeshing, partitioned iteration, stress/drag/lift derivation,
animation, or scientific validation from pixels. Field names above are
resolved by the caller's exact `Model`; the presentation adapters receive
`FieldRef` values and never use names as field identity.

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

## Exact revisions and current replay

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
Stale or foreign plans produce no partial child. Ordinary authoring, edits,
and replay all use the single current artifact contract:

```python
replayed = eqiora.replay(child.to_json())
assert replayed == child
```

The canonical bytes still expose the persisted
`eqiora.model-envelope/v8` schema, but callers do not select that suffix.
Model v1--v7 bytes reject; replay never sniffs, retries, or silently migrates
an older artifact.

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
