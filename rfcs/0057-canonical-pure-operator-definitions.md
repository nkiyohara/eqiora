# RFC 0057: Canonical pure-operator definitions and applications

- Status: Implemented and verified for the bounded pointwise slice;
  [`language.canonical-pure-operator`](../verify/language/canonical-pure-operator/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0038](0038-canonical-tensor-structure-operators.md),
  [RFC 0054](0054-curated-facade-and-control-plane.md),
  [RFC 0055](0055-component-parameter-terms.md), and
  [RFC 0056](0056-pure-calculus-and-support-maps.md)

## Summary

Eqiora makes bounded, typed, content-addressed pure-operator definitions and
their applications canonical Semantic Model meaning. A definition is a
capture-free pointwise tensor calculus over one exact Volume support. An
application names only the definition digest and ordered expression operands.
Source names, package aliases, callbacks, and execution providers are absent
from dispatch and Model identity.

Model and Transaction v5 carry one closed definition table per expression DAG
and one generic application expression kind. Existing v1--v4 bytes, digests,
and meanings remain unchanged. The existing v4 `symmetric_part` and
`isotropic_lift` expression kinds also remain unchanged; v5 is an extension
seam, not a migration that rewrites old meaning.

The falsifying consumer is a dyadic product of two spatial vectors. It reaches
ordinary source and exact-package compilation, v5 replay, typed component
lowering, and numerical evaluation without adding a dyadic-product Kernel
variant, a dedicated component formula, or a package-name recognizer.

## Motivation

RFC 0056 proved that two dedicated tensor operators can share one exact
component calculus after typing. It deliberately stopped before source-level
definitions or a generic canonical application. Without this RFC, the next
physics-neutral tensor operation would again require a dedicated expression
variant, Model generation, scalarization arm, and downstream recognizers.

An unrestricted callback is not an acceptable shortcut. Its meaning depends
on ambient executable code, cannot be validated before execution, and cannot
be reproduced from an artifact. A string operation name is equally weak: a
package rename, import alias, or registry order could silently change
semantics.

The seam must also retain Eqiora's two-layer boundary. A pure operator defines
pointwise mathematical meaning. It does not select quadrature, basis
functions, stabilization, transfer, a solver, a device, or a schedule.

## Decision

### Ownership and dependency direction

The contract follows one direction:

```text
Eqiora source / exact ModelPackage
              |
              v
schema-owned PureOperatorDefinition
typed value classes + ordered exact component body + digest
              |
              v
Semantic ExprDag
closed definition table + digest-keyed applications
              |
              v
TypedResidual
exact dimension + shape + frame + support
              |
              v
IR scalar expansion and optional normalization proof
              |
              v
Realization-specific numerical consumer
```

`eqiora-schema` owns definition meaning, construction limits, canonical bytes,
digest, and typed instantiation. `eqiora-ir` owns component expansion,
classification-only normalization proofs, and scalar lowering. Artifact,
compiler, package, and client layers consume those contracts; none defines a
second calculus.

### Common-volume pointwise tensor calculus

Every formal and the result declares exactly one value class:

```text
InvariantScalar
SpatialTensor(rank)
```

Rank is bounded and `SpatialTensor(0)` is not another spelling of scalar. At
instantiation, every formal must be on the same exact nonempty Volume support.
An invariant scalar has scalar shape and invariant frame. A spatial tensor of
rank `r` has shape `[d; r]` and `SpatialCartesian` frame on the common
`d`-dimensional Volume. The result receives that exact support and derives its
shape and frame from its declared value class.

The ordered body vocabulary remains deliberately small:

```text
exact reduced rational
formal component selected by result axes
Kronecker delta over result axes
negation
addition
multiplication
```

There is no call, lookup, recursion, branch, loop, callback, opaque opcode, or
provider selection. Every operand refers to an earlier node. Formal and node
counts, depth, tensor rank, and all checked aggregate wire counts have explicit
limits.

Physical dimension is derived rather than declared by an operation-specific
result rule. During definition validation, a formal component contributes one
symbolic formal-dimension variable, a rational or Kronecker delta contributes
one, multiplication adds bounded formal exponents, and addition requires the
same exponent monomial. Instantiation evaluates the root monomial against the
exact SI dimensions of the arguments with checked exponent arithmetic. This
admits products of dimensioned operands without weakening additive unit
soundness.

The three initial meanings are therefore ordinary definitions:

```text
symmetric(A)[i,j] = (A[i,j] + A[j,i]) * 1/2
isotropic(s)[i,j] = delta(i,j) * s
dyadic(a,b)[i,j]  = a[i] * b[j]
```

Only the first two retain their existing dedicated v4 expression kinds. The
dyadic definition is the first generic application consumer.

### Definition identity and compatibility

`OperatorDefinitionDigest` is SHA-256 over domain-separated canonical bytes
containing:

- the ordered formal value classes;
- the result value class;
- the ordered calculus nodes; and
- the root.

Names, declaration paths, package identities, aliases, spans, provenance,
runtime code, and providers are excluded. The in-memory type system has one
representation for each value class. The v1 definition encoder retains the
already exposed symmetric and isotropic byte identities by choosing their
legacy tags from their exact semantic patterns; extension tags encode only
new patterns. There are not two in-memory spellings of the same type rule.

A source declaration contributes its exact definition digest to the owning
package semantic declaration. A public source name remains part of that
package's public declaration path, but an application is resolved to the
digest before canonical Model construction. Changing an import alias or
moving declarations between source files cannot change compiled Model bytes.
Changing the body changes the definition and package semantic digests.

### Closed expression-owned definition table

`ExprDag` owns a digest-ordered closed map from
`OperatorDefinitionDigest` to `PureOperatorDefinition`. The only new Semantic
expression kind is a generic application containing:

```text
definition digest
ordered argument ExprIds
```

`ExprDagBuilder::pure_operator` receives the definition value and prior
arguments together. It validates arity, registers or deduplicates the exact
definition, and creates the application. A free-standing digest node cannot
be constructed through the public builder.

At `finish`, the set of referenced definition digests must equal the table
set. Missing, duplicate, digest-colliding, and unused definitions fail closed.
The table is expression-local so Model transactions need no ambient registry
and a detached Relation or Activation guard carries every definition required
to replay it.

Typing resolves the digest from that closed table, instantiates the definition
against the already inferred ordered argument types, and records the derived
result type. Component scalarization expands the same definition body while
retaining argument order, Parameter identities, and dense typed local input
slots. Downstream numerical code receives the generic typed proof or scalar
rows; it does not inspect package names.

### Source surface and qualified lookup

The first source form is explicit about the bounded component calculus:

```text
public pure operator dyadic(
  left: spatial[1],
  right: spatial[1]
) -> spatial[2] = component(left, 0) * component(right, 1);
```

`scalar` and `spatial[rank]` are the only value-class syntax. Definition bodies
admit `rational(n, d)`, `component(formal, result_axis...)`,
`delta(left_axis, right_axis)`, negation, addition, multiplication, and
parentheses. Numerators and denominators remain exact integers; no floating
literal is converted back into an exact rational.

Ordinary expressions accept ordered nonempty argument lists and qualified
callees such as `operators.dyadic(u, v)`. Built-in language operators retain
their existing validation and do not become registry entries. Pure-operator
lookup follows the same exact local/dependency namespace and visibility rules
as other package definitions. Resolution produces a definition digest before
the expression DAG is built.

There is no overload set in this slice. A source path resolves to exactly one
definition, and its declared formal value classes must accept the argument
types.

### Explicit Model and Transaction v5

V5 extends each wire expression with an optional definition table and one
application node. Empty tables are omitted, so the shared DTO does not change
v1--v4 writer bytes. Older encoders and decoders explicitly reject a nonempty
table or generic application, including inside Activation guards.

Each v5 wire definition contains its claimed lowercase digest, a sorted closed
required-feature set, formal and result value classes, ordered body nodes, and
root. The only initially accepted required feature is the versioned component
calculus itself. Normalization proof rules are not execution features.

Decode order is normative:

1. validate schema, encoding, bytes, and nesting;
2. count checked aggregate definitions, formals, calculus nodes, and
   application arguments across Relations and Activation guards;
3. reject missing or unknown required features;
4. rebuild every definition through the schema builder;
5. compare each claimed digest;
6. sort by digest and reject duplicates;
7. resolve applications, prior operands, and exact arity;
8. require referenced and supplied definition sets to match; and
9. reconstruct and validate the Kernel transaction or program.

No numerical allocation, provider lookup, or graph mutation precedes these
checks. Exact codec selection remains explicit under the compatibility API.
Ordinary authoring advances its registered current profile to v5; v1--v4
remain immutable exact codecs.

### Proof and execution order

