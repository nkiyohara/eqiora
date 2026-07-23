# RFC 0034: Occurrence-bound spatial supports

- Status: Accepted and implemented for Cartesian volume and boundary supports
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0021](0021-component-hierarchy-and-instantiation.md)

## Summary

A reusable Component declares typed spatial-support slots and each occurrence
binds them to exact existing Domain identities before deterministic flattening;
the slots themselves never become Semantic Kernel entities.

## Implementation status

The accepted bounded slice implements:

- public `volume(ambient_dimension = n)` and
  `boundary(parent = volume_slot)` support slots;
- a dedicated instance support-binding syntax, separate from scalar Parameter
  expressions;
- Component-local continuum Representations, spatial Fields, and spatial
  Relations whose support is named through those slots;
- exact occurrence binding to existing Cartesian box and boundary Domains;
- nested forwarding through another Component occurrence;
- fail-closed validation of completeness, visibility, kind, ambient dimension,
  and exact `BoundaryOf` parentage before expansion;
- deterministic source identity and complete occurrence provenance for support
  bindings; and
- elimination of every support slot during flattening, leaving only ordinary
  `DefinedOn`, `AppliesOn`, and `BoundaryOf` edges over exact Domain identities.

The registered
[`packages.component-spatial-supports`](../verify/packages/component-spatial-supports/README.md)
case owns the bounded conformance claim. It is deliberately not evidence for a
field-valued Port, multidimensional discretization, or fluid-structure
interaction.

## Motivation

A reusable spatial Component cannot name a Model-local Domain in its
definition. A solid constitutive law, fluid boundary law, or trace Relation
must nevertheless state whether it is defined on a volume or on an exact
boundary of that volume. The concrete Domain changes with each occurrence:

```text
Component definition
  body      : volume support
  interface : boundary support whose parent is body

first occurrence                 second occurrence
  body      -> fluid               body      -> fluid
  interface -> left wall           interface -> right wall
```

Three shortcuts would violate the meaning/Realization boundary:

- inferring support from a later Connection makes endpoint meaning depend on
  graph context;
- binding a mesh, facet set, or transfer map makes a numerical realization
  part of the Semantic Model; and
- storing an unchecked name or callback postpones a type obligation until an
  adapter or executor happens to interpret it.

An ordinary Parameter binding is not the right abstraction either. A support
is an exact nominal identity and topology proof, not a scalar value or an
expression to evaluate. This RFC therefore adds one closed hierarchy
interface while preserving the single flat Semantic Kernel established by
RFC 0021.

## Proposed design

### Closed source contract

The first source form is explicit:

```text
component BoundaryState {
  public support body: volume(ambient_dimension = 2);
  public support interface: boundary(parent = body);

  representation state_space = continuum;
  field state on body as state_space: 1 = 0;

  relation volume_law continuous on body {
    state = 0;
  }
  relation interface_law continuous on interface {
    trace(state) = 0;
  }
}

model Main {
  domain fluid = box(0, 1, 0, 1);
  domain wall = boundary(fluid, axis = 0, side = lower);
  instance probe: BoundaryState(
    support body = fluid,
    support interface = wall
  );
}
```

A definition support slot has one of two v1 forms:

```text
Volume   { ambient_dimension }
Boundary { parent_slot }
```

The ambient dimension is positive and part of the Component contract. A
boundary slot names an exact volume slot in the same Component; it does not
merely repeat a dimension. Slots are public occurrence obligations in v1.
Private slots are explicitly rejected because no v1 declaration can construct
or bind one without making its concrete Domain an implicit implementation
choice.

An instance support binding has the closed form `support slot = target`.
`target` is a visible spatial support in the enclosing occurrence scope, not
an `Expr`. Parameter bindings and support bindings remain distinct AST and
compiler contracts even though both appear in one instance argument list. The
word `support` is contextual, so an existing Parameter literally named
`support` remains representable as `support = value`.

Component Representations remain ordinary declarations after expansion. A
Component Field may use a volume slot as its `on` support, and a Component
Relation may use a volume or boundary slot as its `on` scope. The existing
expression type checker then derives the exact support and ambient dimension:
for a 2D slot `coordinate(1)` is valid while `coordinate(2)` fails. No mesh or
realization participates in this proof.

### One identity-parametric support algebra

The compiler and Semantic Kernel validator share the existing closed type:

```text
SpatialSupport<I> =
  Volume   { domain: I, dimensions }
  Boundary { domain: I, parent: I, dimensions }
```

Only the identity parameter changes between stages:

```text
definition checking  SpatialSupport<SupportSlotName>
occurrence expansion SpatialSupport<FullElaborationIdentity>
kernel validation    SpatialSupport<DomainIdentity>
```

