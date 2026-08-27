# RFC 0015: Bounded Gmsh simplex import

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-18

## Summary

Eqiora admits a narrow external-mesh boundary: ASCII or binary Gmsh MSH 4.1
containing full-dimensional linear triangles in the XY plane or linear
tetrahedra in XYZ. A dedicated L3 adapter applies explicit byte and count
limits, rejects unsupported semantics, decodes the admitted grammar through an
Eqiora-owned bounded parser, and reconstructs the result through the existing
L2 `SimplicialMesh` contract.
The resulting mesh, not the input path or importer state, is the authority
serialized by `eqiora.simplicial-mesh-envelope/v1`.

## Motivation

An external reader is useful only if it does not turn a third-party file
format into canonical model meaning. Gmsh supports physical groups,
partitioning, parametric and high-order nodes, arbitrary element families,
result sections, and evolving auxiliary sections. Pretending that a bounded
mesh reader supports all of those would make successful parsing
indistinguishable from semantic acceptance.

The boundary must also be safe for desktop Studio, Python, and service use.
Resource checks therefore precede declaration-controlled work and allocation,
and malformed input becomes one stable Eqiora diagnostic rather than crossing
the public API.

`Msh41Policy` defines independent semantic budgets for source bytes,
entities, entity references, blocks, nodes, elements, and ignored
lower-dimensional elements. It additionally defines aggregate decoded-byte and
decoded-work budgets. Every declaration-controlled loop is charged before its
allocation. Binary section and block declarations must also fit the minimum
number of bytes that their admitted record grammar could occupy in the
unconsumed input. ASCII declaration-derived cursors and expected field counts
use checked addition and bounded slice access throughout. ASCII tokens are
consumed by allocation-free iterators: format headers, fixed-arity records,
variable entity boundaries, and element connectivity never materialize a
token scratch vector.

The decoded-byte account conservatively includes adapter structural indexes,
tag sets and lookup maps, owned canonical vertex/cell vectors, and the maximum
unique simplex closure that `SimplicialMesh` can construct. The topology charge
uses the exact admitted closure upper bounds: three edges plus one cell for a
triangle, and six edges plus four faces plus one cell for a tetrahedron; shared
entities can only reduce materialization. Hash entries and topology-tree
entries receive explicit word-sized overhead charges. The decoded-work account
covers bounded decoding, tag lookup, canonical reconstruction, and topology
closure. These are deterministic conservative logical budgets, not
measurements or guarantees of an allocator's exact RSS. Source storage remains
independently bounded by `max_bytes`.

Defaults admit at most 16 MiB of source, 256 MiB of conservatively accounted
decoded state, 32 million work units, and 16,384 ignored lower-dimensional
elements. Callers may raise any explicit limit together for trusted workloads;
there is no hidden hard cap. Checked arithmetic closes count, width, cursor,
and accounting overflow. Every importer-owned declaration-sized collection
uses fallible reservation, so exhaustion and impossible capacity are reported
as `EQ0808`.

## Proposed design

```text
MSH bytes
  -> owned bounded MSH 4.1 decoder
  -> owned tag/coordinate/connectivity materialization
  -> SimplicialMesh::new
  -> SimplicialMeshEnvelopeV1
  -> content-addressed Realization
```

The typed `import_msh41(bytes, policy, assignment_sink)` boundary consumes an
`Msh41Policy` that owns the requested spatial dimension, an explicit `MeshQualityGate`, and
all resource bounds. It accepts bytes rather than paths; filesystem selection,
permissions, recent-file UX, and source provenance belong to callers such as
Studio. The ordinary policy returns only the existing `SimplicialMesh` owner.
The ASCII provider policy additionally emits complete source entity-tag
assignments expressed as canonical Mesh facet and cell indices only after the
whole import validates, without publishing parser blocks or importer state.
This keeps the same operation usable from Rust providers and tests without
adding competing mesh meaning.

The owned decoder admits exactly:

- MSH version 4.1, either ASCII with its 64-bit declaration or binary with a
  four- or eight-byte `size_t` declaration and a valid little- or big-endian
  marker; binary `int` and `double` retain their specified four- and
  eight-byte widths independently of `size_t`;
