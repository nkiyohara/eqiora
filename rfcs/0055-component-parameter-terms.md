# RFC 0055: Identity-preserving Component Parameter terms

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-07-21
- Depends on: [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0011](0011-implicit-differentiation-contracts.md),
  [RFC 0021](0021-component-hierarchy-and-instantiation.md),
  [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0038](0038-canonical-tensor-structure-operators.md),
  [RFC 0044](0044-packaged-steady-incompressible-newtonian-2d.md), and
  [RFC 0046](0046-power-conjugate-mechanical-boundaries.md)

## Summary

A Component Parameter is a typed lexical term, not an occurrence-local
Semantic Kernel entity. Component elaboration substitutes that term into its
consuming Relations while preserving the exact identities of enclosing Model
Parameters.

A direct binding to one enclosing Parameter retains that Parameter as the one
mutable and differentiable coordinate. Arithmetic bindings retain a typed
expression DAG over exact Parameter leaves and dimensioned constants. A
literal binding specializes to a true dimensioned constant with no Parameter
identity, mutation target, or automatic-differentiation direction.

Within one compiled Model, spatial coefficient agreement is exact
lowered-relation equality, including the expression tape, canonical Parameter
identities, and their revision-local values. Equal current values of
independent Parameters never establish a shared constitutive coefficient.

## Motivation

RFC 0021 originally specified Component Parameter bindings as compile-time
typed expressions, but the first elaborator materialized every bound
Parameter as a new occurrence-local Kernel Parameter. Two Components bound to
the same enclosing Parameter consequently produced two independent canonical
coordinates. Two Components bound to equal literals produced the same shape
of independence even though the author had supplied no mutable quantity.

The distinction can be hidden in a primal calculation by comparing current
coefficient values. It cannot be hidden from linearization. If volume and
boundary laws both use `mu`, their perturbations must contain the same
`delta mu`. If they instead use distinct Parameters that happen to equal
`1.0` at one revision, those Parameters have distinct tangent and cotangent
directions. Treating them as one coefficient changes the derivative of the
original Relation network.

Physics-specific provenance guesses are not a repair. A Stokes or elasticity
recognizer must not infer origin from a source name, package name, instance
path, RawId layout, or coincident value. The compiler owns binding meaning;
the numerical layer receives an exact, shared expression contract.

## Decision

### Two different meanings named Parameter

A root Model `parameter` declaration remains an ordinary Semantic Kernel
Parameter. It has a canonical identity, a finite coherent-SI value at a Model
revision, and one selectable mutation and differentiation coordinate.

A Component `public parameter` declaration is instead a lexically scoped,
typed binder in the reusable definition. Its contract includes scalar value
kind and physical dimension, but the declaration does not require or justify
a new Kernel node at each occurrence. The term disappears during hierarchy
elaboration after substitution into the ordinary flat Relation network.

Defaults and explicit occurrence bindings follow the same rules. Nested
Component forwarding composes terms; it does not introduce a new identity at
each lexical boundary.

### Exact forwarding, constants, and derived terms

After definition-time checking, every concrete Component Parameter resolves
to one of three forms:

1. **Exact Parameter forwarding.** A direct binding to one enclosing Model
   Parameter is a reference to that exact canonical Parameter identity. Every
   consuming Relation uses the same leaf. The occurrence-qualified Component
   name may be retained as a display alias for diagnostics and authoring, but
   it resolves to the parent identity and creates no second entity.
2. **Constant specialization.** An exact literal becomes a finite constant in
   the target Parameter's coherent-SI dimension. It has no Kernel Parameter,
   occurrence alias, mutation target, or JVP/VJP coordinate. Changing the
   literal means compiling different immutable Model content.
3. **Derived term.** Negation, addition, subtraction, multiplication,
   division, and an integer power retain a typed compiler-owned expression
   DAG over exact enclosing Parameter leaves and dimensioned constants. The
   DAG is substituted into each consuming Relation. Repeated leaves keep one
   canonical Parameter identity, so ordinary primal, JVP, and VJP evaluation
   applies the chain rule without reconstructing binding provenance.

Every binding is finite, dimensionally valid, acyclic, and resource-bounded
before graph mutation, as required by RFC 0021 and RFC 0030. A power exponent
must be an exact compile-time `i32` integer. An exponent that depends on a live
Parameter fails closed: it would make expression structure and physical
dimension depend on a mutable coordinate, for which this contract supplies no
canonical or differentiation meaning.

