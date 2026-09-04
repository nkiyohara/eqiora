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

## Author Eqiora Language source

`eqiora.lang.Source` is the equations-language route when a workflow should be
fully Python-authored without creating a second equation semantics:

```python
import eqiora
from eqiora import lang as q
from eqiora.lang import units as u

source = q.Source()
component = source.component("Diffusion")
body = component.volume("body", dimensions=2)
value = component.field("value", on=body, unit=u.m)
length = component.parameter("length", unit=u.m)
wave_number = q.math.pi / length
component.relation(
    "balance",
    on=body,
    left=q.div(q.grad(value)),
    right=-(wave_number**2) * value,
)

text = source.to_eqi()
model = eqiora.compile(source=source, geometry=geometry, parameters={"length": 1.0})
```

Relations accept either `residual=` or a complete `left=` and `right=` pair.
The latter emits the ordinary Eqiora equation `left = right;`; Python `==` is
not overloaded. The Source draft owns exact supports and expressions, rejects
foreign handles and resource-limit violations, and freezes on its first emission or compile.
It emits ordinary readable UTF-8 `.eqi`; `doc=` values become `//` comments.
`write_eqi(path)` uses same-directory staging and atomic replacement, so an I/O
failure does not publish a partly written source file.

Source values do not type-check or lower equations in Python. Direct compile
materializes `source.to_eqi()` and enters the same Rust parser, type checker,
lowerer, Geometry/support binder, and compiler used by a file path. Consequently,
direct and emitted-file compilation with identical bindings have the same Model
meaning and identity, while comments affect source presentation but not semantic
identity. Compiler failures retain the existing structured diagnostics.
`q.math.pi` is one immutable, ownerless Source expression that emits exactly
`math.pi`; `q.math.sin(expression)` emits the matching compiler-owned scalar
operation. Composing either with a Source-owned expression adopts that Source's
existing ownership. The top-level Source vocabulary is reserved for equation
structure such as `q.grad` and `q.div`, while scalar functions and constants
live under `q.math`. They are not Python numerical operations, and the native
compiler remains the authority for their typing and value semantics.

The same Source owner can emit the bounded scalar property declarations used by
an exact Model Package:

```python
source = q.Source()
contract = source.scalar_property_contract("Diffusivity", unit=u.one)
release = source.scalar_property_release(
    "ReferenceDiffusivity",
    implements=contract,
    value=25,
    source_unit=u.one,
    source_scale=0.001,
    citation="org.example.measurement",
    license="spdx.CC0_1_0",
)
law = source.component("DiffusionLaw")
law_body = law.volume("body", dimensions=2)
diffusivity = law.property("diffusivity", contract=contract)
value = law.field("value", on=law_body, unit=u.one, initial=0)
law.relation(
    "balance",
    on=law_body,
    residual=-q.div(diffusivity * q.grad(value)),
)
material = source.material_composition(
    "ReferenceMaterial",
    properties={"diffusivity": release},
)
root = source.component("DiffusionProblem")
root_body = root.volume("body", dimensions=2)
root.instance(
    "equation",
    component=law,
    supports={law_body: root_body},
    parameters={},
    material=material,
)
source.write_eqi("src/property-diffusion.eqi")
```

The composition mapping may contain several releases. The emitted instance uses
`material = ReferenceMaterial`; compilation checks
that the composition supplies every Component property exactly once and that
each release implements the required nominal contract.

Property contracts and releases are package-nominal. Passing this Source to
ordinary `eqiora.compile(source=...)` therefore fails with a focused
`SourceError`: that path has no exact package namespace. Emit the `.eqi` into an
exact Model Package and use `compile_package` after the package has been locked.
Python does not synthesize package lineage, normalize the release, or evaluate
the property. The locked Rust compiler remains the owner, and its resulting
Model exposes the existing immutable `property_bindings` inspection.

The complete current vocabulary and steady-cylinder Component are shown in
[`examples/python/steady_cylinder_source.py`](../../examples/python/steady_cylinder_source.py).
The baseline slice has one public Component, public volume/parent-boundary
supports and parameters, scalar or spatial-vector continuum fields, continuous
residual Relations, structural SI units, constants, coordinates, arithmetic,
powers, gradient, divergence, trace, normal contraction, symmetric part, and
isotropic lift. The package-oriented extension admits multiple scalar contracts
and constant releases, one material composition, one consumer plus one root
Component, and complete direct or composed bindings.

## Resolve and lock a local package project

