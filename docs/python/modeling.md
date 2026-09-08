# Modeling and realization

## Native declarations

Python declarations are immutable inputs to the same typed draft, validation,
transaction, and canonical artifact path used by other Eqiora clients.
Python does not implement a second model semantics.

```python
import eqiora

x = eqiora.Field("x", role=eqiora.FieldRole.State)
rate = eqiora.Parameter(
    "rate",
    value=1.0,
    value_type=eqiora.ValueType.real(eqiora.Dimension(time=-1)),
)
flow = eqiora.Relation(
    "flow",
    equations=((eqiora.derivative(x) + rate * x, 0),),
)
model = eqiora.Model.define("decay", x, rate, eqiora.Initial((x, 1)), flow)
```

`Field.value_type` holds its mathematical scalar domain, physical dimension and
component roles. `FieldRole.Variable` declares an algebraic unknown;
`FieldRole.State` declares evolution or history independently of support.
Omitted types are dimensionless real scalars. `Initial` supplies simultaneous
ordered left/right equation pairs for fresh initialization; the Field stores no initial literal.
Fresh scalar ODE and admitted index-one DAE initialization checks the initial and
regular equations together. Missing state data, contradictory constraints, or an
unsupported initialization profile reject; this is not a general high-index DAE
solver. Restart uses an accepted State/history without reapplying these equations.

```python
voltage = eqiora.ValueType.complex(eqiora.Dimension(mass=1, length=2, time=-3, current=-1))
body = eqiora.Domain.box("body", (0.0, 1.0), (0.0, 1.0))
channels = eqiora.Field(
    "channels",
    role=eqiora.FieldRole.Variable,
    domain=body,
    value_type=eqiora.ValueType.array(eqiora.ValueType.vector(voltage, 2), 3),
)
```

`ValueType.real(dimension)` and `ValueType.complex(dimension)` construct scalars.
`vector(scalar, extent)` and `tensor(scalar, *extents)` introduce spatial axes;
`array(element, extent)` adds a channel axis without changing the element's frame.
Equal component counts do not make these types interchangeable. Spatial extents
must match the Field's exact Domain. Complex execution is still under development.

Dimensions accept exact rational exponents, for example
`eqiora.Dimension(length=Fraction(-1, 2))` with `Fraction` imported from `fractions`.
Initial equations follow expression typing: nonzero dimensioned constants need
an explicit compatible quantity. A scalar is not broadcast into a vector, tensor,
or array initial state. Distributed execution retains its admitted explicit
initial-data owner; a declaration alone does not establish executable initialization.

The same `value_type=` objects apply to `Parameter`,
`eqiora.lang.Component.field`, and `eqiora.lang.Component.parameter`.
`ValueType.to_eqi()` emits the canonical type through the Rust formatter.

Parameters accept real or complex scalars and nested channel sequences matching the declared
shape. Inspection returns immutable nested tuples with every real/imaginary component:

```python
coefficients = eqiora.Parameter(
    "coefficients",
    value_type=eqiora.ValueType.array(eqiora.ValueType.complex(), 2),
    value=[1 + 2j, 3 - 4j],
)
assert coefficients.value == (1 + 2j, 3 - 4j)
selected = coefficients[1]
```

Nonzero spatial coefficients carry an explicit frame context from an existing
Domain. Their values remain uniform Parameters:

```python
body = eqiora.Domain.box("body", (0, 1), (0, 1))
kind = eqiora.ValueType.tensor(eqiora.ValueType.real(), 2, 2)
coefficient = eqiora.Parameter(
    "coefficient", value_type=kind, value=((2, 3), (5, 7)), frame=body,
)
response = eqiora.Field(
    "response", role=eqiora.FieldRole.Variable, domain=body, value_type=kind,
)
law = eqiora.Relation("law", equations=((response, coefficient),), domain=body)
model = eqiora.Model.define("Coefficients", body, coefficient, response, law)
assert model.parameter("coefficient").value == ((2, 3), (5, 7))
```

Include the frame Domain in `Model.define`; foreign or omitted declarations reject.
The Domain supplies the model-global Cartesian frame and ambient dimension, not
Parameter support. Values retain real/imaginary components and axis order through
inspection, edits, and replay. A channel array of tensors remains distinct from one
spatial tensor. Python Source uses the same constructor as
`q.tensor_value(frame=body, components=((2, 3), (5, 7)))`, where `body` is an exact
`Component.volume` or `Component.boundary` handle from that Component. The resulting
expression can supply a Parameter default through `Component.set_default`. Constructor
components must be closed scalar expressions; referencing a named model value, including
a Parameter alias, inside the constructor rejects.

