# RFC 0040: Occurrence-bound Field slots

- Status: Accepted; central elaboration and packaged-solid application verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0021](0021-component-hierarchy-and-instantiation.md),
  [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0034](0034-occurrence-bound-spatial-supports.md), and
  [RFC 0038](0038-canonical-tensor-structure-operators.md)

## Summary

A reusable Component may require an existing semantic Field through one
occurrence-bound, exactly typed Field slot. Every instance binds that slot to
one visible enclosing Field before deterministic flattening. The slot itself
creates no Field, node, edge, alias, or wire payload; Relations inside the
Component refer directly to the exact bound Field after expansion.

The bounded source contract is:

```text
public component IsotropicBalanceWithPotential2d {
  public support body: volume(ambient_dimension = 2);
  public field slot displacement on body as continuum:
    m shape spatial_vector;
  public field slot load_potential on body as continuum:
    kg / (m * s ^ 2);

  public parameter mu: kg / (m * s ^ 2);
  public parameter lambda: kg / (m * s ^ 2);

  relation balance continuous on body {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) - grad(load_potential) = 0;
  }
}

model Main {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  field load_potential on body as space: kg / (m * s ^ 2) = 0;

  instance law: solid.IsotropicBalanceWithPotential2d(
    support body = body,
    field displacement = displacement,
    field load_potential = load_potential,
    mu = 3,
    lambda = 2
  );
}
```

This is a hierarchy interface, not a tenth Semantic Kernel node. It is the
Field analogue of an occurrence-bound support slot, with the additional
requirement that the complete Field value type and its substituted exact
support agree.

## Motivation

Support slots let a package Relation state where it applies, but they do not
let several reusable laws refer to the same Field. Without a Field binding,
each Component must either own a separate Field or own the entire body,
constitutive law, forcing, and fixed boundary topology together. The first
duplicates unknowns; the second produces a closed benchmark fixture rather
than a composable physics library.

The missing operation is nominal and simple:

```text
definition obligation     displacement : Field<T, body_slot>
occurrence binding        displacement -> exact enclosing Field identity
flattened Relation        refers to that exact identity
```

It is not a numerical function-space binding. Mesh entities, basis functions,
quadrature, distributed layout, arrays, and device buffers remain Realization
or execution data and cannot satisfy the source contract.

## Closed source contract

### Field slot declaration

V1 accepts a Field slot only inside an ordinary Component:

```text
public field slot name on volume_support as continuum:
  physical_dimension [shape value_shape];
```

The declaration is always `public`, required, and initialization-free.
`public` means that an enclosing occurrence must bind the obligation; it does
not expose a mutable child Field through member selection. Private slots,
defaults, initial values, non-volume support, and a slot outside a Component
are rejected.

The explicit `slot` word distinguishes the obligation from an owned private
Component Field. Existing owned Fields retain their current syntax and
materialize one ordinary Kernel Field:

```text
field owned_state on body as local_space: K = 293.15;
```

The first slot representation family is the semantic `continuum` family. It
is written explicitly even though it is currently the only spatial
Representation so that adding another family cannot silently widen old slot
meaning. A target Field must use an admitted continuum Representation.

### Field binding

An instance argument has one separate closed form:

```text
field slot_name = target_name
```

Both names are bare identifiers. The slot must select one public Field slot on
the chosen Component. The target must select one visible Field in the
enclosing occurrence scope. It may be:

- a Model-owned Field;
- a private Field owned by the enclosing Component; or
- an enclosing Field slot already resolved to its own exact target.

Qualified child access, expression-valued targets, implicit name matching,
Connection inference, array views, callbacks, and defaults are not admitted.
Nested forwarding therefore preserves one Field identity rather than
constructing a chain of semantic aliases.

The `field` discriminator remains unambiguous with a scalar Parameter named
`field`: `field = value` is a Parameter binding, while
`field slot_name = target_name` is a Field binding.

## One identity-parametric Field contract

The compiler uses the existing expression type axes rather than introducing a
package-only type system:

```text
FieldContract<I> = {
  dimension: DimExponents,
  shape: ValueShape,
  frame: ValueFrame,
  support: SpatialSupport<I>,
  representation: Continuum
}
```

`I` changes while the rules do not:

```text
definition checking  I = SupportSlotName
occurrence checking  I = FullElaborationIdentity
kernel validation    I = DomainIdentity
```

Support bindings resolve first. The expected slot support is then substituted
with the exact occurrence support before comparison with the target Field.
Dimension, exact shape, frame, support kind, ambient dimension, exact volume
identity, and representation family must all agree. Equal geometry, equal
dimension, equal spelling, or equal values cannot substitute for exact support
identity.

`spatial_vector` specializes from the definition support's ambient dimension
during occurrence-independent checking. A two-dimensional slot is therefore
exactly `[2]` in `SpatialCartesian`, and a same-dimension invariant `[2]`
target remains incompatible.

The source compiler and Semantic Kernel validator continue to share the same
`ExpressionType` and `SpatialSupport` algebra. There is no parallel package
validator whose rules can drift.