This is one typing rule at three identity resolutions, not three parallel
representations with handwritten conversion policy. Physical dimension,
expression shape, and spatial support remain independent axes of an
`ExpressionType`.

A support slot is neither a tenth Kernel node kind nor a universal Resource
payload. It is a typed obligation owned by hierarchy elaboration. This keeps
the canonical vocabulary unchanged and gives the compiler and semantic
validator the same exact parent/support relation.

### Validation before occurrence expansion

Every Component definition is checked without choosing a concrete Domain:

- slot names share the ordinary Component member namespace;
- a volume slot declares a positive ambient dimension;
- a boundary slot names an existing volume slot;
- every slot is public in v1;
- a Field names a valid volume slot and Representation;
- a Relation names a valid volume or boundary slot; and
- the ordinary shape, physical-dimension, support, trace, normal, and residual
  rules hold over slot identities.

Every occurrence is then checked against exact supports in its enclosing
scope. Each required slot must have exactly one binding. Unknown, duplicate,
missing, or non-spatial targets fail. Volume-to-boundary and
boundary-to-volume bindings fail symmetrically. Ambient dimensions must match.
For a boundary slot, the bound boundary's exact parent identity must equal the
Domain identity bound to the declared parent slot; equal dimension or equal
geometry is insufficient.

Nested Components use the same operation. A child instance may bind its slot
to a parent Component's already bound slot. Forwarding therefore composes
without inventing an intermediate Domain, weakening exact parentage, or
special-casing package boundaries.

All definition and occurrence checks finish before expansion can expose a
Transaction. A failure cannot partially mutate a graph.

### Flattening and canonical meaning

Occurrence expansion resolves a slot to the exact existing Domain identity and
places that alias only in the compiler's lexical occurrence scope. Fields,
Relations, and their expression support are rewritten through the alias.

The slot allocates none of the following:

- a Kernel entity;
- a graph identity;
- a public symbol or Model alias;
- a display-only identity;
- an edge; or
- an artifact payload.

After flattening, the Component organization is gone. A Field has an ordinary
`DefinedOn` edge to the bound volume Domain, a Relation has an ordinary
`AppliesOn` edge to the bound volume or boundary Domain, and the existing
boundary retains its one exact `BoundaryOf` edge. Two instances bound to
different boundaries have distinct occurrence identities for their expanded
Fields and Relations while sharing only the Domain identities explicitly bound
by the source.

This is the same semantic endpoint as an equivalent explicit flat model. A
runtime, solver, mesh adapter, and artifact reader need no support-slot API.

### Source identity and provenance

Support declarations and bindings enter canonical source identity as closed,
sorted records. Declaration order, binding order, formatting, source spans,
map insertion, and traversal cannot change identity. Rebinding a slot to a
different exact source target is semantic and changes source identity. The
current local-source namespace is derived from that whole source identity, so
this RFC makes no claim that resulting identity changes are occurrence-local.

The namespace and exact package rules of RFC 0021 and RFC 0022 continue to
apply. Dependency alias spelling is alpha-normalized to the resolved package
namespace for canonical Model meaning. It may remain in package compilation
lineage without changing the flattened Model.

Every expanded entity's provenance contains its definition span, instance
span, scalar Parameter binding spans, and support-binding spans. Provenance
explains how an exact Domain entered an occurrence but does not become part of
the Domain, Field, Relation, or Model identity.

### Resource limits

Support bindings share the existing per-instance binding budget with Parameter
bindings. This prevents a second nominally different limit from allowing the
same instance argument list to grow unbounded. Existing limits independently
bound Component members, occurrence count and depth, canonical identity bytes,
expanded declarations, and provenance origins/binding spans.

Counts use checked arithmetic and validation precedes expansion. The support
resolver uses bounded maps over already admitted declarations and exact
Domain identities; it performs no filesystem access, package discovery,
geometric search, or mesh inspection.

### Boundary with field-valued interfaces and Realization

Field-valued Ports and a `[2]` vector were motivating examples. Their payload
and shape semantics belong to
[RFC 0035](0035-field-valued-boundary-interfaces.md), which this RFC unblocks;
they are not silently accepted here. This slice proves the exact support lookup
that the Port contract can consume.

Canonical meaning in this RFC owns only:

- support kind;
- exact Domain and parent identities;
- ambient dimension; and
- deterministic occurrence binding.

Realization continues to own mesh entities, facet sets, quadrature, discrete
trace spaces, nonmatching interpolation, mortar or Nitsche choice, transfer
operators, coupling schedule, partitioning, and device residency. None can
appear in a support binding or alter the Semantic Model digest.

## Alternatives considered

### Infer support from Connections

This would shorten instance syntax, but a Component's Field and Relation
meaning would remain underdetermined until an external topology was complete.
It also creates ambiguous cases when several compatible boundaries are
connected. Rejected: support is an explicit typed occurrence obligation.