An installed Eqiora distribution can vendor the standard fluid or solid
package, including its exact dependency closure, into a project:

```python
import eqiora

packages = eqiora.vendor_standard_package(".", "Eqiora.Fluid@0.2.0")
fluid = next(package for package in packages if package.name == "Eqiora.Fluid")
print(fluid.path, fluid.semantic_digest)
```

Each returned `VendoredStandardPackage` provides the exact identity needed by
an application package manifest and the project-relative path for a
`[sources.*].path` entry.
Calling the function again is idempotent when every file is unchanged and
rejects a changed destination without overwriting it.

`eqiora.toml` maps short project names and root dependency aliases to contained
package directories:

```toml
schema = "eqiora.project.v1"
root = "application"

[dependencies]
materials = "materials"
components = "components"

[sources.application]
path = "packages/application"

[sources.materials]
path = "packages/materials"

[sources.components]
path = "packages/components"
```

Python resolves that project into the store and atomically writes the canonical
resolution to `eqiora.lock`:

```python
from pathlib import Path

import eqiora

store_root = Path("package-store")
store_root.mkdir()
resolution = eqiora.resolve_local_project(".", store_root)
assert Path("eqiora.lock").read_bytes() == resolution

model = eqiora.compile_package(
    store_root,
    resolution,
    entry_model="materials.Calibration",
)
```

The shared Rust owner opens project-relative paths without following symbolic
links, validates every bounded `package.json` inventory and dependency alias,
prepares the exact graph leaf-first, and publishes the lock only after the
complete closure is installed. A failed resolution leaves the previous lock
usable.

## Compile one exact locked package Model or Component

Python can bind an existing content-addressed package's public Component to
caller-owned Geometry and produce the same ordinary immutable `Model` used by
local source compilation:

```python
from pathlib import Path

import eqiora

store_root = Path("package-store")
resolution = Path("resolution.canonical.json").read_bytes()
model = eqiora.compile_package(
    store_root,
    resolution,
    geometry=geometry,
    component="PoissonRectangle",
    parameters={"wave_number": 3.14159, "source_scale": 19.7392},
)

print(model.digest)
print(model.package_compilation_digest)
for binding in model.property_bindings:
    print(binding.contract, binding.release, binding.normalized_value)
    print(binding.validity, binding.citation, binding.license)
```

The caller selects one explicit store directory and supplies the exact bytes
from `ResolutionRecordV1.canonical_json()`. Exactly one compile mode is selected:
`entry_model=` names a root-local or directly imported public Model, while
`geometry=` plus `component=` binds one root-package public Component to
caller-owned Geometry and optional parameter values. Rust verifies the complete
locked closure and uses the same compiler-owned graph in both modes.
Human-formatted, reordered, newline-terminated, duplicate-key, or
store-mismatched resolution bytes fail closed. Missing or ambiguous support
bindings fail instead of matching Geometry by bounds, coordinates, or digest.

`package_compilation_digest` is read-only lineage for the accepted compilation.
When the package binds an exact scalar property release, `property_bindings` is
an immutable projection of the compiler-owned optional composition, contract, release, consuming
Component, requirement, coherent-SI value, validity, citation, and license. It
is inspection metadata beside the compilation, not a second property evaluator.
The resulting `Model` enters ordinary `eqiora.resolve(model, mesh=..., ...)` and
`eqiora.run(plan)`; its `Plan` and `Run` retain the same digest. Bare Model JSON
still carries Model/Geometry meaning but not the package sidecar, so replayed
Models use the same resolver with `package_compilation_digest is None` and an
empty `property_bindings` tuple. Package lineage persistence belongs to the
symmetric Model artifact I/O work. This
surface does not discover stores or lock files, access registries or networks,
or add a Studio package workflow.

## Check one exact package structurally

An external package author can check the same locked closure without turning
the check into a scientific or execution claim:

```python
report = eqiora.check_package_conformance(
    store_root,
    resolution,
    entry_model="Main",
    profile="eqiora.package.structural-conformance-v1",
)

print(report.packages)
print(report.package_compilation_digest)
print(report.model_digest)
```

The read-only operation accepts one explicit store, exact canonical resolution
bytes, one bare root-local Model selector, and the exact profile token shown
above. It compiles and replays the closure twice through the existing package
and current Model boundaries, then returns immutable in-process facts only
after package-compilation and Model identity agree. Rejections raise the
existing structured `EqioraError` family and return no partial report.

