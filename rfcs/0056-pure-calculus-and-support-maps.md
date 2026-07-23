# RFC 0056: Proof-carrying pure calculus and semantic support maps

- Status: Implemented and verified for the bounded L2 slice;
  [`language.pure-calculus-support-map`](../verify/language/pure-calculus-support-map/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0004](0004-scalar-operator-ir.md),
  [RFC 0007](0007-canonical-spatial-operators.md),
  [RFC 0034](0034-occurrence-bound-spatial-supports.md),
  [RFC 0038](0038-canonical-tensor-structure-operators.md),
  [RFC 0053](0053-discrete-block-system.md), and
  [RFC 0055](0055-component-parameter-terms.md)

## Summary

Eqiora introduces a bounded, capture-free component calculus in `eqiora-ir`
between an admitted `TypedResidual` and numerical classification. A pure
operator definition has closed formal and result-type rules, a topologically
ordered exact body, a domain-separated content digest, deterministic component
expansion, and an optional replayable exact-normalization proof.

The first slice re-expresses the existing Model v4 `symmetric_part` and
`isotropic_lift` meanings through that one calculus. Their existing expression
nodes remain canonical Semantic Model meaning. Component scalarization and the
elasticity and Stokes consumers replay a typed operator-application proof
instead of each owning the operators' component formula or trusting a package
name.

The same L2 boundary adds one semantic support-map oracle: an exact parent
Volume to owning Boundary trace restriction with parent-outward orientation
and pointwise-value pairing. It contains no mesh entity, basis, interpolation
weight, mortar rule, quotient matrix, or transfer implementation.

This bounded RFC adds no Model or Transaction v5, no Kernel node or expression
kind, no source-level user-defined `PureOperator`, and no general weak-form or
transfer language. RFC 0057 subsequently adds the first three extensions
without changing this case's V4 standard-operator evidence or support-map
claim.

## Motivation

RFC 0038 correctly introduced `symmetric_part` and `isotropic_lift` as
physics-neutral structure rather than an elasticity-specific node. The first
implementation nevertheless required one closed expression variant per
operation, wire changes, a dedicated scalarization arm, and repeated exact-DAG
recognizers in later physics paths. Repeating that pattern for tensor
contraction, constitutive composition, restriction, pairing, and frame maps
would make mathematical growth proportional to the number of codecs and
consumers.

The opposite shortcut is also unsound. An opaque callback or string-named
operator could make addition cheap, but its meaning would depend on executable
code outside the Model. Such a value cannot be typed, content-addressed,
replayed, or rejected before execution. Selecting a numerical method from a
source or package name would similarly collapse the Semantic Model and
Realization layers.

Algebraic equivalence adds a second hazard. Exact mathematical identities such
as

```text
2 * ((A + transpose(A)) / 2) = A + transpose(A)
```

are useful for deterministic classification. Rewriting an already ordered
floating-point program according to the same commutative identity can change
rounding, overflow, exceptional values, and reproducibility. Eqiora therefore
needs an exact proof view that can establish admission without silently
authorizing a different executable evaluation order.

Finally, a semantic trace and a finite-element transfer are not the same
object. The former states that a value on one Domain is observed on its exact
Boundary. The latter chooses spaces, meshes, orientations, quadrature, and an
algebraic operator. Putting both into one `SupportMap` would introduce
Realization meaning before a method has been selected.

## Decision

### Authority and layer boundary

The owning path is:

```text
canonical Relation expression
          |
          v
TypedResidual
dimension + shape + frame + exact support authority
          |
          v
eqiora-ir bounded pure calculus
typed application + component expansion + optional exact proof
          |
          v
package-neutral numerical classification
          |
          v
ordinary Realization, private discrete blocks, assembly, and solve
```

`TypedResidual` remains the sole admitted whole-expression typing authority.
The calculus does not infer a second Domain graph or reconstruct support from
names. A typed application receives exact `ExpressionType` values already
derived from the residual, applies one closed formal/result rule, and checks
that the derived result agrees with the typed Kernel node.