- one `$MeshFormat`, `$Nodes`, and `$Elements` section and at most one
  `$Entities` section;
- finite non-parametric nodes with positive, unique tags;
- arbitrary node and element block boundaries and sparse positive tags;
- linear-simplex blocks in every admitted entity dimension;
- lower-dimensional linear simplices, which are ignored after structural
  validation;
- `Tri3` as the only top-dimensional type for dimension two and `Tet4` for
  dimension three.

It rejects invalid endian markers or widths, parametric nodes, physical-group
membership, result or unknown sections, embedded two-dimensional surfaces,
high-order or non-simplex element blocks, missing node references, duplicate
tags, inconsistent section totals, and resource-limit excess. Cell order is
retained: the importer never silently repairs inverted orientation.
`SimplicialMesh::new` remains the single authority for duplicate, isolated,
non-manifold, orientation, and mean-ratio acceptance.

The adapter owns only this deliberately narrow grammar, not the complete MSH
format. ASCII and binary decoding materialize the same private coordinate and
connectivity representation, and no parser-specific type appears in public
signatures.

## Alternatives considered

### Implement the complete MSH grammar in Eqiora

Rejected. It would duplicate a broad evolving format while adding no strength
to Eqiora's mesh semantics. The owned decoder is intentionally small and
exists to bound resources and materialize the admitted subset, not to become a
second general-purpose reader.

### Bind the Gmsh SDK

Rejected for this slice. Native SDK state, deployment, platform ABI, and
geometry-kernel behavior are unnecessary when the accepted authority is a
fixed affine simplex mesh.

### Serialize paths and importer settings into the mesh artifact

Rejected. Paths are host-local and mutable; importer versions describe a
transformation, not accepted mesh identity. Optional source provenance may be
a separate artifact later.

### Delegate admission to a general-purpose MSH parser

Rejected. Parsing is not capability admission. The owned decoder is smaller
than a second broad grammar, applies Eqiora's resource policy before every
declaration-controlled allocation, and prevents an upstream parser from
silently widening the accepted format.

## Compatibility and migration

This adds an optional facade feature and a new adapter crate. Existing model,
mesh, Realization, and run wire bytes are unchanged. The public Rust API is
provisional before 1.0; `eqiora.simplicial-mesh-envelope/v1` retains its
append-only meaning.

## Verification

- Import ASCII and binary fixtures emitted from one source by the current
  stable Gmsh release, prove that both produce identical canonical mesh bytes
  and the same fixed artifact digest, bind that digest into a Realization, and
  solve the canonical two-dimensional one-DOF Poisson oracle.
- Exercise four- and eight-byte `size_t` values and both endian orders with a
  host-independent test encoder, and reject every truncated prefix of all four
  generated representations.
- Import through the feature-gated public `eqiora::io::gmsh` facade and reject
  every truncated prefix of the official Gmsh 4.15.2 binary fixture there.
- Admit exact inclusive count limits, and prove that forged declarations under
  `usize::MAX` public limits return `EQ0808` without panicking or attempting a
  declaration-sized allocation.
- Reject `usize::MAX` ASCII entity-reference, entity, node, and element
  declarations without a panic even when every public limit is raised to
  `usize::MAX`.
- Reject aggregate decoded-byte/work exhaustion before materialization, and
  reject a compact valid tetrahedral mesh padded with one more ignored point
  element than the default independent ignored-element budget.
- Exercise sparse tags, multiple blocks, and ignored boundary elements.
- Structurally accept one positively oriented tetrahedron.
- Reject every admitted-representation and official-fixture truncated binary
  prefix, invalid endian/data-size headers,
  excessive bytes or counts, duplicate tags, missing references, parametric
  nodes, result sections, embedded surfaces, unsupported elements, inverted
  cells, and quality-gate failure.

## Nonclaims

This RFC does not claim MSH 2.2/4.0, physical-name or result-field semantics,
partitioned meshes, periodic links, high-order or curved geometry, mixed
top-dimensional cells, embedded manifolds, global non-overlap, adaptivity,
source provenance, exact allocator RSS accounting, or round-trip export.
Supporting any of those requires a typed consumer and new verification
evidence rather than a wider parser flag.
