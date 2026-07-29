# RFC 0080: Geometry-backed semantic admission

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-07-28
- Related RFCs and evidence:
  [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md),
  [RFC 0079](0079-authored-planar-geometry-artifact.md), and
  [`geometry.geometry-backed-semantic-admission`](../verify/geometry/geometry-backed-semantic-admission/README.md)

## Summary

One explicit `KernelProgram` entry path admits the exact closed bundle of
canonical geometry artifacts named by a selected Model. It derives
two-dimensional volume support for a valid geometry region and validates a
geometry boundary's named entity set relative to that region's own artifact.
The program retains only the derived support map, never artifact content or
borrowed references.

## Motivation

Model v7 preserves a geometry digest and entity-set name, while RFC 0079 proves
canonical geometry content and its identity. Neither contract alone proves
that the Model's digest has been supplied, that a named set exists, or that the
set has the dimension required by its semantic role.

Accepting a digest beside caller-supplied dimensions or set facts would create
a safe-Rust forgery path. Making `eqiora-sem` inspect the concrete planar
region would instead bind the semantic oracle to one geometry family. The
admission seam must establish the relationship without either failure mode.

## Proposed design

### Kind-erased geometry facts

`eqiora-geometry::CanonicalGeometryRef<'a>` is a borrowed opaque value that can
only be constructed from a canonical geometry owned by the geometry crate. It
projects:

- the derived digest bytes;
- ambient and topological dimensions; and
- an exact entity-set name's dimension, when present.

It exposes no coordinates, topology indices, canonical bytes, concrete
geometry kind, or public constructor from independent facts. Its private kind
may add exact-circle, Cartesian-box, curve, or 3D variants without changing
`eqiora-sem`.

### Exact closed bundle

`KernelProgram::from_snapshot_with_geometry` first collects every distinct
digest named by every `GeometryRegion`, including declarations with no current
consumer. Supplied references are indexed by their derived digest. A missing,
unreferenced, or duplicate digest is `EQ0901`; artifact order cannot affect
admission or diagnostic order. Bundle closure is over artifacts, not over
every entity set inside one artifact.

Bundle faults stop entity-set and geometry-consumer validation because no
coherent fact source exists. Existing snapshot, closed-topology, and Domain
topology diagnostics precede bundle diagnostics. Missing-artifact diagnostics
identify the smallest-ID referencing region; extra and duplicate diagnostics
identify the digest.

### Entity-set admission and support

This first family requires
`ambient_dimension == topological_dimension >= 1`; embedded manifolds are not
silently promoted to volumes. A `GeometryRegion` must select a set whose
dimension equals the artifact topological dimension.

A `GeometryBoundary` follows its one already-validated `BoundaryOf` parent. It
uses only that parent's artifact and must select a set of dimension
`topological_dimension - 1`. Searching the whole bundle by name is forbidden:
equal names in two artifacts do not share facts. A Domain with an existing
topology fault is skipped and derives no support.

Successful admission adds internally derived
`SpatialSupport::Volume` and `SpatialSupport::Boundary` entries to one
`BTreeMap` keyed by exact Domain identity. Field validation, Relation typing,
and later typed-residual reconstruction all consume that same map. A
`SpatialCartesian` Field extent must equal the admitted ambient dimension.
The program retains neither geometry bytes nor the borrowed reference.

A boundary-physical Port on a geometry boundary remains rejected. Its public
contract requires a Cartesian normal axis, coordinate, and tangential
intervals, none of which follows from an entity-set dimension. The exact
diagnostic names the missing non-Cartesian boundary-embedding contract.

The existing artifact-free `from_snapshot` remains source-compatible. It
continues to admit geometry declarations for replay while rejecting Field,
Relation, and boundary-physical Port consumers with `requires artifact
admission`.

## Dependency decision

`eqiora-sem -> eqiora-geometry` is one explicit same-L2 dependency exception.
The semantic oracle must derive support from the non-forgeable canonical owner;
geometry remains kernel-neutral and has no reverse dependency. This transitively
reaches `eqiora-meshing` through RFC 0049, but no mesh type or direct meshing
dependency enters `eqiora-sem`.

The geometry public-surface ceiling rises from 32 to 33 for the single opaque
borrowed reference. The deletion condition is to lower it if the canonical
geometry value itself becomes the kind-erased stable fact projection. No
public trait, provider, registry, owned geometry enum, or semantic
geometry-kind switch is introduced.

## Compatibility

Existing Model and Transaction bytes, artifact digests, semantic fingerprints,
and artifact-free entry-point behavior are unchanged. An admitted
declaration-only program differs from its artifact-free program because only
the former retains derived support facts, but both produce byte-identical
Model v7 artifacts and equal structural fingerprints.

`ModelEnvelopeV7::to_program` remains artifact-free and therefore does not
reconstruct the admitted support map. Artifact storage, discovery, and
artifact-aware application replay belong to a later application surface.

## Verification

The positive oracle reuses RFC 0079's independently frozen square-with-hole
artifact:

```text
sha256: e6f8e17ac215ef37ca3c9de07b9979e34f13412a5de11dc9240ea1def8130030
fluid: dimension 2
exterior, hole: dimension 1
```

One Model selects `fluid` and `hole`, types a two-component spatial Cartesian
Field and a divergence Relation, and reconstructs the typed residual after
construction without resupplying the artifact.

Falsifiers cover missing, extra, duplicate, and permuted bundles; declaration
closure; region and boundary dimension reversal; two region aliases sharing
one artifact; a name present only in a foreign artifact; caller-order-invariant
diagnostics; a geometry boundary Port; and Model/fingerprint stability.

## Nonclaims

This RFC adds no source or draft syntax, `box` synthesis, artifact storage or
discovery, package manifest, mesh admission, correspondence, Realization,
numerical lowering, boundary-to-parent geometric-incidence proof, curve,
Cartesian-box geometry artifact, embedded manifold, 3D, CAD, exact circle, or
cylinder-flow demonstration.
