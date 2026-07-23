# RFC 0030: Symbolic package-definition validation

- Status: Implemented
- Authors: Eqiora contributors
- Created: 2026-07-19
- Depends on: [RFC 0021](0021-component-hierarchy-and-instantiation.md),
  [RFC 0022](0022-exact-package-identity-and-resolution.md)

## Summary

An exact Model Package release is returned only after the compiler has
statically validated every Connector, Component, and Model definition in the
complete source closure. Required public component Parameters are typed free
variables during this phase; the compiler does not invent values, instances,
identities, or provenance.

Resolved hierarchy compilation becomes a typed transition:

```text
resolved source
    -> AnalyzedResolvedHierarchy
    -> ValidatedResolvedHierarchy
    -> selected Model elaboration
    -> flat canonical Relation network
```

Only the validated state may elaborate a root Model. Package wires and digest
domains do not change.

## Later compiler boundary

The [RFC 0033 hierarchical connection-set
contract](0033-hierarchical-conserving-connection-sets.md) reuses this RFC's
occurrence-free definition gate. It
replaces the binary physical-membership summary with definition-local public
boundary partitions while leaving Relation ownership as an independent linear
obligation. The registered RFC 0030 case now admits idempotent reconnection of
an already connected child boundary class, but it still creates no occurrence
and proves neither occurrence-level exposure elimination nor execution. Local
hierarchical and direct-flat normalization are now registered by
[`language.hierarchical-connection-sets`](../verify/language/hierarchical-connection-sets/README.md).
Selected exact-package forwarding, provenance, canonical equivalence, and
affine execution are now registered separately by
[`packages.hierarchical-physical-boundary`](../verify/packages/hierarchical-physical-boundary/README.md).
RFC 0033's accepted bounded topology scope intentionally makes no
result-query claim for an eliminated exposure name; typed
physical-boundary projections belong to a separate field-interface contract.

## Motivation

Compiler-derived release preparation currently parses and globally indexes the
complete exact source closure, resolves direct aliases, and derives canonical
declaration identity. It elaborates only a caller-selected Model later. An
unused or second Model, or an uninstantiated Component body, can therefore
carry an invalid binding, relation, connection, or recursive instance graph
while a `PackageReleaseV1` is still returned.

That is too weak for fail-closed distribution. A semantic digest should not
name source that the same compiler already knows is statically ill-typed.
Conversely, a reusable Component with required public Parameters is an open
typed abstraction. Validating it by constructing a hidden instance would
invent values, occurrence depth, identities, and provenance that do not exist
in the package.

## Decision

### Validation is a compiler typestate

`analyze_resolved_hierarchy` remains the parser, global definition index, and
canonical-declaration comparison barrier. It returns
`AnalyzedResolvedHierarchy`.

The consuming operation

```rust
AnalyzedResolvedHierarchy::validate_definitions(self)
    -> Result<ValidatedResolvedHierarchy, Vec<Diagnostic>>
```

checks the complete definition graph. `compile_root` belongs only to
`ValidatedResolvedHierarchy`. Both states expose the root namespace and
canonical declarations needed by package identity, but an unchecked graph
cannot reach elaboration through the public compiler API.

Package preparation follows this order:

1. analyze the authoring namespace and exact dependency closure;
2. derive compiler-owned canonical declarations;
3. validate all definition bodies;
4. construct the candidate release;
5. resolve the candidate under its final exact namespace;
6. analyze and compare every release semantic claim; and
7. validate the exact graph before returning the release.

Locked compilation analyzes and compares exact semantic claims before the same
validation transition, then elaborates only the requested root Model.

### Definitions are typed abstractions

The checker has two logical passes.

The interface pass resolves every Connector contract and every Component
Parameter and Port contract. Required public Parameters become typed free
variables. A required private Parameter is invalid because no legal occurrence
can bind it. Defaults are symbolic expressions over the owning Component's
Parameters.

The body pass checks every public, private, used, and unused definition in
stable `(namespace, kind, name)` order. It validates:

- SI dimensions and supported scalar expression forms;
- Parameter default dependencies, cycles, and dimensional compatibility;
- nested instance target visibility and direct-alias scope;
- binding names, visibility, required completeness, and dimensions;
- Field, Clock, Relation, Port, and Connection contracts;
- nominal Connector identity rather than dimensions alone;
- component reference cycles and maximum future occurrence depth; and
- every Model independently, including Models not selected for execution.

Nested bindings substitute symbolic parent terms into the child interface.
No graph Transaction, kernel identity, instance path, or provenance entry is
created by definition validation.

### Static and concrete obligations remain distinct

The symbolic term contract records dimension and whether a value is known.
Arithmetic whose operands are known retains the existing finite-value checks.
An expression depending on a required public Parameter may remain symbolic
when only its static dimension is needed.

Obligations that truly require a concrete occurrence value stay at ordinary
binding/elaboration time. Examples include a future nonzero constraint, a
value-range constraint, or behavior that depends on a concrete parameter.
Such obligations must be explicit; validation may neither guess a witness nor
silently mark them proven.

The expression, nominal Port, and Connection compatibility rules are shared
compiler-internal contracts consumed by both definition checking and flat
lowering. A second handwritten type system that can drift from execution is
not acceptable.

### Physical endpoints carry compositional closure obligations

Each scalar physical endpoint has two independent linear slots: exactly one
constitutive Relation owner and exactly one conserving Connection-set
membership. The cardinality algebra is a pure schema contract shared by
occurrence-free definition admission and whole-model semantic validation.