The calculus belongs to `eqiora-ir`. It is not another Semantic Kernel and is
not re-exported by the curated `eqiora` facade. The canonical expression DAG
continues to state Model meaning; the calculus is a lowered proof and
component-expansion seam used after semantic admission.

Its typed definitions, expansion, proof, and support-map vocabulary are public
from the L2 `eqiora-ir` crate because independent numerical consumers and the
registered conformance target must use the same checked contract. This is one
cohesive L2 conformance surface, not a stable end-user SDK: the curated
`eqiora` facade deliberately does not export it, and no duplicate callback or
string-opcode extension surface is added.

The private block system from RFC 0053 remains downstream of this seam. The
accepted elasticity and Stokes projections still travel through their normal
local-operator, assembly, canonical-CSR, and private block paths. This RFC does
not add calculus nodes, proof objects, or support maps to the block vocabulary.

### Closed first-slice calculus

A `PureOperatorDefinition` contains:

- an ordered non-empty formal list;
- one closed result-type derivation;
- a topologically ordered node array; and
- one root that refers to that array.

The first formal type rules are deliberately only:

- an invariant scalar on one Volume support; and
- an exact spatial Cartesian `[d,d]` tensor on one `d`-dimensional Volume
  support.

The result is either the complete type of one formal or the spatial Cartesian
`[d,d]` isotropic lift of an invariant scalar formal. Physical dimension,
shape, frame, and exact support are checked together. A definition cannot
change support identity or manufacture a discrete space.

The first body vocabulary is:

```text
exact reduced rational
formal component selected by result axes
Kronecker delta over result axes
negation
addition
multiplication
```

There is no identifier lookup, lexical environment, call node, recursion,
branch, loop, callback, foreign function, opaque opcode, or package name.
Formal slots are numeric and component access is expressed only through exact
result axes. Capture is therefore unrepresentable. Every operand refers to an
earlier node, so recursive and cyclic definitions are unrepresentable.

Construction fails closed above 64 formals, 4,096 nodes, depth 256, or 16,384
normal-form terms. Exact rationals are reduced to a signed `i64` numerator and
positive `u64` denominator; zero denominators and arithmetic overflow are
errors. The first dimensional checker admits multiplication only when at least
one side is dimensionless. Unsupported contraction, division by a live term,
nonlinear intrinsic, reduction, or support change is rejected rather than
carried as an unknown node.

These are version-one implementation bounds, not a claim that the eventual
mathematical vocabulary should remain this small. New rules require a typed,
bounded representation and a falsifying consumer; they do not enter as an
escape hatch.

### Content identity and component expansion

A pure definition has a domain-separated SHA-256 digest over its formal
rules, result rule, exact ordered nodes, and root. Source names, package names,
paths, spans, and runtime provider names are absent. Two independently built
definitions with the same typed body have one identity; two differently
ordered bodies may have different definition identities even when an admitted
normalization later proves them mathematically equivalent.

Instantiation first derives the exact result `ExpressionType`. Component
expansion then substitutes one in-range row-major result coordinate into the
body. It resolves only formal coordinates and Kronecker deltas; negation,
addition, multiplication, and rational nodes retain their exact topological
order. `symmetric_part` is represented by

```text
(A[i,j] + A[j,i]) * (1/2)
```

and `isotropic_lift` by

```text
delta(i,j) * s
```

with exact rationals and Kronecker delta resolved during component expansion.
The ordered expanded calculus, rather than an algebraically normalized form,
feeds executable scalar lowering.

Formal components substitute the original typed operand. They do not replace
its symbols with current values. Consequently, existing component
scalarization retains exact `ScalarSymbolCoordinate` values for every real
`SymbolRef`, including Parameters. Executable scalar rows read those values
through dense typed `ScalarInputSlot` plumbing; a slot contains its ordinal
and exact source coordinate but is not a Semantic `Parameter`, is not
serializable as one, and cannot compare equal to one. Two distinct Parameters
remain distinct even when their values are equal, and a forwarded Parameter
from RFC 0055 remains that same differentiation coordinate. No placeholder
Semantic identity is invented during lowering.

### Exact normalization is a proof view, not an execution rewrite