### Treat support as an ordinary Parameter expression

This reuses binding syntax but gives a Domain identity value-expression
semantics, invites arithmetic or callbacks, and conflates SI dimension with
spatial support. Rejected. The syntax is adjacent; the contracts are separate.

### Pass mesh or facet handles

This makes assembly convenient but binds a reusable physics definition to one
discretization and allows Realization choices to change model identity.
Rejected. Numerical handles belong after semantic lowering.

### Add Component and support nodes to the Kernel

This preserves authoring hierarchy for runtimes, but every consumer would
need a second elaboration semantics and explicit-flat equivalence would no
longer be canonical by construction. Rejected. Authoring hierarchy remains a
projection plus provenance.

### Use unchecked names or a universal Resource payload

This defers compatibility to adapters and cannot fail closed on kind,
dimension, or exact parent. Rejected. The closed `SpatialSupport<I>` contract
is smaller and shared.

### Allow private slots in v1

A private required slot cannot be supplied by the parent, while compiler
inference or an embedded Domain would hide a semantic decision. Rejected as
uninhabitable. A later proposal may add a private local Domain declaration,
but it must define that Domain's canonical identity and visibility directly.

## Failure modes

The following are hard compiler errors before Transaction construction:

- zero-dimensional volume slot;
- unknown or non-volume boundary parent slot;
- private support slot;
- missing, duplicate, or unknown occurrence binding;
- target that is not a visible spatial support;
- volume/boundary kind mismatch;
- ambient-dimension mismatch;
- boundary whose exact parent differs from the bound parent slot;
- boundary of a boundary in the Cartesian v1 source contract;
- Field or Relation use of an unknown or wrong-kind slot; and
- expression shape/support violations after occurrence-independent typing.

The compiler may not repair these errors through name similarity, geometric
coincidence, default dimensions, Connection inference, or mesh inspection.

## Compatibility and migration

The change is additive source syntax and compiler-owned elaboration metadata.
It adds no Semantic Kernel node or edge kind and does not change Model v1/v2,
Transaction v2, artifact, package-release, realization, or run wire formats.
No wire version increases.

The source-identity encoder writes the new instance support-binding field only
when at least one support binding exists. Components and instances without
support slots therefore retain their exact existing source-identity bytes.
Existing scalar Parameter syntax, including a Parameter named `support`, keeps
its prior parse and meaning.

Support-aware Components require the new compiler. Their flattened Model is an
ordinary existing graph and can be consumed by existing kernel, artifact, and
execution code that already understands its Domain/Field/Relation contract.
There is no persistent support-slot object to migrate.

## Verification

The machine-readable `packages.component-spatial-supports` case is the sole
registry claim for this RFC. It must falsify the contract through:

- one 2D Cartesian volume and two exact boundaries;
- two occurrences of one Component bound to different boundaries;
- exact `DefinedOn`, `AppliesOn`, and `BoundaryOf` identities;
- distinct expanded occurrence identities;
- absence of support-slot entities, symbols, and aliases;
- complete support-binding provenance;
- nested forwarding through one wrapper Component;
- a Component-local continuum Representation, volume Field, volume Relation,
  and boundary `trace` Relation;
- exact-package dependency alias invariance for the flattened Model; and
- rejection of missing, duplicate, unknown, private, kind-mismatched,
  dimension-mismatched, wrong-parent, and boundary-of-boundary bindings before
  graph mutation.

Source-identity tests separately require declaration/binding permutation
invariance, exact-target sensitivity, the combined Parameter/support binding
limit, and unchanged legacy bytes when no support bindings exist. Expression
tests require the declared ambient dimension to admit `coordinate(1)` and
reject `coordinate(2)` for a 2D volume slot.

Passing this case proves occurrence-bound Cartesian support. It does not prove
a field-valued Port, vector/tensor Field payload, mixed element, 2D/3D
assembly, FVM/FEM realization, mesh import, geometric matching, transfer,
fluid or solid physics, or coupled execution.

## Security, safety, and governance

The feature introduces no unsafe code, dynamic loading, callback, ambient
filesystem authority, package discovery, network authority, or execution
provider. Package sources still pass the exact content and resolution barriers
before elaboration. Resource exhaustion is governed by the shared hierarchy,
identity, and provenance budgets described above.

Acceptance records only the bounded claim named by the registered case. Any
new support kind, implicit binding, private local support construction,
geometric matching rule, persistent support wire, or Realization handle in the
source contract requires a new RFC and falsifying evidence.

## Unresolved questions

None for the accepted bounded slice. Field-valued Ports and their vector or
tensor shape contract proceed through
[RFC 0035](0035-field-valued-boundary-interfaces.md) against the exact support
lookup defined here.