This is structural compatibility only. The conformance fixture deliberately
includes scientifically false documentation that still passes: a report does
not establish physical truth, well-posedness, realizability, solver support,
accuracy, convergence, performance, or verified physics. It executes no
package code or tests and supplies no registry, discovery, installation,
publishing, signature, trust, badge, attestation, durable report wire,
scientific-evidence lookup, execution workflow, or Studio surface.

## Authored CAD to exact geometry

The first accepted path projects one closed authored-CAD history into its exact
transverse Geometry. Python names the two native-owned sketch inputs and does
not implement their operations:

```python
import eqiora

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

assert geometry.selection_dimension("fluid") == 2
assert geometry.selection_dimension("cylinder") == 1
print(geometry.digest)
```

Rust owns validation, graph binding, operation order, canonical ordering,
bytes, and exact handle identity. Every coordinate and radius is a coherent-SI
metre.
The same `GeometryGraph` owns solid authoring through
`graph.rectangle_extrusion(...)` and
`graph.circular_through_cut(solid, ...)`, producing the existing exact
canonical operations. The solid operation retains its explicit depth and CAD tolerances;
none enter the derived 2D Geometry, whose classification tolerance is supplied
separately. The circle remains centre-and-radius geometry, so chord count,
mesh size, and approximation tolerance cannot enter it. A general Sketch,
arbitrary planes or profiles, operation DAGs, general Booleans or sections,
multiple holes, Model binding, solve, Result, Studio, and visualization remain
separate slices. Installed Python exposes the common `Geometry` projection
only through the accepted authored graph; it does not publish a demo-shaped
constructor.

## Bounded Gmsh mesh

The matching meshing operation is an explicit typed provider choice:

```python
request = eqiora.meshing.GmshMesher(
    maximum_boundary_error=1e-4,
    maximum_target_size=0.05,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
plan = eqiora.meshing.resolve(geometry, request)
mesh = eqiora.meshing.generate(plan)

assert mesh.source_digest == geometry.digest
print(mesh.digest)
```

`resolve` is planning-only: it retains the exact source and derives the bounded
subdivision receipt directly from Geometry and policy without launching Gmsh or
constructing cells. `generate` then invokes exact Gmsh 4.15.2 once for that
call, admits its MSH 4.1 linear triangles, and derives
realized named selections through the geometry-to-mesh correspondence.
`maximum_target_size=None` leaves the global characteristic-size ceiling to the
provider; a finite positive value makes that ceiling caller-owned. The resolved
value and its automatic/explicit ownership are retained in production lineage.
It is a Gmsh characteristic target, not a guarantee on every realized edge.
`canonical_bytes` and `digest` identify only the accepted inner simplicial
mesh. The returned object retains source, correspondence, Mesh, and
provider-production identities. Missing, wrong-version, failed, or invalid
Gmsh output rejects without falling back to the retired spoke mesh.

This bounded operation supports the rectangle-with-circular-hole family and
affine 2D triangles. It does not add caller-owned MSH import, paths, fields,
multiple pieces, 3D, curved elements, repair, local or adaptive sizing, general Geometry
matching, fixed output counts, or cross-platform byte identity.

## Exact-cylinder steady Stokes result

The first fluid application keeps the component's equations, fields,
dimensions, Parameters, and abstract support names in the installed `.eqi`
source. Python is the sole owner of concrete shape and size. `compile` checks
that exact Geometry selections close the selected public Component, derives
Parameter dimensions from its declarations, and returns the ordinary immutable
`Model` used by every resolver:

```python
from importlib.resources import files

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

pressure = result.output(plan.capability.pressure)
pressure_values = pressure.values("vertex")
print(result.plan_key, pressure.coefficient_count("vertex"))
print(result.solve)
print(min(pressure_values), max(pressure_values))
force = result.boundary_force(geometry.selection("cylinder"))
inlet = result.boundary_flux(geometry.selection("inlet"))
outlet = result.boundary_flux(geometry.selection("outlet"))
print(force.on_domain, inlet.value + outlet.value)
```

Freshly compiled and replayed Models use the same root resolver. The source or
host path is not Model meaning; only the accepted source, concrete Geometry,
and values enter identity. `compile` is keyword-only and accepts exactly one of
`path=` or `source=`; `filename=` labels diagnostics only for `source=`.
The Plan exposes the exact spaces, scales, solver tuple, backend, placement,
and existing Realization bytes before a worker starts.
The common `Result` exposes immutable velocity and pressure `FieldOutput`
objects selected by exact Model-bound `FieldRef` values; each output retains
the paired common `Mesh`.
Field values and Mesh coordinates/connectivity lazily publish
read-only NumPy views in matching mesh order. Exact `GeometrySelection` values
select the supported boundary force and inlet/outlet flux observations directly
from the Result. `eqiora.fluid.steady_stokes_evidence(result)` remains an
optional verification projection over the same accepted observations.