One component may be classified into a finite commutative polynomial over
exact formal-component atoms with exact rational coefficients. The
normalizer combines exact coefficients, sorts formal atoms within monomials,
and orders the resulting monomials. It is total only within the admitted
polynomial vocabulary and resource limits.

`NormalizationProof` records:

- the versioned proof rule;
- the complete formal-argument and result type context;
- a digest of the exact result coordinate and ordered source component;
- the admitted exact normal form and its digest.

Verification compares the complete typed context, rehashes the supplied
source, rejects an unknown rule, reruns the bounded normalizer, and compares
both proof content and digest. A proof for a different source cannot be
replayed merely because a caller presents the same claimed normal-form digest.
Normal-form equivalence also requires equal formal and result types, so an
exact zero in one physical dimension or support cannot prove an exact zero in
another.

Two independently verified proofs may establish that their exact normal forms
agree. This fact authorizes semantic admission or classification only. It
does **not** authorize reordering, reassociation, common-subexpression
elimination, constant folding beyond the ordered component expansion, or any
other change to executable floating-point instructions. Such a numerical
transformation needs an explicit Realization policy and its own evidence.

The proof is likewise not general symbolic equality. It does not reason about
floating-point literals, transcendental functions, conditionals, derivatives,
integrals, support maps, or identities outside the exact polynomial rule.

### Existing V4 tensor operators

`StandardPureOperator::SymmetricPart` and
`StandardPureOperator::IsotropicLift` identify the two definitions justified
by the first slice. `OperatorApplicationProof` classifies an existing V4
Kernel node only after:

1. locating the exact node in a `TypedResidual`;
2. matching the expected existing expression variant;
3. replaying the selected content-addressed definition against the exact
   operand type; and
4. proving that the derived result type equals the type already inferred for
   the Kernel node.

The proof retains the existing operand, result type, operator family, and
definition digest. A different Kernel node returns no classification. A
matching node with an invalid operand or result fails explicitly.

Component scalarization uses the same definitions for the exact coordinate
formula. Canonical elasticity and Stokes recognition use application proofs
for the tensor-structure operations and issue diagnostics containing the
operation, expression-node index, and proof failure. They may still inspect
the proof's operand for the bounded constitutive patterns admitted by their
own RFCs. This is not yet general constitutive normalization.

### Semantic trace support map

The first `SupportMap` proves only one relation:

```text
source = Volume(domain, dimensions)
target = Boundary(boundary, parent = domain, dimensions)
intent = TraceRestriction
orientation = ParentOutward
pairing = PointwiseValue
```

The source must be a Volume. The target must be a Boundary whose parent is the
exact source Domain identity, and their ambient dimensions must agree.
`SupportMap::classify_trace` derives both supports from the typed operand and
result of one existing canonical `Trace` node. Missing support, a foreign
parent, a dimension mismatch, or a different operator fails closed.

`PointwiseValue` describes semantic pairing only; it is not an interpolation
promise. The support map cannot represent mesh numbering, discrete spaces,
quadrature, basis evaluation, mortar projection, a conforming quotient,
conservative transfer, MPI ownership, or an execution provider. Those remain
typed Realization artifacts selected after method and capabilities are known.

### Named pure operators and package identity were deferred from this slice

`DeclarationKindV1::PureOperator` reserved a declaration-family tag in the
package semantic-content schema, but this RFC did not implement that source
feature. [RFC 0057](0057-canonical-pure-operator-definitions.md) now owns the
bounded source declaration, qualified exact-package lookup, generic canonical
application, and Model/Transaction v5 extension. The registered RFC 0056 case
remains deliberately V4-only and is not retroactively evidence for them.

The two definitions in this RFC remain compiler-owned standard lowerings and
their dedicated V4 expression nodes remain authoritative. RFC 0057 preserves
their exact digests while specifying a separate generic path for new
definitions. Overloading, polymorphism, recursion, general tensor algebra, and
weak-form composition remain unimplemented; neither the reserved tag nor the
new bounded dyadic consumer is evidence for those broader surfaces.

## Prior art and deliberate departures

