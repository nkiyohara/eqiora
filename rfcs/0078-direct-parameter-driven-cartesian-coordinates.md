# RFC 0078: Direct Parameter-driven Cartesian coordinates

- Status: Accepted; implementation is tracked by
  the semantics-and-replay slice, and atomic
  regeneration by the value-transaction slice
- Authors: Eqiora contributors
- Created: 2026-07-24
- Depends on: [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md),
  [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md),
  [RFC 0055](0055-component-parameter-terms.md), and
  [RFC 0077](0077-exact-cartesian-domain-edit.md)

## Summary

One Cartesian coordinate endpoint is exactly one fixed coherent-SI length or
one reference to an exact root Model Parameter:

```text
CartesianCoordinateSource =
    Fixed(length quantity)
  | Parameter(exact root Model Parameter ID)
```

A Cartesian Domain persists only these coordinate sources. It does not also
persist evaluated bounds. `eqiora-sem::KernelProgram` is the single owner that
resolves the sources against revision-local Parameter values and returns
validated `AxisBounds` to every artifact, geometry, API, and numerical
consumer.

The Domain records each referenced Parameter through the existing typed graph
vocabulary:

```text
Domain --DependsOn--> Parameter
```

The set of Parameter IDs present in the coordinate definition must equal the
set of outgoing `DependsOn` targets exactly. Model and Transaction v7 persist
this meaning. Historical v1--v6 codecs remain frozen.

## Motivation

The topology-preserving edit in RFC 0077 proves that a complete set of
materialized Cartesian intervals can be replaced atomically while retaining
semantic body and boundary identities. It intentionally does not explain why
the new intervals exist. A Parameter-driven editor needs that dependency to be
Model meaning rather than client state, but the current Cartesian Domain
persists only evaluated `AxisBounds`.

A dependency edge alone is insufficient: it cannot identify which axis and
which lower or upper endpoint the Parameter supplies. A CAD or client sidecar
that carries that missing information would become a second semantic
authority. Persisting both an evaluation recipe and its current result inside
the Domain would instead create two canonical truths that every decoder,
transaction, and consumer must keep synchronized.

The smallest complete semantic object is therefore a closed coordinate source
in the Domain definition plus one whole-Model evaluator. Direct Parameter
leaves prove identity, typing, persistence, regeneration, and client
ownership without committing Eqiora to a geometry expression language.

## Source and schema contract

### Closed source spelling

Inside a root `model`, each argument of `box(...)` is either a signed numeric
literal or one unqualified root Model Parameter name:

```text
model adjustable_box {
  parameter extent: m = 1;
  domain body = box(0, extent, 0, extent, 0, 1);
}
```

The numeric spelling lowers to `Fixed`. The bare name lowers to `Parameter`.
Calls, qualified paths, unary operations on names, binary arithmetic, powers,
fields, ports, time, and Component Parameter names are not coordinate-source
syntax. A negative fixed literal remains admitted; `-extent` does not.

Parsing owns only this closed syntactic distinction and source ranges. It does
not infer units or evaluate names. Formatting preserves axis and endpoint
order and emits no implicit expression.

### Canonical definition

The Semantic Kernel Cartesian Domain stores one non-empty ordered axis list.
Each axis stores one lower and one upper `CartesianCoordinateSource`.

- `Fixed` contains one finite quantity with physical dimension length.
- `Parameter` contains one typed `Id<Parameter>`.
- axis order is physical coordinate order and is semantic;
- lower and upper roles are semantic;
- the same Parameter may occur at more than one endpoint in one Domain; and
- repeated references create one dependency-set member, not duplicate edges.

The first v7 admission allows one coordinate Parameter to belong to at most
one Cartesian Domain definition. Reusing it at several endpoints of that one
Domain is valid; sharing it across two Domains is rejected. This is not a
mathematical necessity. It keeps the first authoring and regeneration owner
total while multiple-Domain regeneration remains a nonclaim.

The schema layer validates the local closed shape, typed IDs, fixed dimensions,
and finite fixed values. It cannot prove that a referenced Parameter exists,
has a current value, or produces an increasing interval; those are
whole-Model obligations.

### Name and type resolution