Indices are static exact nonnegative integers; mutable Parameters cannot supply indices.
Typed value edits preserve the complete declared type and all components through replay.
This authoring support does not establish a complex numerical solver.

Declare `value_type=eqiora.ValueType.integer()` to retain Python integers exactly,
including values above `2**53`. Values must be signed 64-bit integers; booleans,
floating-point values and overflow reject. Without an explicit integer type,
ordinary numeric defaults keep their real interpretation. `ParameterRef.value`
returns the exact typed value, including after edits and Model replay.

```python
species = eqiora.FiniteSpace("Species", labels=("A", "B"))
population = eqiora.Parameter(
    "population", value_type=eqiora.ValueType.counts(species),
    value=(2, 9007199254740993),
)
observed = eqiora.Field("observed", role=eqiora.FieldRole.Variable)
relation = eqiora.Relation("observation", equations=((observed, 0),))
model = eqiora.Model.define("Population", species, population, observed, relation)
assert model.parameter("population").value == (2, 9007199254740993)
```

`ValueType.coordinates(species)` holds signed integer components in the same
ordered basis; `counts(species)` requires nonnegative components. Equal labels in
another `FiniteSpace` do not establish the same type. `IndexSet("Rows", extent=3)`
and `ValueType.index(rows)` similarly retain a distinct nominal identity and admit
only ordinals from zero through two. Include each native declaration in
`Model.define`. Nominal types need their declaration's lexical scope for source
rendering, so their standalone `to_eqi()` rejects.

Python Source registers spaces with `source.space(...)` and constant-sized sets
with `component.index_set(..., extent=3)`. Its `counts`, `coordinates`, and `index`
constructors require handles from the owning Source or Component. The closed
`eqiora.lang.quotient`, `remainder`, `to_real`, `to_integer`, and `ordinal`
expressions use the shared compiler's explicit conversion and arithmetic rules.
Products, dual spaces, general maps, and dynamic indexing remain
outside this bounded discrete profile.

Finite scalar reductions bind one symbolic index through `Component.sum` or `Component.product`:

```python
rows = component.index_set("Rows", extent=3)
total = component.sum(lambda i: (q.ordinal(i) + 1) ** 2, over=rows)
component.let_alias("total", total)
```