Normalization remains a classification-only proof view. The ordered calculus
body is the executable expansion, and argument order is semantic. A proof of
an exact polynomial identity does not authorize floating-point reassociation,
commutation, folding, or replacement by a different body. Any numerical
rewrite requires an explicit Realization policy and separate evidence.

## Alternatives considered

### Add `OuterProduct` to `ExprNode`

This would complete the immediate CFD need but repeat the extension pattern
that motivated the issue. Rejected. The dyadic product must falsify the generic
seam.

### Put definitions in a process registry

A registry makes Model meaning depend on installation order and available
plugins. Rejected. Every expression is closed under the definitions it uses.

### Store definitions once at Model root

A root table avoids local duplication but makes detached transactions and
Activation guards depend on external context. Rejected for v5. Artifact-level
compression may deduplicate bytes later without changing semantic ownership.

### Use arbitrary tensor extents and index algebra now

General contraction, broadcast, permutation, reduction, and weak forms would
greatly enlarge typing and code generation. Rejected for this slice. Spatial
tensor rank over one common Volume is enough to prove a real extensibility
seam without claiming a universal tensor language.

### Change the existing v4 operators into generic applications

That would create a migration with no semantic benefit and invalidate exact
artifacts. Rejected. Old meanings remain old meanings; new expressions use the
new seam.

## Compatibility and migration

Model and Transaction v1--v4 bytes, digests, schema selection, and replay are
unchanged. V4 `SymmetricPart` and `IsotropicLift` nodes remain authoritative.
The definition implementation moves from IR to schema because it is now
canonical meaning; exact normalization and scalar expansion remain in IR.

The current authoring profile advances from v4 to v5 in Rust, Python, Studio,
and the shared profile fixture. Historical selection stays isolated in the
compatibility APIs. A v5 document retains v5 for value-edit transactions and
child revisions.

No public 1.0 compatibility lifetime has begun. Nevertheless, the two exposed
RFC 0056 definition digests remain fixed, and artifact compatibility is tested
independently from current authoring.

## Verification

The registered `language.canonical-pure-operator` case and its lower-level
schema, IR, compiler, and artifact regressions prove:

1. independently built symmetric and isotropic definitions retain their
   existing exact digests;
2. a dyadic definition derives exact rank, frame, support, and multiplied SI
   dimension, and rejects arity, rank, frame, or support mismatches;
3. local and exact-package source declarations reach the same generic
   application identity and component expansion, while equivalent variants
   within each authoring path retain exact v5 bytes;
4. package alias, source-file location, formal spelling, and declaration order
   cannot change package identity or Model bytes, while a body change changes
   definition and package identity;
5. no dyadic-specific Kernel node, component formula, or package-name
   recognizer exists;
6. unknown feature, forged digest, missing/duplicate/unused definition,
   forward operand, and every aggregate resource limit fail before graph
   mutation or numerical allocation;
7. argument and executable calculus order survive replay; and
8. v1--v4 fixed bytes and digests remain exact regressions and reject v5
   expressions even when their schema label is forged.

The capability matrix records only this bounded pointwise seam. It does not
generalize the evidence to weak forms, arbitrary tensor algebra, or production
CFD.

## Security and failure boundaries

Definitions contain owned data and closed enums only. Topological references,
exact formals, bounded axes, checked counters, rational reduction, monomial
overflow checks, closed feature negotiation, and digest reconstruction prevent
ambient capture, recursion, executable injection, and oversized semantic
payloads.

Package resolution remains exact and offline. No native library, Python
callback, network lookup, dynamic registry, or provider discovery is added.
Diagnostics reject an unsupported definition rather than retaining an opaque
node for later execution.

## Nonclaims

This RFC does not claim:

- universal tensor algebra, general contraction, broadcast, reduction, or
  symbolic equivalence;
- general weak-form, integral, test/trial, or constitutive compilation;
- support change, trace transfer, interpolation, mortar, remeshing, or mesh
  functions;
- recursive or algorithmic functions, conditionals, loops, state, side
  effects, callbacks, or dynamic plugins;
- floating-point reassociation or an optimizer derived from normalization;
- numerical method, solver, backend, device, schedule, or provider selection;
- a Python-native pure-operator definition builder in this slice; or
- general transient CFD, ALE, FSI adjoints, or shape sensitivity.

Those capabilities require their own typed contract, falsifier, registered
evidence, and Realization boundary. Unknown rules never widen this slice.