The compiler resolves a coordinate name only against an exact scalar
Parameter declared in the enclosing root Model. It rejects an unknown name,
another entity kind, a Component Parameter, an occurrence member, and a
Parameter whose declared dimension is not length. It emits the exact resolved
Parameter ID in the Domain definition and one `DependsOn` edge for every
distinct referenced ID.

The existing two meanings named Parameter remain separate:

- a root Model Parameter is a canonical node with revision-local value,
  mutation identity, and differentiation identity; and
- an RFC 0055 Component Parameter is a compile-time lexical term that
  disappears during elaboration.

This RFC does not reinterpret the latter as a persisted geometry coordinate.

## Whole-Model dependency and evaluation

### Exact dependency equality

`DependsOn` remains one general typed dependency relation. Its admitted
Semantic Kernel endpoints extend from
`Relation -> Field | Parameter | Port` to also include
`Domain -> Parameter`. A new geometry-specific edge kind would duplicate the
same consumer-to-input meaning.

For each Cartesian Domain, whole-Model validation derives the set of Parameter
IDs from all coordinate sources and compares it with the exact outgoing
`DependsOn` target set. Missing, extra, wrong-kind, dangling, or
outside-Model targets fail. The existing Relation invariant remains unchanged:
its symbol set must still equal its own dependency targets. A reverse index
over the validated Domain reference sets enforces the first-v7 one-Domain
ownership restriction without making that index persisted meaning.

### One resolved-bounds owner

`KernelProgram` resolves Cartesian bounds after definitions, revision-local
values, and edges have been selected from one immutable Model revision. For
each endpoint, it:

1. copies a `Fixed` length quantity, or finds the exact referenced Parameter;
2. requires one finite revision-local scalar value with physical dimension
   length;
3. constructs each axis in canonical axis order; and
4. requires finite, strictly increasing `AxisBounds`.

Evaluation is a direct bounded projection over coordinate leaves. There is no
evaluation graph, recursion, cycle, scheduling policy, or solver.

The implementation exposes one
`KernelProgram::resolved_cartesian_bounds(...)`-style semantic query. Existing
Geometry Identity, CAD validation, API, numerical, and other metric-bound
consumers must migrate from matching and interpreting raw Cartesian payloads
independently to consuming that query. The Model v7 encoder necessarily
serializes the coordinate definition itself; other artifact producers that
need evaluated metric bounds use the semantic query. A private in-memory cache
is permitted only as a derivable optimization; it is not persisted meaning,
an artifact, or a second public contract.

The first implementation inventory is closed:

- `eqiora-sem` Domain, boundary, Field-support, and physical-boundary
  validation consume the resolved projection;
- Geometry Identity and bounded CAD target validation in `eqiora-artifact`
  consume the resolved projection;
- Cartesian recognition and lowering in `eqiora-numerics` consume the
  resolved projection; and
- `eqiora-api` metric projections and edit previews consume the resolved
  projection.

The Model/Transaction encoders, compiler canonical-declaration identity, and
structural semantic fingerprint are definition consumers: they encode or
compare coordinate sources and exact Parameter references rather than
evaluated bounds. A new metric consumer may not evaluate sources independently
merely because it was absent from this initial inventory.

### Diagnostic ownership

Diagnostics remain at the narrowest authoritative layer:

- the language reports malformed endpoint syntax and exact source ranges;
- the compiler reports name, kind, scope, declared-dimension, and non-finite
  source-initial-value failures;
- the graph schema reports an inadmissible edge endpoint pair;
- Semantic Kernel validation reports dependency-set or one-Domain-ownership
  mismatch, absent or invalid decoded/current revision-local values, and
  resolved interval failure; and
- artifact decoders report unknown versions, unknown variants, malformed
  IDs, limits, and locally invalid payloads before graph mutation.

Whole-Model diagnostics identify the Domain, zero-based axis, endpoint role,
and referenced Parameter when applicable. Clients do not recreate these
checks.

## Canonicalization and identity

Coordinate sources are encoded in axis order and lower-before-upper order.
`Fixed` and `Parameter` are distinct tagged alternatives. Resolved Parameter
IDs, rather than source aliases or declaration traversal order, enter
canonical Model meaning. Dependency targets are deduplicated and sorted by
exact ID. Declaration and dependency input permutations cannot change the
emitted canonical transaction or Model bytes.