## Validation and flattening

Validation is atomic and ordered:

1. index Component members and reject duplicate names;
2. validate support slots and Field-slot types without choosing occurrences;
3. validate Component Relations against Field slots as ordinary typed Field
   symbols;
4. resolve every instance support binding;
5. substitute those exact supports into the required Field contracts;
6. resolve and compare every Field binding;
7. insert each exact target Field only as a compiler-local child-scope alias;
8. expand Relations through the ordinary structural expression rewriter; and
9. construct a Transaction only after all occurrences have succeeded.

The Field slot never allocates:

- a Kernel Field or any other entity;
- a graph identity, display identity, symbol, or edge;
- an equality Relation between a slot and its target;
- a public `instance.slot` selection;
- a package, artifact, Model, Transaction, or Run wire payload; or
- a numerical degree of freedom, array, or transfer object.

After flattening, a Relation expression contains the exact internal name and
identity of the target Field. Existing semantic lowering, differentiation,
numerics, solvers, artifacts, and clients see only an ordinary flat Relation
network. No downstream layer can branch on a Field slot or package name.

## Identity, provenance, and limits

Field-slot declarations and bindings enter source identity as new closed,
sorted records. Declaration order, instance argument order, formatting,
source spans, file order, and map insertion order cannot change their
canonical identity. A different binding target is semantic and changes source
identity.

Legacy source identity remains byte-for-byte unchanged when a Component and
its instances contain no Field slot or binding. The new instance record is
encoded only when at least one Field binding exists.

Every expanded declaration already records the instance declaration and all
binding spans. Field-binding spans join that same sorted provenance set. They
explain how the target identity entered an occurrence without becoming part
of Model meaning or allocating an alias entity.

Parameter, support, and Field bindings share the existing per-instance total
binding budget. Field bindings cannot create a third unbounded argument list.
Existing identifier, member, expansion, depth, canonical-byte, source-byte,
and provenance limits remain independent. Counts use checked arithmetic and
all allocation follows bounded validation.

## Package and Realization boundary

An exact Model Package may place Field slots inside its existing Component
declarations. The package semantic digest sees the canonical Component text;
no new package declaration family or package wire version is needed. Package
aliases that resolve to the same exact package identity cannot change the
flattened Model.

The Semantic Model and package may own:

- typed Field obligations;
- constitutive, balance, source, and boundary Relations;
- material Parameters;
- support obligations; and
- typed physical Ports.

Realization continues to own mesh and boundary entities, basis and mixed
spaces, quadrature, stabilization, assembly, sparse layout, solver,
preconditioner, scheduling, partitioning, transfer, and device placement. A
Field binding can never carry any of those choices.

The first completed physics application is the public
`Eqiora.Solid.LinearElasticity.IsotropicBalanceWithPotential2d` Component. It
owns one two-dimensional volume support slot, displacement and load-potential
Field slots, the two Lamé Parameters, and the isotropic balance Relation. The
enclosing root owns the exact volume and boundary Domains, both continuum
Fields, the load-definition Relation, and every homogeneous displacement-trace
Relation. This deliberately leaves load and boundary closure composable while
preventing a physics package from capturing a numerical method.

## Prior art and deliberate differences

UFL's current form language distinguishes symbolic functions supplied later
from form arguments, and a Coefficient retains an exact function-space shape
and domain. This supports the decision to make an external Field a typed
symbolic obligation rather than an unchecked callback. Eqiora deliberately
binds a semantic continuum Field before discretization; it does not import
UFL's finite-element space into Model meaning.