[UFL](https://docs.fenicsproject.org/ufl/main/manual/form_language.html)
demonstrates the value of shape-aware tensor indexing, restrictions, measures,
and user-defined mathematical composition close to source notation. Eqiora
adopts the separation between tensor structure and eventual finite-element
execution, but not Python execution as semantic identity and not a universal
variational-form surface in this slice.

[TSFC and its GEM intermediate representation](https://arxiv.org/abs/1705.03667)
show why preserving mathematical structure through staged lowering can avoid
premature expansion. Eqiora similarly keeps typed structure until component
lowering, but the present calculus is much smaller: it neither accepts general
forms nor generates finite-element kernels.

[MLIR Linalg](https://mlir.llvm.org/docs/Dialects/Linalg/) uses generic
structured properties shared by named operations so transformations need not
depend on one-off operation knowledge. Eqiora adopts that generic-definition
and named-application relationship. It deliberately rejects unrestricted
regions, side effects, external-library names, and executable payloads as
mathematical meaning.

[Modelica pure functions](https://specification.modelica.org/master/functions.html)
provide useful precedent for mathematical functions being pure by default.
Eqiora's first contract is stricter: there is no algorithmic function body,
external call, hidden state, or source-level function surface yet, only a
closed exact component DAG.

[PETSc DM](https://petsc.org/release/manual/dmbase/) exposes interpolation,
restriction, and injection as mappings between discretization managers rather
than as bare topological labels. [MFEM finite-element spaces](https://docs.mfem.org/html/classmfem_1_1FiniteElementSpace.html)
likewise construct transfer operators from concrete coarse and fine spaces.
These APIs support Eqiora's decision to keep numerical transfer in
Realization: a semantic support map says *what supports are related*, while a
space-aware transfer operator says *how discrete data moves*.

## Alternatives considered

### Continue adding one Kernel expression case per operation

This is simple and gives each operation explicit artifact identity. It remains
appropriate when an operation is genuinely irreducible canonical meaning.
Using it for every named tensor composition, however, multiplies schema,
codec, scalarizer, recognizer, and compatibility work. Rejected as the sole
extension mechanism after the two V4 operators established a concrete shared
calculus need.

### Replace the existing V4 nodes with generic calculus in a new wire

This would make the Model itself more uniform, but it would invalidate or
fork accepted V4 meaning for no first-slice user benefit. The lowered seam can
prove the architecture while retaining immutable artifacts. Rejected; a
future canonical calculus wire would require independent need and migration
evidence.

### Build a universal weak-form language now

A UFL-like surface could naturally express contractions, integrals,
test/trial pairing, and many PDEs. It also commits immediately to measures,
function spaces, differentiation, restrictions, nonlinear operators, source
ergonomics, and code generation. That scope would obscure whether the small
composition seam works. Rejected for this slice; later operators are added
only with a real vertical consumer.

### Use Rust, Python, or native callbacks as operator bodies

Callbacks are easy to author and can be fast, but introduce capture, ambient
state, platform code, lifetime and ABI concerns, and nonreplayable identity.
They also make fail-closed validation impossible without executing untrusted
code. Rejected as Semantic or lowered mathematical meaning.

### Canonicalize the executable floating-point DAG

One canonical DAG would simplify recognizers and might improve optimization.
Commutative exact algebra is not IEEE floating-point equivalence, however, and
would silently change evaluation order. Rejected. An exact proof view and the
ordered executable component have different authorities.

### Put interpolation and mortar data in `SupportMap`

This produces one convenient coupling object, but its identity would depend on
mesh, space, method, and backend choices absent from the Semantic Model. It
would also make a trace choose a numerical realization. Rejected; a later
Realization may bind one semantic map to one explicit transfer artifact.

## Compatibility and migration

This RFC changes no Kernel node kind, `ExprNode` vocabulary, Model envelope,
Transaction operation, package wire, Realization wire, Run wire, or decoder.
Model and Transaction V1--V4 remain closed, and no V5 is introduced. Existing
V4 `SymmetricPart` and `IsotropicLift` bytes, digests, replay behavior, and
semantic meaning remain authoritative.

The component scalarizer and bounded physics recognizers change their internal
route while preserving their accepted typed and numerical outputs. Existing
canonical-CSR fingerprints and registered elasticity, Stokes, private-block,
and fixed-reference FSI evidence remain regression oracles.

The calculus definitions, normalization proofs, application proofs, and
support maps are not durable artifacts in this slice. Their Rust API belongs
to the lowered `eqiora-ir` crate and is intentionally absent from the curated
public facade. No stored proof requires migration.

The reserved package `PureOperator` declaration kind gains no new source or
wire meaning here. Source packages cannot rely on it until a later accepted
RFC closes the full package-identity path.

## Verification

The registered
[`language.pure-calculus-support-map`](../verify/language/pure-calculus-support-map/README.md)
case runs the `eqiora-ir` integration target `pure_calculus_support_map` and
must prove:

1. the existing typed V4 `symmetric_part` application replays the exact
   standard definition digest and retains its exact operand;
2. both standard tensor operations instantiate through the shared type and
   component calculus; construction and checker unit tests separately retain
   the closed shape, frame, support, component, and definition gates;
3. `2 * ((A + A^T) / 2)` and `A + A^T` have distinct ordered definition
   identities but independently replayed exact proofs of one admitted
   component normal form;
4. a proof rejects a substituted source or typed context instead of accepting
   a caller-provided digest; the private checker unit suite additionally
   rejects unknown rule versions until a durable proof decoder exists;
5. component scalarization retains two distinct Parameter identities through
   dense typed local slots, rather than merging equal-valued coordinates or
   fabricating placeholder Semantic Parameters;
6. one typed `Trace` derives the exact Volume-to-owning-Boundary map with
   parent-outward orientation and pointwise-value pairing, while a foreign
   parent is rejected; and
7. the map and proof contracts contain no numerical transfer data and grant
   no floating-point reassociation authority.

The existing canonical-tensor, packaged elasticity, packaged Stokes,
physics-neutral discrete-block, and fixed-reference FSI cases remain separate
execution regressions. They prove the unchanged V4 and numerical paths; the
new case does not duplicate their physical-solution claims.

## Security, safety, and governance

Definitions are owned data with closed variants. Topological back-references,
explicit formals, fixed axes, and resource limits prevent arbitrary recursion,
ambient capture, unbounded traversal, and executable injection. Rational
construction checks denominator and arithmetic range. Canonical digest domains
separate definition identity, ordered component identity, and exact
normal-form identity.

Proof verification never trusts an advertised digest alone. It checks the
exact source digest, rejects unknown rule versions, reruns bounded
normalization, and compares the complete result. Unsupported calculus features
and support relations fail closed with typed errors.

No unsafe code, native loading, Python execution, network resolution, dynamic
registry, or provider discovery is introduced. Numerical transfer continues
to cross the ordinary Realization and evidence gates rather than acquiring
authority from this semantic oracle.

## Nonclaims and deferred work

This RFC does not implement or claim:

- source-level or Python-native user-defined `PureOperator` declarations;
- inclusion of definition digests in package semantic identity;
- a new Model/Transaction generation or a generic calculus artifact wire;
- arbitrary tensor indexing, permutation, contraction, broadcast, reduction,
  integration, test/trial pairing, pullback, pushforward, or frame transform;
- general algebraic, floating-point, differential, or symbolic equivalence;
- rewriting or optimizing an executable floating-point program from a proof;
- a universal weak-form compiler, symbolic algebra system, or constitutive
  language;
- nonmatching trace transfer, interpolation, restriction matrices, mortar,
  Nitsche, quotient, conservative transfer, contact, or homogenization;
- a public block IR, transfer-provider SDK, dynamic plugin ABI, callback, or
  string-opcode registry;
- a durable normalization-proof or support-map artifact; or
- a new elasticity, Stokes, FSI, nonlinear, transient, adjoint, or shape-
  sensitivity capability.

Future work should first use a second real mathematical consumer to justify
the next calculus or support-map rule. Source-level named operators require a
separate package/language decision. Numerical transfer requires a separate
Realization contract binding exact supports to exact discrete spaces and
evidence. Neither extension may widen this slice by treating an unknown node,
proof rule, or provider as implicitly compatible.