The existing compiler rule normalizes signed source zero for fixed coordinates
and Parameter initial values. The regeneration slice applies the same rule to its requested
replacement value before no-op detection, plan identity, or transaction
construction, so `+0` and `-0` requests cannot produce different regeneration
plans. Exact artifact replay does not silently rewrite an already persisted
floating-point payload.

Both the coordinate definition and current Parameter values contribute to
exact Model identity through their existing domains. Changing a referenced
Parameter value therefore changes the Model digest even though the immutable
Domain definition and graph topology remain unchanged. The newly resolved
bounds change Geometry Identity through its existing exact Model-bound
projection.

A v7 Domain containing only `Fixed` sources has the same bounded mathematical
meaning and resolved `AxisBounds` as the corresponding historical fixed-bounds
Domain. A conformance fixture may prove structural equivalence across those
two exact artifacts. It does not claim equal Model bytes, transaction bytes,
digests, source provenance, or general cross-generation equivalence.

The compiler's package canonical-declaration identity includes the closed root
Model coordinate syntax and resolved declaration identity. The semantics-and-replay slice must
prove that changing `Fixed` to `Parameter`, or changing the referenced root
declaration, changes the package semantic digest, while source/declaration
permutations do not. Historical package identities are not relabelled. This
changes no `eqiora-package` payload and adds no package-instance or Component
Parameter binding behavior.

## Exact wire generation and compatibility

Model v7 is required. Model v6 stores only concrete axis bounds and has no
field from which a decoder can reconstruct whether an endpoint is fixed or
which exact Parameter drives it. Encoding the recipe in an unrelated artifact
would not repair that missing Model meaning.

Transaction v7 is also required. Initial source compilation and every typed
construction of the new Domain definition must carry it through
`DefineKernelNode`; a new Model wire without the matching definition grammar
in the ordinary transaction path would make authoring and replay disagree. No
new graph operation is needed.

Compatibility is explicit:

- v1--v6 Model and Transaction schemas, bytes, digests, semantics, golden
  fixtures, exact entry points, and rejection behavior remain frozen;
- v1--v6 reject the parametric coordinate payload;
- v1--v6 Model and Transaction codecs also reject
  `Domain --DependsOn--> Parameter`, even though the shared in-memory
  `EdgeKind` learns that endpoint pair;
- ordinary current authoring moves from the explicit v6 codec to v7;
- v7 decoding never sniffs, retries, or falls back to an older generation;
- no historical artifact is automatically upgraded or rewritten; and
- recompiling old fixed-literal source under current authoring may produce new
  v7 bytes and digests while preserving the bounded fixed geometry meaning.

The first v7 implementation must add both exact codec selectors and keep every
historical selector independently callable. It may not silently widen an
older encoder merely because the in-memory graph vocabulary learned the new
Domain-to-Parameter endpoint pair.

Moving current authoring to v7 must not remove RFC 0077's fixed-geometry edit
capability. That slice therefore migrates the existing fixed-only
`CartesianDomainEditPlan` consumer to v7 coordinate definitions: an accepted
edit replaces the selected axis endpoints with `Fixed` sources and otherwise
retains RFC 0077's exact identity and atomicity contract. Historical v6 plans
remain available through their exact v6 document path. A direct bounds edit
against any Domain containing a `Parameter` coordinate is rejected; the regeneration slice
owns that change.

## Regeneration and RFC 0077 reuse

RFC 0077 owns direct replacement of materialized fixed bounds. Its public
`CartesianDomainEditPlan` includes a transaction that removes and redefines
the Domain once. Parameter-driven regeneration does not compose or execute
that transaction: the coordinate definition is unchanged.

The implementation of RFC 0077 also owns a smaller validated, canonical
axis-keyed edit-set seam. The regeneration slice reuses that internal seam to compare the
before and after `KernelProgram` projections. It then emits one ordinary
Parameter `SetValue` transaction:

```text
RevisionIs(base)
ValueEquals(parameter, before)
SetValue(parameter, after)
```

No Domain node or incidence edge is removed, redefined, or reconnected.
Preview resolves all affected endpoints before emitting the transaction and
records the complete canonical difference and expected child identity. Commit
replays the exact transaction against the exact base and must reproduce the
previewed child.

