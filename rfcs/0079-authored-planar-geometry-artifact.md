# RFC 0079: Authored planar geometry artifact

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-07-28
- Related RFCs and evidence:
  [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md), and
  [`geometry.authored-planar-geometry-artifact`](../verify/geometry/authored-planar-geometry-artifact/README.md)

## Summary

One opaque geometry-layer value owns the validated content, canonical JSON, and
domain-separated identity of a `straight-edged-planar-v1` region. The artifact
layer admits externally supplied bytes only when bounded decoding,
`PlanarRegion` revalidation, and byte-for-byte canonical reconstruction all
agree.

## Motivation

Model v7 can persist a geometry digest and entity-set name, but a digest alone
does not prove that any geometry content exists or that the named set has the
required dimension. The first authored-region implementation could produce
bytes internally but exposed no external decoder, registered evidence, or
independent identity oracle. Private wire mutation tests could not exercise the
trust boundary an external artifact crosses.

Identity must also live below semantic admission. Passing a digest beside
caller-supplied entity facts would let safe Rust forge the relationship that
admission needs to prove. Moving the artifact type wholesale into
`eqiora-geometry` cannot preserve the repository's `ArtifactDigest` return type
without a reverse dependency.

## Proposed design

### Ownership

`eqiora-geometry::CanonicalGeometryV1` contains, privately:

- one validated canonical `PlanarRegion`;
- the exact canonical JSON bytes derived from that region; and
- `sha256(schema || NUL || bytes)`.

It is constructed only from a `PlanarRegion` or through bounded decoding that
revalidates to the same bytes. It has no constructor accepting an independent
digest, entity-set catalog, or semantic facts.

`eqiora-artifact::GeometryDefinitionV1` remains a small wrapper. Its existing
`from_region`, `region`, `canonical_json`, and `digest` signatures remain
source-compatible and byte-compatible. It adds explicit decoder limits,
external JSON admission, and a read-only accessor to the lower canonical value.
The wrapper alone translates raw digest bytes into `ArtifactDigest`.

This RFC adds no semantic admission, Model vocabulary, or dependency from
`eqiora-sem`.

### Straight-edged planar v1 content

Coordinates are finite binary64 values in metres. Classification tolerance is
one finite positive binary64 metre value and is part of identity. A region has
one or more planar faces, each with one outer straight-edge loop and zero or
more hole loops. Named sets contain canonical primitive indices of exactly one
topological dimension.

Entity-set names are unique across the whole region, not merely within a
dimension. This makes the existing `entity_set(name)` accessor and later
Model-side bare string reference functions rather than ambiguous searches.
Names are exact, untrimmed UTF-8 strings for identity and lookup; leading or
trailing whitespace is significant, while a name that is empty after trimming
is rejected.

Every vertex pair must be separated by strictly more than the positive
classification tolerance in Euclidean distance. The predicate is all-pairs in
meaning. Its deterministic implementation sweeps the already x-major-sorted
vertices, retains prior points whose x distance is at most the tolerance,
queries a total-ordered y window, and decides candidates with overflow- and
underflow-safe `hypot`. The accepted case is `O(n log n)` because a
tolerance-width band can contain only a constant packing of mutually separated
points; a violating case stops on its first witness.

Entity-set members are already indices in the resulting canonical primitive
enumeration. They are not author-relative and are never remapped:

- vertices use the canonical coordinate order;
- faces use the canonical outer-loop order; and
- edges traverse canonical faces, each outer loop first and then each canonical
  hole, with the closing edge implied.

An authoring surface may calculate these indices for a user. That ergonomic
projection is not persisted identity.

### Canonical order

The complete normalization is:

1. normalize every coordinate's negative zero to positive zero;
2. sort vertices lexicographically by `(x, y)` and remap loop indices;
3. orient outer loops counter-clockwise and holes clockwise;
4. rotate each loop so its smallest vertex index is first;
5. sort holes and faces lexicographically by canonical index sequence;
6. sort entity-set members ascending and deduplicate them; and
7. sort entity sets by `(dimension ascending, name byte order)`.

Compact UTF-8 JSON has no whitespace, uses kebab-case declaration-order fields,
and uses the repository's `serde_json` canonical-v1 binary64 rendering. That
renderer uses shortest round-trip significant digits together with its fixed
plain-versus-exponent presentation rules; identity is pinned to the resulting
bytes, not to the shortest character-count spelling of the same value. Private
wire structs reject unknown fields. The exact schema and encoding are:

```text
eqiora.geometry-definition-envelope/v1
eqiora.canonical-json/v1
```

The kind is `straight-edged-planar-v1`; the length unit is `metre`.

### External admission and budgets

Admission proceeds in one closed order:

1. the artifact JSON preflight rejects encoded byte and nesting-depth excess
   before deserialization;
2. the geometry decoder independently enforces its 4 MiB family byte cap and
   deserializes private, unknown-field-denying wire structs;
3. vertex, face, loop-index, entity-set, and member totals are checked before
   geometric work; the shipped limit of 4,096 total loop indices directly
   bounds the current quadratic segment-intersection validation;
4. `PlanarRegion::new` revalidates coordinates, topology, orientation,
   containment, tolerance separation, set names, dimensions, and membership;