- [UFL form language: Coefficient functions](https://docs.fenicsproject.org/ufl/main/manual/form_language.html#coefficient-functions)
- [UFL Coefficient API](https://docs.fenicsproject.org/ufl/main/_modules/ufl/coefficient.html)

Modelica 3.7 supplies useful prior art for public/protected class interfaces,
static name lookup, and flattening, while its connectors require structurally
matching named elements. Eqiora keeps the useful declaration discipline but
does not adopt public mutable child access, `inner`/`outer` lookup,
redeclare/replaceable semantics, expandable connectors, or ordered search
paths. A Field slot is an explicit occurrence obligation with one exact
target.

- [Modelica Language Specification 3.7](https://specification.modelica.org/maint/3.7/)
- [Modelica connectors and connections](https://specification.modelica.org/maint/3.7/connectors-and-connections.html)

## Alternatives considered

### Let a Component own every Field

This already works for closed components. It cannot compose several package
Relations over one Field without duplicating unknowns or making one Component
own the entire physical body and boundary topology. Retained as an ordinary
option, rejected as the only package composition mechanism.

### Make a public Component Field addressable

This creates a persistent child entity and makes external Relation meaning
depend on authoring hierarchy. It also weakens explicit-flat equivalence and
requires every downstream consumer to preserve public member lookup. Rejected.
The slot disappears before the Kernel.

### Add a PureOperator first

A pure constitutive map is mathematically valuable, but the current source
language has only unary built-in calls. A sound imported operator requires
qualified callees, multiple arguments, parameter and result types,
support/shape polymorphism, visibility, recursion rejection, capture-free
substitution, expansion budgets, identity, and inline provenance. That is a
separate typed function-language decision and is not necessary to reuse an
existing Relation. Deferred until at least two concrete operator uses can
falsify its generic contract.

### Add a separate RelationTemplate or law declaration

This describes the mathematical role directly but duplicates Component
visibility, package resolution, instantiation, identity, provenance, and
limits. A future `law` syntax may lower to a Component with Field slots if
repeated use demonstrates that the ordinary syntax is too heavy. A second
semantic hierarchy is rejected.

### Bind a mesh function or array

This would make package reuse convenient for one discretization while
collapsing Semantic Model and Realization. Rejected. Discrete Field artifacts
and external arrays have separate typed ownership and lineage.

### Infer the target by name or Relation context

This makes refactoring semantic, permits ambiguous matches, and delays failure
until a surrounding graph happens to be complete. Rejected. Every occurrence
binding is explicit and exact.

## Failure modes

The compiler rejects before Transaction exposure:

- a private Field slot, default, initializer, or non-continuum family;
- a slot on an unknown, boundary, or otherwise non-volume support;
- a missing, duplicate, or unknown Field binding;
- a target that is not a visible Field or forwarded Field slot;
- dimension, shape, frame, representation, support-kind, ambient-dimension,
  or exact-support mismatch;
- a `spatial_vector` slot without a positive exact volume dimension;
- qualified child selection or expression-valued Field target;
- a combined Parameter/support/Field binding count beyond the existing limit;
- expansion or canonical identity overflow; and
- any unsupported newer Field-slot syntax.

The compiler may not repair these failures through broadcasting, unit
conversion, geometric coincidence, mesh inspection, name similarity, default
binding, or a generated equality Relation.

## Falsifying verification

The registered `packages.occurrence-bound-fields` case must prove:

- scalar invariant and spatial-vector continuum slots bind to exact existing
  Fields and add zero flattened Fields;
- Relation DAG symbols refer to the exact targets;
- one nested wrapper forwards the same target identity through two levels;
- local, exact-package, package-alias, declaration-order, binding-order, and
  file-order variants preserve canonical flattened meaning;
- binding a different valid Field changes source identity and Relation
  references;
- every listed failure mode is rejected before graph mutation; and
- existing source-identity goldens remain unchanged when the feature is
  absent.

That central conformance case is implemented and registered. It proves the
hierarchy seam independently of any physics-specific lowerer or numerical
allocation.

The second registered
[`solid.packaged-isotropic-balance-2d`](../verify/solid/packaged-isotropic-balance-2d/README.md)
case completes the physics application. It moves the existing two-dimensional
isotropic balance Relation into the exact public
`Eqiora.Solid.LinearElasticity.IsotropicBalanceWithPotential2d` Component and
binds root-owned displacement and load-potential Fields. The root continues to
own its load definition and four boundary closures.

The case proves that:

- the packaged hierarchy and explicit Model have equal flat Relation/support
  meaning under complete deterministic identity normalization;
- package alias, provider name, declaration order, binding order, and file
  order are not lowering or execution keys;
- the existing name-independent method-neutral lowerer and Q1/CSR/CG path run
  unchanged and produce identical packaged and explicit solutions;
- the `4, 8, 16, 32` mesh sequence passes monotone L2 and H1 convergence and
  the registered observed-order gates;
- an independent affine potential preserves integrated body force `[1, 0]`
  and componentwise boundary-reaction balance across the package boundary; and
- package compilation, Model v4, Realization v1, Run v2, and package
  execution-binding identities replay as one exact, independently validated
  lineage.

## Compatibility and migration

The feature is additive source syntax and compiler-owned elaboration state.
It adds no Kernel kind or persistent hierarchy. Model v1-v4, Transaction
v1-v4, package v1, artifact, Realization, and Run wire formats do not change.
Old sources and source identities remain fixed when the optional records are
absent.

Downstream code consumes only the same ordinary flat Fields and Relations it
already understands. Removing a Field slot from source therefore requires no
artifact migration; it changes only how the compiler produced that flat
meaning and its source provenance.

## Nonclaims

This RFC does not implement or claim:

- public mutable Field members or `instance.field` selection;
- Field defaults, optional bindings, arrays, slices, or subfields;
- discrete function spaces, mesh functions, buffers, or transfer maps;
- PureOperator or general typed function declarations;
- local expression aliases such as `let stress = ...`;
- public field-valued physical Ports;
- boundary collections, partitions, or indexed Port families;
- traction, Robin, mixed, or nonzero essential-boundary execution;
- three-dimensional, nonlinear, or dynamic elasticity;
- fluid, structure-fluid coupling, ALE, or contact; or
- a field-result artifact or result-query API.

Those features may build on this contract only by preserving the same
meaning -> lowered contract -> Realization -> adapter -> evidence path.