Here `q` is `eqiora.lang`. The callback runs once to author the body; the compiler expands
its three terms and obtains 14. A nested reduction needs a distinct `name`, such as
`name="j"`. Binders cannot escape their callback or capture another declaration, and the
set and captured declarations must belong to the same Component. An array expression accepts
`values[q.ordinal(i)]` within this scope. General runtime indexing remains unsupported.
The [finite reduction rules](../language/numeric-catalog.md#finite-sums-and-products) define
ordering, scalar types, product units, expansion bounds and unsupported initializer contexts.

A numeric Parameter default uses the declared dimension's coherent unit.
For example, `parameter rate: 1 / s = 1;` gives the same value as
`parameter rate: 1 / s = 1[1 / s];`. Explicit input units still express compatible
conversions. This context applies only to numeric Parameter defaults;
nonzero literals in general expressions do not silently acquire units.

A native Relation receives ordered `equations=((left, right), ...)` pairs.
Use `(residual, 0)` for a numerical residual equation. Named `equal`, `not_equal`,
`less`, `less_equal`, `greater`, and `greater_equal` functions produce predicates;
`logical_not`, `logical_and`, and `logical_or` compose them. Python `==` retains
handle identity, and symbolic Python truth testing rejects. Declarations and expressions
are frozen; validation and artifact creation happen atomically in Rust.

## Author Eqiora Language source

`eqiora.lang.Source` is the equations-language route when a workflow should be
fully Python-authored without creating a second equation semantics:

```python
import eqiora
from eqiora import lang as q
from eqiora import units as u

source = q.Source()
component = source.component("Diffusion")
body = component.volume("body", dimensions=2)
value = component.field("value", on=body, role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
length = component.parameter("length", value_type=eqiora.ValueType.real(eqiora.Dimension(length=1)))
wave_number = component.let_alias("wave_number", q.math.pi / length)
component.relation(
    "balance",
    on=body,
    left=q.div(q.grad(value)),
    right=-(wave_number**2) * value,
)

text = source.to_eqi()
model = eqiora.compile(
    source=source,
    entry="Diffusion",
    geometry=geometry,
    bindings={"body": geometry.selection("body"), "length": 1.0},
)
```

Source Relations require an ordered `left=` and `right=` pair, including an
explicit `right=0` for a residual equation. They emit `left = right;`; Python `==` is
not overloaded. The Source draft owns exact supports and expressions, rejects
foreign handles and resource-limit violations, and freezes on its first emission or compile.
It emits ordinary readable UTF-8 `.eqi`; `doc=` values become attached `///`
documentation. A blank paragraph is emitted as an empty `///` line, keeping the
block attached to its declaration. Documentation is bounded to 16,384 UTF-8 bytes.
`write_eqi(path)` uses same-directory staging and atomic replacement, so an I/O
failure does not publish a partly written source file.

A Source can contain multiple Components within its existing declaration bound.
Use `parent.instance(...)` to bind a child's requirements explicitly, and select the
entry with `eqiora.compile(source=source, entry="Parent", ...)` when the source
contains multiple public Components. A Source containing property contracts still
requires the exact Model Package compilation path described below.

`component.let_alias(name, expression)` declares a private immutable expression alias.
Its type and spatial support are inferred; `value_type=` asserts the inferred type,
and `on=` asserts the exact inferred Support owned by this Component.
Aliases can use the component's Parameters, fields, and other aliases. For example,
`heat_flux = component.let_alias("heat_flux", coefficient * q.grad(potential))`
can appear in a volume relation as `q.div(heat_flux)`. A Parameter-only alias can also
supply a nested-instance Parameter argument; a field-dependent alias cannot.

Aliases do not become required parameters, independent edit targets, unknowns, or equations.
Each occurrence retains the original dependencies, so Parameter edits and differentiation
pass through the expression. Expressions must have an intrinsically inferable spatial support:
keep context-dependent `coordinate`, `trace`, and `normal` expressions in their relations.
Writing `on=` cannot give a constant spatial support or select an unspecified boundary.
An equal-shaped, separately declared Support is still a different nominal support.
`at=clock` asserts that the expression's runtime dependencies belong to that exact
Component-owned Clock. Equal periods do not make different Clocks interchangeable.
Static expressions and mixtures of continuous and clocked dependencies cannot assert a
single clock. State operators retain their exact-clock and initialization rules through
aliases; reading a current state through an alias adds no clock restriction.

Create nominal periodic clocks with exact seconds, using an integer or `fractions.Fraction`:

```python
from fractions import Fraction

tick = component.clock("tick", period_s=Fraction(1, 10))
memory = component.field(
    "memory", on=body, role=eqiora.FieldRole.State,
    value_type=eqiora.ValueType.real(), at=tick,
)
component.initial(left=q.pre(memory), right=1)
observed = component.let_alias("observed", memory, on=body, at=tick)
component.relation(
    "update", on=body, at=tick, left=q.next(memory), right=q.pre(memory),
)
```

The optional `phase_s` defaults to zero. Initial equations are simultaneous, and the
clock's first tick follows initialization. Clock handles belong to their declaring
Component; foreign handles are rejected before changing a declaration. `q.pre` and
`q.next` use the same Rust state-role and use-context checks as emitted source.
This authoring path does not extend the execution backends' admitted spatial time models.

Source values do not type-check or lower equations in Python. Direct compile
materializes `source.to_eqi()` and enters the same Rust parser, type checker,
lowerer, Geometry/support binder, and compiler used by a file path. Consequently,
direct and emitted-file compilation with identical bindings have the same Model
meaning and identity. Prose changes affect source bytes and package source-bundle
identity, not the physical Model. Compiler failures retain the existing structured
diagnostics.
`q.math.pi` is one immutable, ownerless Source expression that emits exactly
`math.pi`; `q.math.sin(expression)` emits the matching compiler-owned scalar
operation. Composing either with a Source-owned expression adopts that Source's
existing ownership. The top-level Source vocabulary is reserved for equation
structure such as `q.grad` and `q.div`, while scalar functions and constants
live under `q.math`. They are not Python numerical operations, and the native
compiler remains the authority for their typing and value semantics.

The same Source owner can emit the bounded constant property declarations used by
an exact Model Package:

```python
source = q.Source()
contract = source.property_contract("Diffusivity", value_type=eqiora.ValueType.real())
release = source.property_release(
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
value = law.field("value", on=law_body, role=eqiora.FieldRole.Variable, value_type=eqiora.ValueType.real())
law.relation(
    "balance",
    on=law_body,
    left=-q.div(diffusivity * q.grad(value)),
    right=0,
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
    bindings={law_body: root_body, diffusivity: material["diffusivity"]},
)
source.write_eqi("src/property-diffusion.eqi")
```

The composition mapping may contain several releases. Each instance binding
selects a member explicitly, such as `diffusivity = ReferenceMaterial.diffusivity`.
Compilation checks that every required property is supplied exactly once and
that each release implements the required nominal contract.

Contracts and releases authored in the same Source can compile locally through
`eqiora.compile(source=source, entry=..., bindings=...)`. The shared Rust compiler
checks each exact release or composition member against its nominal contract;
a same-spelled handle from another Source is rejected before emission.

To retain exact package provenance, emit the `.eqi` into a Model Package, lock
it, and use `compile_package`. That route exposes the existing immutable
`property_bindings` inspection. Local compilation does not synthesize package
lineage, and Python does not normalize or evaluate the property itself.

The complete current vocabulary and steady-cylinder Component are shown in
[`examples/python/steady_cylinder_source.py`](../../examples/python/steady_cylinder_source.py).
The baseline slice has one public Component, public volume/parent-boundary
supports and parameters, typed scalar, spatial-vector/tensor and channel-array
continuum fields, continuous residual Relations, structural SI units, constants,
coordinates, arithmetic,
powers, gradient, divergence, trace, normal contraction, symmetric part, and
isotropic lift. The package-oriented extension admits multiple scalar contracts
and constant releases, one material composition, one consumer plus one root
Component, and complete direct or composed bindings.

## Resolve and lock a local package project

An installed Eqiora distribution can add an exact standard fluid or solid
dependency to an existing project through the same manifest/lock transaction:

```python
import eqiora

resolution = eqiora.add_bundled_dependency(
    ".", "package-store", "Eqiora.Fluid.Incompressible", version="0.4.0"
)
```

Create the store directory first. The request must match the exact release
shipped in the distribution. The solid package is
`Eqiora.Solid.LinearElasticity`, version `0.6.0`.
The manifest records `version = "0.4.0"` and `bundled = true` for the fluid
dependency; local dependencies instead record an explicit `path`.

`eqiora.toml` is the author-maintained project and package manifest. It owns the
canonical name, exact version, source root, entry module, and direct
dependencies:

```toml
[package]
name = "org.example.application"
version = "0.1.0"
source = "src"
entry = "models.main"

[dependencies."org.example.materials"]
version = "1.0.0"
path = "packages/materials"

[dependencies."org.example.components"]
version = "2.1.0"
path = "packages/components"
```

Each dependency directory contains its own `eqiora.toml`. Dependency paths are
relative to the declaring manifest; `../library` explicitly selects a sibling
outside the project directory. Absolute paths and parent segments after named
directories are rejected. Each package's source root remains confined to that
package directory. Package names, not local aliases, authorize imports.

`entry` selects a module relative to the source root: `models.main` selects
`src/models/main.eqi` with the default source root. The generated package
manifest retains this selection for offline compilation. `entry_model` selects
a Model in that module or through one of its explicit imports.

Python resolves that project into the store and atomically writes its current
project lock to `eqiora.lock`:

```python
from pathlib import Path

import eqiora

store_root = Path("package-store")
store_root.mkdir()
resolution = eqiora.resolve_local_project(".", store_root)
assert eqiora.open_project(".", store_root) == resolution

model = eqiora.compile_package(
    store_root,
    resolution,
    entry="materials.Calibration",
)
```

To move the project offline, create a destination directory and copy the accepted
closure with `eqiora.vendor_project(".", store_root, "vendor")`. After moving the
project, `eqiora.open_project(".", "vendor")` returns the validated resolution for
`compile_package`. Reopening checks current root sources and every locked package
without reading external dependency paths or selecting bundled releases.

`fetch_project` fills a store from the explicit source requests only if they still
match the accepted lock. `update_project` explicitly re-derives that lock from the
current sources. Compilation and reopening never update requests or fetch packages.
The CLI uses the same operations: `package add --bundled`, `package fetch`,
`package update`, `package vendor --destination`, and `package check`, each with
the project path and `--store`.

### Explicit Git sources

On Linux, add a public HTTPS repository or an explicit absolute/`./`/`../` local
repository path through the same project owner:

```python
resolution = eqiora.add_git_dependency(
    ".", "package-store", "org.example.Materials", version="1.0.0",
    repository="https://example.org/materials.git", revision="refs/heads/main",
)
```

The CLI equivalent is `eqiora package add . org.example.Materials --version 1.0.0
--git https://example.org/materials.git --rev refs/heads/main --store package-store`.
Revisions are lowercase full 40-digit commit IDs or explicit `refs/heads/...` /
`refs/tags/...` names. Arbitrary revision expressions are rejected.

`eqiora.lock` is a project envelope containing the exact semantic resolution and
immutable Git commit selections. API return bytes remain the semantic resolution
accepted by `compile_package`; they are not the entire project lock. `fetch_project`
retains the accepted commit even if its branch moves. `update_project` explicitly
resolves the authored request again. `open_project`, compile and run never invoke Git.

Git acquisition requires `/usr/bin/git`, `/usr/bin/prlimit` and home-backed `TMPDIR`.
Each acquisition has a 90-second deadline; Git runs with 1 GiB address space,
64 MiB per-file storage, 60 CPU seconds, 64 open descriptors and bounded output.
Admission limits are 64 MiB stored inventory / 10,000 entries, 4,096 source files,
32 directory levels, 8 MiB per expanded blob and 32 MiB total source bytes.
One project acquires at most 16 repositories with nesting depth 8.

There is no checkout, hook/filter execution or submodule acquisition. Local repository
configuration and object alternates are not accepted. Git tree symlinks, gitlinks,
path escapes and conflicting paths fail before publication. Fetched package-local
dependencies stay inside the fetched tree; fetched packages cannot select ambient
local Git repositories. HTTPS is unauthenticated, with redirects and credential
helpers disabled; userinfo, query strings and fragments are rejected. Unsupported
containment environments fail instead of running an uncontained fetch.

The shared Rust owner opens manifest-relative paths without following symbolic
links, discovers bounded `.eqi` inventories, generates each closed package
manifest, prepares the exact graph leaf-first, and publishes the lock only
after the complete closure is installed.
An optional package-root `README.md` is retained as documentation in the exact
source bundle; it is never compiled as model source.

Use `eqiora.add_local_dependency(project_root, store_root, name, version="1.0.0",
path="packages/library")` to add or replace a direct dependency, and
`eqiora.remove_local_dependency(project_root, store_root, name)` to remove it.
Both return the new semantic resolution bytes. The complete candidate is validated before the
manifest and lock are published; a failed update preserves the accepted pair.
Remove source imports before removing a dependency they require.

The CLI uses the same operations:

```bash
eqiora package lock . --store package-store
eqiora package add . org.example.Library --version 1.0.0 --path packages/library --store package-store
eqiora package remove . org.example.Library --store package-store
eqiora package check . --store package-store --entry-model Main
```

If a process stops during publication, locked compilation reads the previous
accepted lock. The next explicit update recovers the saved pair before resolving.
Concurrent project writes are rejected; retry after the other operation finishes.

## Compile one exact locked package Model or Component

Python can bind an existing content-addressed package's public Component to
caller-owned Geometry and produce the same ordinary immutable `Model` used by
local source compilation. Here `support_bindings` explicitly maps every support
name in the selected signature to a Geometry selection; each boundary maps to
`(boundary_selection, parent_selection)`:

```python
from pathlib import Path

import eqiora

store_root = Path("package-store")
resolution = Path("resolution.canonical.json").read_bytes()
model = eqiora.compile_package(
    store_root,
    resolution,
    geometry=geometry,
    entry="PoissonRectangle",
    bindings={**support_bindings, "wave_number": 3.14159, "source_scale": 19.7392},
)

print(model.digest)
print(model.package_compilation_digest)
for binding in model.property_bindings:
    print(binding.contract, binding.release, binding.normalized_value)
    print(binding.validity, binding.citation, binding.license)
```

The caller selects one explicit store directory and supplies the exact bytes
from `ResolutionRecordV1.canonical_json()`. The required `entry=` names the
selected public Model or Component. `bindings=` explicitly supplies its required
signature inputs, with `geometry=` authenticating any Geometry selections. Rust
verifies the complete locked closure and uses the same compiler-owned graph
for both declaration kinds.
Human-formatted, reordered, newline-terminated, duplicate-key, or
store-mismatched resolution bytes fail closed. Missing or ambiguous support
bindings fail instead of matching Geometry by bounds, coordinates, or digest.

`package_compilation_digest` is read-only lineage for the accepted compilation.
When the package binds an exact typed constant property release, `property_bindings` is
an immutable projection of the compiler-owned optional composition, contract, release, consuming
Component, requirement, complete value type and coherent-SI value, validity, citation, and license. It
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
    entry="SteadyFlowPastCylinder",
    geometry=geometry,
    bindings={
        "fluid": geometry.selection("fluid"),
        **{
            name: (geometry.selection(name), geometry.selection("fluid"))
            for name in ("inlet", "outlet", "walls", "cylinder")
        },
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
uv venv --python 3.13 .venv
uv pip install --python .venv/bin/python '.[gmsh,matplotlib]'
uv run --no-project --python .venv/bin/python examples/python/exact_cylinder_stokes.py \
  --pressure-png exact-cylinder-pressure.png
```

These commands build the checked-out source with its matching example. The
equivalent composition API is:

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
    entry="MixedBoundaryElasticity2d",
    geometry=geometry,
    bindings={
        "body": geometry.selection("body"),
        **{
            name: (geometry.selection(name), geometry.selection("body"))
            for name in ("x_lower", "x_upper", "y_lower", "y_upper")
        },
        "mu": 3.0, "lambda": 0.0, "length_scale": 1.0,
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
    entry="FixedReferenceFsi2d",
    bindings={
        **parameters,
        **{region: geometry.selection(region) for region in ("fluid", "solid")},
        **{
            f"{region}_{side}": (
                geometry.selection(f"{region}_{side}"), geometry.selection(region)
            )
            for region in ("fluid", "solid")
            for side in ("x_lower", "x_upper", "y_lower", "y_upper")
        },
    },
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
    across_type=eqiora.ValueType.real(voltage),
    through_type=eqiora.ValueType.real(current),
)
left = eqiora.ConservingPort("left", domain=electrical)
right = eqiora.ConservingPort("right", domain=electrical)
component = eqiora.Relation(
    "component",
    equations=((eqiora.across(left), 0), (eqiora.through(right), 0)),
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

Domain, boundary, Field support, and Relation support are
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
potential = eqiora.Field(
    "potential",
    role=eqiora.FieldRole.Variable,
    domain=interval,
)
source = eqiora.Parameter(
    "source",
    value=1.0,
    value_type=eqiora.ValueType.real(eqiora.Dimension(length=-2)),
)
model = eqiora.Model.define(
    "poisson",
    interval,
    lower,
    upper,
    potential,
    source,
    eqiora.Relation(
        "balance",
        domain=interval,
        equations=((-eqiora.div(eqiora.grad(potential)) - source, 0),),
    ),
    eqiora.Relation(
        "lower_value",
        domain=lower,
        equations=((eqiora.trace(potential), 0),),
    ),
    eqiora.Relation(
        "upper_value",
        domain=upper,
        equations=((eqiora.trace(potential), 0),),
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
`eqiora.model-envelope/v17` schema, but callers do not select that suffix.
`.eqi` remains source text; `.eqmodel` is the canonical compiled Model artifact.
Only the current schema is accepted; decoding never sniffs, retries, or silently
migrates an older artifact.

Independent definitions allocate fresh canonical occurrence identities, so
exact equality and digest equality are intentionally stronger than structural
comparison:

```python
source_model = eqiora.compile(
    source="""
    model decay(parameter rate: 1 / s = 1) {
      state x: 1;
      initial { x = 1; }
      relation flow {
        derivative(x) + rate * x = 0;
      }
    }
    """
)
x = eqiora.Field("x", role=eqiora.FieldRole.State)
rate = eqiora.Parameter(
    "rate",
    value=1.0,
    value_type=eqiora.ValueType.real(eqiora.Dimension(time=-1)),
)
native_model = eqiora.Model.define(
    "decay",
    x,
    rate,
    eqiora.Initial((x, 1)),
    eqiora.Relation(
        "flow",
        equations=((eqiora.derivative(x) + rate * x, 0),),
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


Quantity inputs use the compiler-owned `eqiora.units` catalog. For example,
`q.quantity(Decimal("998.2"), u.kg / u.m**3)` preserves the exact decimal input
until compiler normalization. Import `Decimal` from Python's `decimal` module.
Integer inputs retain their decimal digits. Float inputs use Python's shortest
round-trip decimal spelling; this is a source-authoring policy, not a claim of
exact binary-ratio rescaling. Native numerical inputs remain binary64 values in
coherent SI. Quantity literal spellings are limited to 256 bytes.