5. the admitted region is re-encoded through the only canonical producer; and
6. reconstructed bytes must equal the supplied bytes exactly before identity
   is returned.

Semantic counts are checked after bounded JSON deserialization rather than by a
second streaming JSON implementation. The byte cap bounds that allocation.
The loop-index limit is deliberately much lower than the byte-derived maximum
because it directly bounds the superlinear geometric validation stage; future
subquadratic validation may justify a reviewed increase without changing wire
identity.

Geometric validation is deliberately the existing binary64
`PlanarRegion` validation, not an exact-real predicate kernel. Segment crossing
uses binary64 orientation signs and containment uses binary64 ray crossing.
Near-collinear or exactly degenerate embeddings can therefore be rejected or
classified differently from exact arithmetic. Admission proves that the
declared indexed loops pass this validator; it does not prove robust CAD
topology for every degenerate binary64 input.

Whitespace, author order, loop rotation, or field reordering in external JSON
is rejected instead of silently normalized. Equivalent programmatic
authorings first become one `PlanarRegion`, then one artifact identity.

### Identity and equivalence

The frozen digest is:

```text
sha256(
  b"eqiora.geometry-definition-envelope/v1"
  || 0x00
  || canonical_json
)
```

Equality claims only exact canonical content identity. It does not claim
geometric congruence, rigid-motion equivalence, tolerance-based equality, or
equivalence between distinct curve/solid representations.

Every declared vertex remains identity-bearing even when no face loop
references it. Canonicalization neither prunes such vertices nor equates a
region containing them with the same indexed faces without them.

## Alternatives considered

### Keep wire and hashing only in the artifact crate

This preserves the initial file layout but leaves semantic admission unable to
consume content without depending upward on the artifact layer or accepting a
forgeable digest/facts pair. It is rejected.

### Move `GeometryDefinitionV1` wholesale into geometry

The moved type could not retain an inherent `digest() -> ArtifactDigest`
without a reverse dependency, and a re-export cannot add inherent methods. It
is rejected in favor of the lower opaque value and upper wrapper.

### Accept noncanonical JSON and normalize it

That makes the supplied bytes and the content digest disagree about which
object was admitted, or gives one content multiple identities. It is rejected;
producers may normalize before crossing the artifact boundary.

### Pre-count every semantic item with a second streaming parser

That duplicates JSON syntax handling and creates a permanent agreement
obligation with `serde_json`. The 4 MiB pre-deserialization cap already bounds
allocation; semantic counts run before geometry validation. The duplicate
parser is rejected.

## Compatibility and migration

The schema string, JSON field order, and `(dimension, name)` entity-set ordering
remain unchanged. Regions without negative-zero coordinates that already pass
the all-pairs separation rule and whole-region entity-set name uniqueness
retain their canonical bytes and digest.

This slice does contain three explicit migrations. Negative-zero coordinates are
normalized to positive zero, so a previously admitted region containing
`-0.0` receives new canonical bytes and a new digest. The separation check now
examines every vertex pair rather than only adjacent vertices in sorted order,
so inputs with a non-adjacent pair at or below the tolerance are rejected.
Completing whole-region entity-set name uniqueness additionally rejects inputs
that contradicted the documented accessor and uniqueness diagnostic.

Historical Model and Transaction generations, semantic fingerprints, geometry
identity, correspondence, CAD, and remeshing artifacts are unchanged. This RFC
does not make a Model geometry reference admissible; that is a dependent slice.

The two exported geometry names raise the frozen `eqiora-geometry` public
surface from 30 to 32. The decoder-limit wrapper raises `eqiora-artifact` from
154 to 155. Both are explicit reviewed public-capability additions.

## Verification

The independent oracle uses a dyadic unit square with a centred square hole and
dyadic tolerance `0.0625`. A Python derivation written by the non-implementing
contract lane emits the RFC-defined bytes without reading Rust:

```text
bytes: 482
sha256: e6f8e17ac215ef37ca3c9de07b9979e34f13412a5de11dc9240ea1def8130030
```

The registered Rust evidence must agree exactly, externally decode those bytes,
and replay the same region. Mutants cover whitespace, author order, rotated
loops, a filled hole, invalid topology and membership, duplicate names across
dimensions and author orders, unknown wire vocabulary, each resource budget,
signed-zero canonicalization, all-pairs tolerance boundaries, and both
digest-framing components. Existing geometry, correspondence, CAD, and
remeshing cases remain migration falsifiers. The registered test executes the
independent derivation and therefore requires a host `python3`; absence fails
the evidence rather than skipping it.

## Security, safety, and governance

No unsafe code, filesystem access, network access, or code execution is added.
All external allocation is preceded by byte/depth limits. Semantic count limits
precede geometry work, including a direct 4,096-index ceiling before the
quadratic intersection checks. SHA-256 provides content identity, not
authenticity or authorization. The independent oracle is frozen by an agent
that does not implement the Rust slice, and the complete diff requires
cross-model review before integration.

## Unresolved questions

Curves, general 3D geometry, booleans, CAD representations, Model admission,
semantic spatial support, mesh realization, and geometric-equivalence policy
are deliberately unresolved here.