This operation admits only the checked exact-cylinder component, Geometry,
mesh, MINI/P1 policy, and SparseLU request. It is not a general Model catalog,
arbitrary Geometry/component closure, or general CFD authoring. Velocity
projection, drag/lift, transient flow, and FSI remain separate slices. The
runnable file is
[`examples/python/exact_cylinder_stokes.py`](../../examples/python/exact_cylinder_stokes.py).

## Exact-cylinder pressure rendering

Install the Gmsh and Matplotlib adapters and ask the same runnable file to save
the accepted pressure field:

```console
python -m pip install 'eqiora[gmsh,matplotlib]'
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
helper currently renders scalar vertex or cell fields as still images.

## Mixed-boundary structural result

The installed package also carries the accepted mixed-boundary elasticity
source. Python compiles it through the current `Model` path, resolves an
explicit linear-elasticity intent before execution, and submits the resulting
model-bound Plan through the ordinary Run path:

```python
from importlib.resources import files

graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(
    rectangle,
    named_topology={
        "body": rectangle.region,
        "x_lower": rectangle.boundaries[0],
        "x_upper": rectangle.boundaries[1],
        "y_lower": rectangle.boundaries[2],
        "y_upper": rectangle.boundaries[3],
    },
)
mesh_plan = eqiora.meshing.resolve(
    geometry,
    eqiora.meshing.CartesianMesher(cells=(16, 16)),
)
mesh = eqiora.meshing.generate(mesh_plan)
model = eqiora.compile(
    path=files(eqiora).joinpath("examples", "mixed-boundary-elasticity.eqi"),
    geometry=geometry,
    parameters={"mu": 3.0, "lambda": 0.0, "length_scale": 1.0},
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
result = eqiora.run(plan)

displacement = result.output(plan.capability.displacement)
mesh = displacement.mesh
evidence = eqiora.solid.linear_elasticity_evidence(result)
```

The root `Plan` exposes the exact caller-owned mesh, Q1 spatial policy, linear
solver policy, backend, and execution placement before a worker starts.
Resolution admits only supported typed policy combinations and rejects other
values instead of silently falling back.

The common `Result` owns one immutable vector `FieldOutput` selected by the
Plan's exact Model-bound `FieldRef`; `displacement.mesh` is its paired exact
caller-generated `Mesh`. Output values, Mesh coordinates,
and Q1 connectivity lazily publish memoized, read-only NumPy views in one
co-indexed canonical order. The typed elasticity observation keeps the
Plan identity, reference-CG solve summary, assembly counts, constrained reaction,
integrated body force, and exact bounds outside the common result transport.
Model, Geometry, correspondence, Mesh, Plan, and Result identity remain
Rust-owned and relationally exact. Stress, strain, traction recovery,
analytic error, other meshes, and general structural solving are not implied.

The optional still displays original and explicitly scaled deformed edges:

```python
import eqiora.matplotlib as eqplot

figure = eqplot.plot_deformed_field(
    result,
    field=plan.capability.displacement,
    scale=1.0,
)
figure.savefig("mixed-boundary-displacement.png")
```

The complete runnable workflow is
[`examples/python/mixed_boundary_elasticity.py`](../../examples/python/mixed_boundary_elasticity.py).

## Fixed-mesh monolithic FSI result

The fixed-reference FSI path uses the same root lifecycle as every common
numerical Plan. Python authors the adjacent two-region `Geometry`, generates
its authenticated common `Mesh`, compiles the equations-only Component, and
then supplies exact Model-bound spatial scopes:

```python
model = eqiora.compile(
    path=files(eqiora).joinpath("examples", "fixed-reference-fsi.eqi"),
    geometry=geometry,
    component="FixedReferenceFsi2d",
    parameters=parameters,
)
plan = eqiora.resolve(
    model,
    mesh=mesh,
    spatial=(
        eqiora.fem.MiniP1().at(model.domain("fluid")),
        eqiora.fem.P1().at(model.domain("solid")),
    ),
    temporal=eqiora.time.BackwardEuler(step_s=0.05),
    solve=eqiora.solve.Linear(
        relative_tolerance=1.0e-11,
        absolute_tolerance=1.0e-13,
        maximum_iterations=20_000,
    ),
    scaling=None,
)
state = eqiora.State.initial(
    plan,
    time_s=0.0,
    fields=(
        eqiora.InitialField(model.field("fluid_velocity"), vertex_values=..., cell_values=...),
        eqiora.InitialField(model.field("fluid_pressure"), vertex_values=...),
        eqiora.InitialField(model.field("solid_velocity"), vertex_values=...),
        eqiora.InitialField(model.field("solid_displacement"), vertex_values=...),
    ),
)
result = eqiora.run(plan, state=state, steps=2, output_steps=(1, 2))
evidence = eqiora.fsi.evidence(result)
```

`DomainRef`, `InitialField`, `Plan`, `State`, `Run`, `Result`, and `Trajectory`
are common types. The Model decides that this is FSI; `eqiora.resolve` admits
only the complete `MiniP1@fluid + P1@solid` partition and binds the actual
Model, Geometry, Mesh, correspondence, production lineage, four exact Fields,
backward Euler policy, full coupled scaling receipt, MINRES provider, and host
placement. `scaling=None` requests automatic coupled scales; a complete
`IncompressibleScaling` value makes them manual.

Initial coefficients are immutable, exact-Field assignments in coherent SI.
They must be complete and association-correct; pressure has no auxiliary
zero-mean restriction in this fixed-reference formulation. A compatible State
can restart a freshly resolved Plan even when solve or scaling policies differ,
while a foreign Model, Geometry, field, or state space is rejected.

The complete runnable workflow is
[`examples/python/fixed_reference_fsi.py`](../../examples/python/fixed_reference_fsi.py).
It is one fixed-reference 2D affine-triangle monolithic formulation. It does not
claim partitioned coupling, FVM/FEM transfer, ALE, remeshing, checkpointing,
general multiphysics policy maps, or per-domain time and solve policies.

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

## Typed spatial Plan

Spatial execution uses the same root lifecycle as the examples above: author
one concrete Geometry, resolve a typed meshing provider, compile an
equations-only component with that Geometry, and call
`eqiora.resolve(model, mesh=..., spatial=..., solve=...)`. The returned common
`Plan` owns the exact Model, Mesh, and numerical policy identities; execution
accepts only `eqiora.run(plan)` or `eqiora.submit(plan)`. Specialized scalar
requests and model-plus-realization execution are absent.

The same resolved Plan can be moved as one exact local artifact without its
producer process:

```python
plan.write("case.eqplan")
portable = eqiora.Plan.read("case.eqplan")
result = eqiora.run(portable)
```

`.eqplan` contains exactly `plan.to_bytes()`. Reading re-resolves the Plan
against the locally admitted provider identities and rejects unknown,
noncanonical, oversized, non-regular, symlinked, or wrongly suffixed inputs.
Use it to move one exact Plan between local processes.

Complete Results and spatial Trajectories use the same exact, type-owned file
boundary. Reopening always requires the owning Plan:

```python
result.write("run.eqresult")
reopened = eqiora.Result.read(portable, "run.eqresult")

trajectory = reopened.trajectory
trajectory.write("run.eqtrajectory")
same_trajectory = eqiora.trajectory.Trajectory.read(
    portable, "run.eqtrajectory"
)
```

The files contain exactly `result.to_bytes()` and `trajectory.to_bytes()`.
For a dynamic Result, the Result remains the single complete root and owns its
Trajectory; the separate Trajectory file is an optional spatial projection,
not a second occurrence record. Process-local Runs, restart checkpoints,
archives, and cloud transport remain outside this boundary.

## Exact revisions and compiled Model files

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
and byte/file decoding all use the single current artifact contract:

```python
restored = eqiora.Model.from_bytes(child.to_bytes())
assert restored == child

child.write("child.eqmodel")
same = eqiora.Model.read("child.eqmodel")
assert same.revision == child.revision
```

The canonical bytes still expose the persisted
`eqiora.model-envelope/v8` schema, but callers do not select that suffix.
`.eqi` remains source text; `.eqmodel` is the canonical compiled Model artifact.
Model v1--v7 bytes reject; decoding never sniffs, retries, or silently migrates
an older artifact.

Independent definitions allocate fresh canonical occurrence identities, so
exact equality and digest equality are intentionally stronger than structural
comparison:

```python
source_model = eqiora.compile(
    source="""
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