A Component's own public physical Port may export either slot open or filled.
All four states are meaningful: a primitive can delegate both obligations, a
constitutive component can own the Relation slot while leaving topology to its
parent, and an internally closed public Port can expose an observation point.
Private Ports must close both slots inside their Component. Every public Port
of a child instance must close in its immediate parent, and every Model
endpoint must close completely. This prevents an implicit grandchild
re-export that the language cannot name or type.

Definition-DAG validation supplies one verified children-first order. Typed
body checking records the resolved physical endpoints used by each Relation;
the closure proof consumes those records and typed Connection endpoints rather
than reinterpreting expressions. References to the same endpoint within one
Relation fill one owner slot, while references from distinct Relations are an
error. Each instance receives independent slots. No occurrence, flat graph,
or fabricated identity is required.

The membership obligation deliberately means participation in a normalized
maximal connection set. RFC 0030 originally discharged it with one
non-overlapping Connection declaration per set. The later draft compiler slice
retains typed fragments and a public boundary partition instead; this changes
the topology proof without changing the owner obligation or the final flat
Kernel invariant.

### Graph limits are checked without expansion

Definition validation checks each body once. A memoized component-reference
graph pass detects strongly connected cycles and computes longest possible
instance depth and bounded occurrence footprint without expanding every
definition from every root.

Existing `HierarchyLimits` remain authoritative for source size, depth,
instances, declarations, Connections, identities, and provenance at selected
occurrence elaboration. The definition phase additionally bounds definition
count, instance edges, symbolic parameter terms, and accumulated diagnostics.
All counts use checked arithmetic and reject resource excess before large
allocation.

A standalone Component is measured as if it were placed immediately below a
future Model root: Model depth is one and its first Component depth is two.
This prevents a published abstraction from being structurally impossible to
instantiate under the current contract.

### Diagnostics have real source identity only

Definition diagnostics retain the package-qualified source label, declaration
or binding range, stable code, and a message naming the definition path. They
do not carry a fabricated `GraphPath`, because no occurrence exists.

Source semantic failures use the existing language type diagnostic family;
unsupported syntax and resource failure use the existing lowering family.
Diagnostics are returned in stable source order independent of package input,
file input, or declaration order.

## Verification

The implementation must prove:

1. a valid Component with a required public Parameter and no default is
   accepted without inventing a value;
2. defaults depending on required public Parameters remain symbolic and typed;
3. an unused public or private Component with an invalid Relation fails;
4. a nested binding with the wrong dimension or a missing required binding
   fails even when no Model instantiates the parent;
5. a required private Parameter fails;
6. an invalid second Model fails even when another Model is valid;
7. an unused recursive component cycle and an over-depth acyclic graph fail;
8. private, transitive, and unknown imported definitions fail at their exact
   source spans;
9. dependency, file, and declaration order do not change validation outcome;
10. no release, Model, Transaction, identity, or fabricated GraphPath is
    returned on failure; and
11. valid pre-existing package, source, resolution, Model, and compilation
    digests remain byte-for-byte unchanged;
12. public Component physical Ports may export each owner/membership slot
    independently, while an open private Port fails;
13. repeated access within one Relation has one owner, but two distinct
    Relations owning the same endpoint fail;
14. a parent cannot refill a closed Relation-owner slot or leave a child
    obligation untreated; scalar-physical membership fragments may
    idempotently extend an already connected child boundary class, while
    signal and structural-marker duplicate-use rules remain unchanged; and
15. two instances of one Component retain independent closure obligations,
    while an accepted nested definition agrees with defensive flat semantic
    validation after elaboration.

The composed-package dimension mismatch registered by
`packages.composed-model-package` moves from selected-Model compilation to
package preparation. Its existing successful identity oracle remains
unchanged.

## Alternatives considered

### Compile every existing Model

Rejected. It catches invalid Models and their reachable Components but permits
invalid reusable Components that no Model happens to instantiate. Package
validity would depend on examples rather than declarations.

### Generate one wrapper Model per Component

Rejected. Required Parameters have no canonical witness values. A wrapper also
invents instance depth, identity, path, and provenance and can introduce
value-dependent failures unrelated to the abstraction's static type.

### Validate only public declarations

Rejected. Private bodies are part of canonical package source and may be used
by public definitions. An unused private error must not become a delayed trap
after a later source edit makes it reachable.

### Store checked HIR or obligations in `PackageReleaseV1`

Rejected for this revision. Exact source is already content-addressed and is
rechecked before compilation. A future compiled-interface artifact may cache a
versioned checked contract, but it must not be added implicitly to the v1
release wire or digest domains.

## Compatibility

No package manifest, release, resolution, Model, compilation, Run, or binding
wire changes. Valid package identities remain unchanged because validation is
an admission predicate, not digest input.

Previously constructible releases whose unused definitions are statically
invalid will now be rejected. This is an intentional pre-release tightening of
the compiler contract, not a compatibility fallback.

## Nonclaims

Definition validation does not prove solvability, equation squareness,
stability, convergence, physical fidelity, behavioral equivalence for every
parameter value, execution success, package compatibility ranges, registry
publication, signatures or trust, field-valued physical boundary interfaces,
selected-occurrence cross-package connection-set union, imported Model
references, build scripts, native code, or dynamic plugins. The separate
selected-compilation evidence does not widen this occurrence-free gate.