The first plan requires exactly one affected Cartesian Domain; the semantic
admission above prevents a geometry Parameter from having more. A Parameter
with no coordinate reference remains an ordinary value and continues through
`ValueEditPlan`. `ValueEditPlan` must reject a Parameter with one geometry
dependency, directing it to the regeneration owner. This keeps impact
calculation, Geometry Identity transition, and regeneration evidence behind
one application owner. Studio, Python, CAD adapters, and other clients consume
that owner; they do not compose a value edit with independent Domain commits.

## Follow-up slices

The decision is intentionally split into two implementation issues:

1. The semantics-and-replay slice owns source,
   compiler, schema, dependency validation, `KernelProgram` evaluation,
   the closed metric-consumer inventory, compiler package-declaration identity,
   exact Model/Transaction v7 replay, fixed-only ModelDraft compatibility, and
   migration of RFC 0077's fixed edit plan to current v7. Its proving Model
   uses one root length Parameter at two endpoints of one 3D Cartesian Domain.
2. The regeneration slice owns one immutable
   Parameter-driven regeneration plan, one exact value transaction, the
   complete two-axis difference, Geometry Identity transition, and retained
   selection evidence.

Parameter-referenced ModelDraft authoring, Python, Studio, Component Parameter
forwarding, batch editing, multiple Domain semantics/regeneration, mesh
regeneration, and wider CAD behavior may fan out only after those two owner
slices close. Existing fixed-only ModelDraft behavior is compatibility work in
the semantics-and-replay slice, not a deferred capability.

## Alternatives considered

| Formulation | Semantic authority | Evaluation cost | Initial implementation | Compatibility and extension |
|---|---|---:|---|---|
| Closed direct coordinate sources | One Model definition | Linear in endpoints | Moderate v7 and consumer migration | Explicit generation boundary; later terms require a new decision |
| Recipe plus materialized bounds | Two synchronized values | Linear plus equality checks | Lower consumer churn | Every edit and decoder inherits consistency risk |
| CAD or geometry sidecar | Split across artifacts | Lookup plus external replay | Low kernel churn | Model replay is incomplete without optional state |
| Residual or generic expression DAG | One but over-broad authority | Expression evaluation | High typing, canonicalization, and policy cost | Prematurely fixes geometry algebra and differentiation |
| Component Parameter reuse | Compile-time/runtime mismatch | Elaboration-dependent | High hierarchy coupling | Conflicts with RFC 0055 identity semantics |
| Shared Parameter across Domains | One Model definition | Linear in all dependents | Requires multi-Domain edit closure | Natural later extension, but leaves the bounded first editor incomplete |

### Persist the recipe and materialized bounds

This minimizes immediate consumer migration, but creates two canonical
representations of the same geometry. Every compile, decode, value edit, and
replay would have to prove equality between them. A mismatch would have no
principled source of truth. Rejected despite its lower short-term
implementation cost.

### Persist a CAD or geometry sidecar

A sidecar could map Parameters to endpoints while leaving v6 unchanged, but
the same Model could then mean different geometry depending on which optional
artifact a client supplied. That violates the Model/Realization separation and
makes semantic replay incomplete. Rejected.

### Reuse the residual `ExprDag`

Residual expressions include runtime Fields, Ports, derivatives, state
operators, pure operators, activation context, and differentiation behavior.
Admitting that vocabulary for a coordinate endpoint would couple geometry
definition to Relation execution and introduce many meaningless forms.
Rejected.

### Introduce a generic geometry expression language

An affine or general typed expression could describe widths, centers,
constraints, and derived coordinates. It would also require operator
canonicalization, dependency and cycle rules, evaluation order, resource
limits, differentiation policy, and failure semantics before the direct-leaf
case needs any of them. The direct sum is mathematically complete for the
first falsifier and leaves a clean later extension point. Generic expressions
are deferred, not implicitly accepted.

### Reuse Component Parameter terms

RFC 0055 terms are lexical compile-time binders that substitute into a flat
Relation network. Treating them as revision-local geometry inputs would either
fabricate occurrence Parameters or retain an authoring hierarchy as a second
runtime model. Rejected. Direct Component forwarding, constants, and derived
geometry terms require a later decision.

