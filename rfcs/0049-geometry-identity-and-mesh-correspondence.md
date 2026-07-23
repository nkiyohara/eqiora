# RFC 0049: Geometry identity and mesh correspondence

- Status: Accepted; bounded implementation verified in
  [`geometry.fixed-reference-interface-identity-2d`](../verify/geometry/fixed-reference-interface-identity-2d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0006](0006-spatial-realization-contracts.md),
  [RFC 0035](0035-field-valued-boundary-interfaces.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md), and
  [RFC 0048](0048-dynamic-linear-solid-semantics.md)

## Summary

Eqiora identifies geometry through the exact semantic `Domain`, not through a
CAD-kernel face number or a mesh tag. A content-bound correspondence artifact
proves one revision-local chain:

```text
exact Model Domain
  -> exact entity in one geometry revision
  -> exact entities in one mesh revision.
```

Volume Domains map to geometry bodies and mesh cell sets. Boundary Domains map
to geometry boundary entities and mesh facet sets. The artifact chain binds
exact Model, geometry, correspondence, and mesh revision digests and derives
boundary orientation from the exact parent-side role and mesh incidence. A
caller cannot supply a normal sign.

This is the narrow Geometry Identity seam required before fixed-reference
fluid--structure interaction. It is neither a CAD kernel nor a transfer map.

## Identity boundary

Three identities remain distinct:

- a semantic Domain ID names physical intent and belongs to Model meaning;
- a geometry entity ID is local to one exact geometry revision; and
- a mesh entity ID is local to one exact mesh revision.

Coincident point sets do not merge semantic Domains. In particular, the fluid
and solid sides of an interface remain two Boundary Domains with distinct
parents. Their ordinary conserving Connection supplies physical interface
meaning; the correspondence artifact proves where both sides occur in the
chosen geometry and mesh revisions.

Adapter-local names, physical-group tags, declaration order, and array order
are not semantic identity. They may be recorded as provenance, but cannot be
used in place of an exact Domain reference.

## Version-neutral Model replay

Geometry Identity consumes the sealed replayable Model boundary from RFC
0037, not a concrete latest-generation envelope. Model v1, v2, v3, and v4
therefore enter the same geometry and correspondence code after their own
explicit decoder and whole-model validator have succeeded.

The replay result carries exact wire identity and validated meaning together.
Equal Domains encoded in two Model wire domains derive equal geometry roles
but remain different exact Model artifacts; a geometry revision sealed to one
rejects substitution by the other. An identity-only reference cannot enter
geometry because it does not prove possession of canonical content.

## Revision-local correspondence

An accepted artifact is closed over one exact Model artifact, one exact
geometry revision, and one exact affine-simplex mesh artifact. It records the
producer identity and version, coherent length unit, finite positive geometric
tolerance, and content digest for every bound resource.

For every admitted body it proves:

1. the semantic body is an exact Cartesian volume Domain;
2. the geometry entity is an exact body in the bound geometry revision;
3. the mesh cell set is nonempty, in range, and belongs only to that body;
4. every mapped Boundary has the unique semantic body as parent;
5. every boundary facet is in range and has exactly one adjacent cell within
   that parent's cell subset; and
6. the body's relative boundary-facet inventory is complete and has neither
   duplicate nor unowned facets.

The outward orientation is the incidence orientation from that unique
parent-side cell toward the complement of the parent's cell subset. It is
therefore `OutwardFrom(parent)`, even when the same mesh facet is also the
outward boundary of another cell subset. Reversing local facet vertex order
does not reverse this meaning when the oriented incidence remains valid.

The artifact canonicalizes selected Domain input order, Domain roles, and
derived entity memberships. Reordering selected body IDs or explicit
revision-association candidates cannot change canonical bytes when the exact
referenced artifacts and their entity identities are unchanged. Source or
Connection edits select a new exact Model artifact and are not erased by this
contract. Renumbering a mesh changes its mesh digest and
requires a new correspondence artifact; identity is never inferred by
coordinates alone.

## Tolerance ownership

The finite positive `tolerance_m` belongs to the Semantic Cartesian geometry
producer. It is the coherent-SI precision with which that exact geometry
revision classifies points and facets against its bodies and boundaries. It
is serialized into Geometry Identity and therefore changes its digest.

Cartesian entity topology itself is derived from exact canonical bounds. The
tolerance governs membership classification only. Correspondence consumers
reuse the geometry-owned value and accept no second mesh-local tolerance, so
one correspondence cannot silently reinterpret the geometry. Mesh spacing,
quality thresholds, numerical-solver tolerances, CAD healing, and future
import uncertainty remain separately owned policies.

## Cross-revision retention

