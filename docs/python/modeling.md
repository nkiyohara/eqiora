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
operations, multiple holes, selection handles, meshing, Model binding, solve,
Result, and visualization remain separate slices.

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