The compiler's lowering expression is an internal typed seam, not a second
source AST and not a new public symbolic-algebra API. It exists so hierarchy
substitution can carry exact quantities and Parameter references into the one
ordinary Kernel expression builder.

### Display aliases and provenance are not meaning

For exact forwarding, the root name and any occurrence-qualified child names
may all resolve to the same Parameter identity. Those aliases help Studio and
diagnostics explain the lexical route by which the binding was selected. They
do not enter Model identity and cannot be used by a physics recognizer.

A literal or derived term has no fabricated Parameter alias because no single
Parameter entity is represented by that Component slot. Binding spans,
definition spans, instance spans, and the complete forwarding chain remain in
compiler provenance. Moving a file or changing a source span can change that
provenance without changing the flattened Model.

### Exact coefficient agreement

All admitted spatial boundary recognizers use one shared coefficient-
agreement operation. Two lowered scalar spatial coefficients agree only when
their complete normalized representations agree, including:

- coordinate dimension and dependency classification;
- instruction tape and selected root;
- ordered canonical Parameter identities; and
- corresponding finite values at the exact Model revision.

This is structural identity at one revision, not numerical value equality.
Two independent Parameters with equal current values remain distinct. A
constant and a Parameter with the same value remain distinct. The same
Parameter expression at incompatible revision-local values does not acquire
cross-revision equality.

This admission rule compares coefficients inside one compiled Model. The
direct/package evidence compares identity-normalized Parameter leaves and
primal/JVP/VJP actions; it does not claim that two authoring frontends already
emit byte-identical instruction tapes.

V1 deliberately does not prove algebraic equivalence. For example, `2 * mu`
and `mu * 2`, or `mu + 0` and `mu`, may remain distinct exact tapes even when
they have equal primal and derivative functions. A proof-producing calculus
and normalization seam is defined by
[RFC 0056](0056-pure-calculus-and-support-maps.md). That work must preserve
this RFC's exact Parameter leaves and may widen an equivalence class only with
an explicit, replayable proof.

### Canonical floating-point boundary

At compiler-owned semantic `f64` boundaries, either spelling of zero is
normalized to positive zero. This applies to typed literal quantities,
Parameter initial values, binding evaluation, and source semantic identity.
Unary negation of zero cannot retain a negative-zero operator tree merely
because the parser observed `-0.0`.

Consequently, otherwise identical `+0.0` and `-0.0` bindings produce the same
Transaction operations, canonical Model v4 bytes, Model digest, and package
semantic digest. The exact source bytes are still different. Under RFC 0022,
the source bundle digest and `SourceBundleIdentityV1` therefore differ, so
diagnostic and supply-chain provenance can recover which spelling was
published without making it mathematical meaning.

This rule is confined to compiler semantic floating-point inputs. It is not a
claim that all runtime arrays, imported datasets, IEEE operations, or result
artifacts erase signed zero.

### Artifact identity and compatibility

This RFC adds no Semantic Kernel node, expression opcode, Model envelope
field, Transaction operation, or decoder fallback. Model v4 and Transaction
v4 remain the explicit closed schemas established by RFC 0038. Existing
stored Model and Transaction artifacts keep their bytes and meaning, and old
transactions replay byte-exactly through their existing versioned path. The
compiler never rewrites an old artifact to the new elaboration result.

Newly compiling hierarchical source may, however, produce different content
from a pre-1.0 compiler. Occurrence-local Parameters that the old compiler
fabricated disappear; direct bindings reuse parent identities; and derived
terms retain their original Parameter leaves. The resulting Transaction and
Model digests may therefore change. This is an intentional correction of
pre-1.0 source elaboration semantics, not an in-place wire migration.

Identity domains remain distinct:

- `ModelPackageIdentityV1` identifies canonical typed package declarations
  under RFC 0022's semantic canonicalization version;
- `SourceBundleIdentityV1` additionally identifies exact inventoried source
  bytes and diagnostic material;
- a Model digest identifies the canonical flattened Relation network emitted
  by the selected compiler;
- a Transaction digest identifies the exact ordered graph edit; and
- package-compilation and Run lineage identify the exact compiler output,
  resolution, source bundles, Model, Realization, and Run records required by
  their respective schemas.

A compiler upgrade alone does not silently relabel an accepted package
semantic identity. Its new Model and compilation identities are recorded as a
new compilation of that exact package source. When the Model digest changes,
downstream package-compilation and Run lineage must change with it; historical
evidence remains attached to its original immutable identities.

## Alternatives considered