Retention across geometry revisions is a separate explicit proof. For the
complete semantic Domain inventory named by a correspondence, an accepted
successor relation must be total and one-to-one: every prior geometry entity
has exactly one successor and every successor has exactly one predecessor.
Source and target Domain identities are revision-local and need not have equal
ULIDs; the explicit association itself is the retention evidence. Boundary
pairs are derived from each retained parent pair and exact `(axis, side)` role.

`Missing`, `Split`, `Merged`, and `Ambiguous` are explicit outcomes, not cases
that a tolerance or name heuristic may repair. Any consumer requiring retained
selection identity rejects all four without emitting a successor
correspondence. Remeshing may change the number and numbering of mesh entities;
the new revision must prove its own complete geometry-to-mesh memberships.

This is deliberately weaker than universal persistent topology naming and
stronger than best-effort face matching.

## Fixed-reference interface witness

The registered slice partitions one conforming two-dimensional
affine-triangle mesh into disjoint fluid and solid cell subsets. The two
Cartesian bodies are adjacent along one complete side. Their distinct
interface Boundary Domains are joined by one ordinary conserving Connection
and map through one shared geometry boundary entity to the same complete
mesh-facet set.

Each interface facet has exactly one adjacent fluid cell and one adjacent
solid cell. Incidence therefore derives opposite parent-outward orientations.
The witness contains only exact semantic IDs, exact revision digests, complete
entity memberships, and derived orientation. It constructs no trace quotient,
interpolation, mortar space, or coupling operator.

## Falsifying verification

The registered
[`geometry.fixed-reference-interface-identity-2d`](../verify/geometry/fixed-reference-interface-identity-2d/README.md)
case must prove:

- the two body cell sets are nonempty, disjoint, and complete for the bound
  mesh;
- each body's exterior and interface facets form its exact relative boundary;
- the two interface Boundary Domains retain distinct identities and parents
  while mapping to the same complete facet set;
- every interface facet has one adjacent cell in each body subset and derives
  opposite outward orientations;
- selected-body input and association-candidate order cannot change canonical
  correspondence bytes;
- exact Model, geometry, and mesh digests are required;
- explicit Model v1--v5 replay preserves decoded geometry meaning while exact
  wire-domain identity remains distinct;
- changing geometry-owned classification tolerance changes geometry identity,
  with a displaced interface accepted only by the revision whose declared
  precision admits it; and
- an explicitly total one-to-one geometry successor preserves the semantic
  selection while `Missing`, `Split`, `Merged`, and `Ambiguous` successors
  fail closed.

It must reject before accepted evidence:

- a stale or mismatched resource digest;
- an unknown, wrong-kind, wrong-dimension, or wrongly parented Domain;
- an out-of-range, duplicate, overlapping, missing, or surplus cell or facet;
- a facet that is interior to one parent subset or has invalid adjacency;
- unequal, partial, or noncoincident interface memberships;
- equal-facing rather than opposite parent-outward interface orientations;
- a shadow Boundary with the same geometry but a different semantic ID; and
- any incomplete, one-to-many, many-to-one, or ambiguous revision lineage.

## Alternatives considered

### Promote CAD entity IDs to semantic IDs

Rejected. Kernel entity numbering and topology can change after regeneration,
healing, or import. It cannot be the stable source of physical intent.

### Store a caller-authored normal sign

Rejected. A copied sign can disagree with the semantic parent or oriented mesh
incidence. Parent-outward orientation is derived from those authoritative
relations.

### Match revisions by names or geometric proximity

Rejected. Best-effort matching makes split, merge, and tolerance behavior
non-reproducible. Only explicit total one-to-one retention supports an exact
identity claim.

### Build the interface transfer now

Rejected. Trace quotient, interpolation, mortar, and ALE policies are
Realization choices. This RFC supplies only the identity and orientation proof
they may consume.

## Compatibility and security

The contract adds no Semantic Kernel node and changes no Domain, Boundary,
Connection, mesh, or package meaning. Existing Model, Geometry Identity, and
mesh wire bytes and digest preimages remain unchanged. The version-neutral
replay change widens only the typed Rust construction boundary before 1.0.
Correspondence decoding is closed, resource-bounded, and validates
all referenced identities and memberships before producing evidence. A failed
validation returns no partial correspondence or retained-selection claim.

## Nonclaims

This RFC does not implement or claim:

- STEP, BREP, NURBS, sketching, extrusion, boolean operations, healing, or a
  CAD kernel;
- Gmsh physical-group tags or any adapter tag as semantic identity;
- universal persistent topology naming or heuristic face matching;
- curved, high-order, nonmatching, embedded, adaptive, or three-dimensional
  correspondence;
- trace transfer, quotient degrees of freedom, mortar, Nitsche, or
  interpolation;
- moving geometry, remeshing policy, ALE, or geometric conservation;
- fluid, solid, or FSI assembly, time integration, solve, or solution
  evidence; or
- Studio selection and parametric-regeneration UX.
