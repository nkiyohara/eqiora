# RFC 0073: Structural semantic fingerprint

- Status: Implemented; bounded structural comparison
- Authors: Eqiora contributors
- Created: 2026-07-22
- Depends on: [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md), and
  [RFC 0054](0054-curated-facade-and-control-plane.md)

## Summary

Eqiora exposes a generation-tagged structural semantic fingerprint for one
accepted `KernelProgram`. The current generation v4 is the domain-separated digest of a closed,
alpha-normalized, exactly canonically labelled projection of the selected
Semantic Model graph. It supports bounded comparison across independent source,
Rust-native, and Python-native authoring routes without weakening exact Model
artifact identity.

The fingerprint is evidence about structure. Exact artifact identity remains
the sole authority for replay, execution inputs, provenance, and mutation
preconditions.

## Motivation

Direct authoring correctly allocates fresh Model and Kernel occurrence ULIDs.
Canonical Model artifacts retain those identities, so independent
constructions of the same typed relation network have distinct exact digests.
That is required for immutable occurrence and provenance semantics, but it is
the wrong equality for duplicate inspection or route-equivalence tests.

Source text, aliases, names, and package identity cannot fill the gap. They are
presentation or distribution concerns, disappear on exact replay, and vary
between otherwise equivalent frontends. A comparison identity must instead be
derived once from the validated semantic graph shared by every route.

## Ownership and authority

The Semantic Kernel remains the meaning oracle and is unchanged by this RFC.
`eqiora-artifact` owns the comparison projection because it already owns
identity/evidence concerns and may depend on the accepted `KernelProgram`.
Compilers and bindings only delegate to that shared implementation; they do
not maintain route-local normalizers.

The selected Model root is implicit in the closed projection. Its ULID and all
nine Kernel entity ULIDs are alpha-normalized. Distinct entities nevertheless
remain distinct vertices, and every nominal reference and semantic edge keeps
its target relationship. Two equal-valued Parameters cannot collapse into one;
two nominal Domains cannot become shared merely because their payloads match.

## Current projection

The current projection includes geometry identity and Cartesian coordinate sources:

- `GeometryRegion` with the full 32-byte geometry digest and exact entity-set
  name;
- `GeometryBoundary` with its exact entity-set name; and
- their nominal identity and `BoundaryOf` topology through the same graph-edge
  projection as every other Domain; and
- tagged fixed Cartesian endpoints or nominal Parameter references, retaining
  ordered axes and lower/upper endpoint roles.

The projection also includes:

- every admitted Domain, Representation, Field, Parameter, Port, Relation,
  Activation, Connection, and ClockDomain definition;
- revision-local current quantities and Model boundary membership;
- exact rational dimensions, value shapes, frames, supports, connector and boundary
  references;
- residual and guard expression DAGs, ordered roots, symbol roles, pure
  operator definitions, and their complete content identities;
- Activation and exact Clock meaning; and
- every typed Semantic Model edge and physical connection topology.

It excludes:

- actual Model and entity ULID bytes and graph revision;
- source/display names, aliases, filenames, spans, formatting, and admitted
  declaration order;
- package namespace, package version, dependency/provenance records, and
  compiler build identity; and
- Model artifact codec and exact artifact digest.

Expression-arena allocation IDs are local implementation details and are also
alpha-normalized. Root order and non-commutative operand order remain meaning.
Finite binary64 values retain exact bits except that both signed zeros encode
as mathematical zero. Non-finite quantities are rejected by this projection.

The projection is closed over the vocabulary explicitly enumerated above. A
future node, edge, expression, symbol, or enum variant is not silently omitted:
construction returns a diagnostic until the feature explicitly extends its
projection. All constructors emit generation v4, including scalar and fixed-box
Models. There is no vocabulary-dependent generation selection or older encoder.

## Exact canonical labelling

### Mutation policy

Comparison belongs to the accepted Model graph, not to its source container or
execution plan. Each newly admitted construct extends that same projection;
the table does not make unimplemented vocabulary executable.

| Mutation | Model structural comparison | Identity that retains the change |
| --- | --- | --- |
| Prose, notation, or identifier rename | Unchanged when the accepted graph is unchanged | Authored source/package bytes |
| Declaration order | Unchanged when nominal relationships are unchanged | Authored source/package bytes |
| Equation root order or noncommutative operand order | Changed | Model structure and exact artifact |
| Law, initial value, reset law, or mathematical table | Changed when admitted as Model meaning | Model structure and exact artifact |
| Fixed endpoint replaced by an equal-valued Parameter reference | Changed: the nominal dependency is meaning | Model structure and exact artifact |
| Provider binary swap with the same Model | Unchanged | Provider and execution provenance |
| Mesh or Formulation choice with the same Model | Unchanged | Mesh, Formulation, and execution artifacts |
| Referenced Geometry digest or entity-set name | Changed | Model structure and exact artifact |
| Source move | Unchanged when the accepted graph is unchanged | Source/package closure |
| Output cadence outside the Model | Unchanged | Execution/result contract |
| Model clock or activation contract | Changed | Model structure and exact artifact |