### Keep occurrence-local Kernel Parameters

This makes every binding uniformly materialized, but invents independent
mutation and differentiation coordinates that were absent from the source. It
also makes nested forwarding multiply identities. Rejected.

### Compare coefficient values in each physics recognizer

This preserves a narrow primal result but erases tangent and cotangent
meaning, duplicates policy, and lets recognition depend on revision
coincidence. Rejected.

### Materialize every literal as a distinct material Parameter

This could be a valid language feature if the author explicitly declared a
material coordinate. Doing it implicitly for a compile-time literal invents
mutation and AD visibility and makes two identical literals occurrence-
dependent. Rejected for Component bindings; authors who need a coordinate
declare and bind a Model Parameter.

### Fold every binding to its current numerical value

This removes duplicate Parameters but also destroys the parent Parameter
leaves of arithmetic bindings and therefore their chain rule. Rejected.

### Canonicalize algebraically equivalent DAGs now

Commutative reordering and simplification require exact decisions about
floating-point evaluation order, physical types, differentiation, and proof
replay. The exact structural contract closes the present semantic error
without pretending to solve general equivalence. That separate boundary is
[RFC 0056](0056-pure-calculus-and-support-maps.md).

### Add a new Model or Transaction wire generation

No new canonical vocabulary is required: the correct flattened result already
uses constants, existing operations, and existing Parameter references.
Changing the wire would encode an authoring implementation detail in the
Semantic Model. Rejected.

## Falsifying verification

Conformance requires all of the following; a primal-value comparison alone is
insufficient:

1. A volume Component and a boundary Component bound directly to the same
   parent coefficient retain one canonical Parameter identity, including
   occurrence aliases that resolve back to that identity.
2. Otherwise identical volume and boundary laws bound to independent parent
   Parameters with equal current values fail the shared-coefficient contract.
3. A literal binding emits a dimensioned constant and no Parameter node,
   alias, mutation coordinate, JVP coordinate, or VJP coordinate. Changing the
   literal changes the compiled Model, while `+0.0` and `-0.0` have identical
   semantic Model/Transaction bytes and distinct exact source-bundle identity.
4. An arithmetic binding retains its exact parent Parameter leaves and agrees
   with the analytic chain rule. A live power exponent fails before graph
   mutation.
5. Direct and exact-package forms agree in primal coefficient value, Parameter
   JVP, and VJP for one isotropic-elastic boundary and one Newtonian-fluid
   boundary.
6. Existing Model v4 and Transaction v4 fixtures continue exact decode and
   replay without migration or content sniffing.

The registered
[`solid.mixed-boundary-elasticity-2d`](../verify/solid/mixed-boundary-elasticity-2d/README.md)
case owns the elasticity identity, independent-equal-value, primal, JVP, and
VJP falsifiers. The registered
[`fluid.port-closed-si-mini-stokes-2d`](../verify/fluid/port-closed-si-mini-stokes-2d/README.md)
case owns the corresponding Newtonian volume/boundary falsifiers. The
registered
[`fluid.packaged-steady-stokes-2d`](../verify/fluid/packaged-steady-stokes-2d/README.md)
case owns exact package lowering, literal specialization, signed-zero digest
separation, and immutable replay.

Passing one case does not widen another case's numerical, package, boundary,
or differentiation claim.

## Security, safety, and governance

Binding evaluation remains pure, finite, bounded, acyclic, and callback-free.
No native function, Python object, package name, provider label, source span,
or filesystem path enters Parameter meaning. Unknown expression forms,
non-finite values, dimension errors, unsupported live exponents, and resource
limit violations fail before Model mutation.

Changing which source constructs create a Parameter coordinate, widening
coefficient equivalence, or adding a new compiler-semantic floating-point
normalization changes mathematical or identity meaning and requires a later
RFC with replay and falsifying evidence.

## Nonclaims and deferred decisions

This RFC does not provide a general symbolic algebra system, behavioral
equivalence, coefficient proof language, runtime-dependent exponent, structured
Parameter, mutable Component instance object, or cross-revision value
equivalence. It does not claim cross-frontend raw expression-tape identity,
general spatial AD, FSI adjoints, shape
derivatives, hybrid saltation, or algebraic normalization.

The pure calculus and proof-producing normalization seam belongs to
[RFC 0056](0056-pure-calculus-and-support-maps.md).
Higher-order differentiation, coefficient-dependent discretization policy,
and public provenance wire remain separate bounded decisions. None may recover
Component binding identity from current values or package-specific heuristics.