### Add `ParameterizedBy`

The existing `DependsOn` direction already means that one semantic definition
consumes exact inputs and already has an exact reference-set equality rule for
Relations. A geometry-specific synonym adds vocabulary without separating a
different mathematical relationship. Rejected.

### Admit one coordinate Parameter in several Domains now

Shared dimensions across several bodies are mathematically natural. Admitting
them now would require the regeneration slice to own complete multi-Domain impact,
association, and selection evidence or would leave a valid Model with no
accepted value-edit path. The first v7 instead rejects cross-Domain sharing.
A later extension must add the complete atomic dependent set before relaxing
that admission rule.

### Compose client-side value and Domain plans

This exposes partial or inconsistent successors, duplicates dependency
evaluation across clients, and either rewrites an unchanged recipe or permits
a value change without complete impact evidence. Rejected. The application
owner emits one ordinary value transaction after semantic preview.

## Falsifying verification

The semantics-and-replay slice must falsify the semantic and persistence contract with at least:

- unknown, wrong-kind, Component-scoped, or non-length coordinate names;
- non-finite or absent revision-local Parameter values;
- one coordinate Parameter referenced by more than one Domain;
- missing, extra, wrong-kind, dangling, or foreign `DependsOn` targets;
- a coordinate reference set that differs from the dependency target set;
- evaluation producing equal or reversed bounds;
- declaration or dependency permutations changing canonical v7 Model or
  Transaction bytes and digests;
- v6 accepting any parametric coordinate payload;
- any v1--v6 Model or Transaction codec accepting a
  `Domain --DependsOn--> Parameter` edge;
- an unknown or forged v7 coordinate alternative replaying;
- a fixed-only v7 Domain changing the bounded historical fixed geometry
  meaning; and
- fixed-only ModelDraft or RFC 0077 editing disappearing under current v7;
- direct fixed-bound editing admitting a Parameter-backed Domain;
- package canonical-declaration identity ignoring the fixed/Parameter choice
  or exact root Parameter declaration; and
- any metric consumer in the closed implementation inventory producing bounds
  that differ from the `KernelProgram` projection.

The regeneration slice must independently falsify regeneration with at least:

- stale, foreign, wrong-target, same-revision/different-digest, non-finite,
  no-op, and non-length requests;
- omission of either affected endpoint from the canonical axis edit set;
- an after-value producing an invalid interval;
- direct `ValueEditPlan` admission of the geometry-driving Parameter;
- `+0` and `-0` requests producing different plans or transactions;
- any Domain remove/redefine or incident-edge reconnect in the transaction;
- caller-order variation changing plan identity, child bytes, or digests;
- preview and commit disagreement on Model or Geometry Identity; and
- a partial child passing an independently compiled target-bounds and volume
  oracle.

No implementation may claim the capability before its exact bounded case is
registered and the capability matrix states the same boundary.

## Security and resource bounds

Direct coordinate sources make evaluation linear in the already-bounded
number of Cartesian endpoints. Existing Model and Transaction decoder limits
bound nodes, edges, axes, values, and canonical bytes before mutation. Exact
typed IDs and Model closure prevent ambient lookup. There is no file, network,
callback, plugin, or expression-evaluator authority.

All coordinate sources, dependency equality, current values, and complete
resolved intervals are validated before regeneration emits a transaction.
Preview and commit operate on immutable or cloned state and expose no partial
Model.

## Nonclaims

This RFC does not claim:

- affine, nonlinear, generic, symbolic, or constraint geometry expressions;
- Component Parameter forwarding, package-instance binding, or derived
  coordinate terms;
- multiple-Parameter batch edits, one Parameter shared across multiple Domain
  definitions, or multiple-Domain regeneration;
- source rewriting, incremental source editing, or source-span persistence in
  Model artifacts;
- Parameter-referenced ModelDraft authoring, Python, Studio, CAD-kernel,
  mesh-regeneration, ALE, remeshing, optimization, or shape-sensitivity
  support;
- a new graph operation, regeneration wire or artifact, generic edit context,
  scheduler, dynamic plugin, or trait registry; or
- automatic artifact migration, decoder fallback, or compatibility inferred
  from mathematical equivalence.