Changed scientific meaning must not be erased by numerical sampling or general
algebraic simplification. Future derivatives retain kind, held-fixed bindings,
axis order, and derivative order; complex expressions retain conjugation and
adjoint distinctions. Reduced, stochastic, and approximate models retain their
assumptions and meaning-bearing dependencies. Their owning features implement
these boundaries when they admit the corresponding vocabulary.

### Algorithm

The projection is an attributed directed multigraph. Vertex intrinsic bytes
contain typed payloads but no occurrence identifier. Labelled outgoing edges
carry semantic edges, nominal references, and expression-symbol roles.

Canonicalization proceeds as follows:

1. partition vertices by complete intrinsic bytes;
2. refine partitions using complete sorted incoming and outgoing labelled
   target-cell signatures;
3. when symmetry remains, individualize each candidate and recursively refine;
4. serialize the lexicographically least discrete labelling; and
5. hash the bytes with the domain
   `eqiora.structural-semantic-fingerprint/v4` using SHA-256.

Refinement uses complete bytes, not a probabilistic intermediate digest. The
individualization search is exact; occurrence order may affect traversal only,
never the selected minimum. One global best certificate is retained across the
search rather than one per recursion level. Node, reference, expression,
per-certificate byte, cumulative serialization-byte work, search-state, depth,
and refinement-work limits bound resource use. Exceeding any limit returns
`EQ0901` and no fingerprint. There is no approximate fallback.

The comparison API constructs both canonical projections. If their typed
fingerprints differ it returns false. If their digests agree, it also compares
the private canonical bytes; unequal bytes are reported as a collision rather
than semantic equality.

## Public API

The public Rust facade exposes:

```text
ModelDocument::structural_fingerprint()
ModelDocument::structurally_equivalent(other)
StructuralSemanticFingerprint { generation, digest }
```

The public type is version-neutral; its explicit generation is part of
equality and display. Internal construction limits are an admission policy and
do not alter accepted generation-v4 bytes.

Python exposes the same boundary as the frozen
`StructuralSemanticFingerprint`, `Model.structural_fingerprint`, and
`Model.structurally_equivalent(other)`. The comparison releases the GIL and
still executes only the shared Rust contract. `Model.__eq__` is unchanged.

## Verification

The registered case proves:

- fresh source compilations with renamed declarations and admitted reordering
  have distinct exact artifact references but equal structural fingerprints;
- source, Rust-native draft, Python-native draft, and current exact replay
  routes preserve the stated identity boundary;
- scalar and scalar-physical graphs compare across authoring routes;
- expression-arena allocation order and signed zero do not change the result;
- values, operators, symbol rewiring, and nominal Domain sharing do change it;
- geometry digest, region name, boundary name, and boundary-parent topology
  change it, while fresh occurrence IDs do not; and
- a deliberately exhausted exact-labelling budget fails without producing a
  route-dependent value.

The fixtures are intentionally bounded. The projection encodes the full
current vocabulary, while future authoring-route claims require their own
evidence when those surfaces exist.

## Alternatives considered

- **Exact artifact digest.** Retained as the authoritative occurrence identity;
  it intentionally differs across independent construction.
- **Normalized source or aliases.** Rejected because replay has no source
  presentation and renaming would alter the result.
- **Package semantic digest.** Rejected because publisher, version,
  dependencies, and distribution identity are intentionally different axes.
- **Generate persistent ULIDs from structure.** Rejected because comparison
  evidence must not become occurrence authority.
- **One-dimensional refinement only.** Rejected because it does not decide
  isomorphism for all symmetric graphs.
- **A universal cache key or durable fingerprint wire.** Not needed for the
  bounded comparison consumer and would prematurely create a compatibility
  surface.

## Identity boundary

Fingerprint generations are independent of Model artifact codecs and compiler
crate versions. Equality is defined only for equal explicit generations.
Every current construction uses v4. Previous comparison generations are not
accepted or constructed; callers recompute comparison identities from current
Models.

This RFC does not define persistent entity identity, compilation-result cache
semantics, a durable fingerprint artifact, mathematical equivalence, automatic
merge, collaborative editing, migration of existing artifacts, or execution
authority. Those concerns must continue to use their owning contracts.

## Security and resource bounds

Canonical graph labelling may be expensive on highly symmetric inputs. All
semantic construction and search dimensions are bounded and checked before or
during work. Integer overflow, unknown vocabulary, malformed references,
unreachable or cyclic expression structure, limit exhaustion, and digest
collision fail closed with no partial public identity. Process-level allocator
failure follows ordinary safe-Rust behavior and is not converted into a product
diagnostic.
