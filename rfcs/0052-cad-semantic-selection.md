# RFC 0052: CAD realization and semantic selection

- Status: Implemented and verified for the bounded box/semantic-selection
  slice; [`geometry.cad-semantic-selection-box`](../verify/geometry/cad-semantic-selection-box/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md)
  and [RFC 0051](0051-durable-spatial-state-and-trajectory.md)

## Decision

The first CAD slice is one replaceable, compile-time geometry-kernel adapter
behind an Eqiora-owned closed contract. It consumes one complete explicitly
SI-unit-declared STEP stock, one fully constrained axis-aligned XY rectangle, one positive-z
extrusion, and one boolean intersection. The accepted output is exactly one
axis-aligned three-dimensional box.

That box must equal an existing Cartesian body `Domain` in the exact Model.
The existing Geometry Identity, mesh, and geometry--mesh correspondence
artifacts remain the authoritative path to physics:

```text
STEP source + closed CAD design
  -> isolated CAD-kernel adapter
  -> normalized box observation
  -> exact Model Cartesian Domain
  -> Geometry Identity
  -> Mesh revision and correspondence
  -> physical boundary Relation / Port
```

CAD is therefore a Geometry Realization and editor input, not new Semantic
Kernel meaning. No CAD entity, feature-history object, source entity rank,
renderer primitive, or kernel face number becomes a Model node or identity.

## Why this narrow shape

The purpose of the first slice is to falsify the architecture, not to present
a broad CAD product. An imported stock intersected with a constrained sketch
extrusion exercises STEP/B-rep input, sketching, extrusion, boolean execution,
regeneration, visible selection, and boundary replay while still ending in a
shape the accepted Geometry Identity contract can classify exactly.

General B-rep canonicalization at this point would either duplicate a CAD
kernel or create an untyped geometry payload. Both would weaken the existing
meaning/realization boundary. A later curved or multi-body case must introduce
only the additional typed observation vocabulary its evidence needs.

## Closed contracts

`CadBoxDesignV1` contains the target Semantic body, raw STEP SHA-256, exact
metre or millimetre STEP unit, expected stock bounds, the fully constrained
rectangle, extrusion depth, STEP source uncertainty, and CAD modeling
tolerance. Coordinates are converted explicitly to coherent-SI metres. Its
mathematical output is the exact positive-volume intersection of the stock and
extruded rectangle.

`CadBoxRealizationV1` contains three kernel-independent observations: imported
stock, extruded tool, and boolean result. Each observation must be exactly one
solid, one closed shell, six planar axis-aligned faces, and no repair. It has
no field capable of carrying a kernel object or face rank.

`CadKernelAdapter` is a compile-time seam, not a runtime plugin ABI. It accepts
complete source bytes and the closed design, returns only Eqiora-owned
observations, and exposes exact adapter and kernel versions for evidence. The
first implementation is isolated from the pure contract. Replacing it must
not alter Semantic Model, Geometry Identity, mesh, or selection contracts.

## Identity and provenance

Five identities remain distinct:

- the exact Semantic `Domain` is physical intent;
- the exact Geometry Identity digest scopes revision-local geometry entities;
- the raw STEP SHA-256 identifies the complete imported byte stream;
- the CAD design digest identifies exact modeling intent and policy; and
- the CAD build-evidence digest identifies one adapter/kernel replay and its
  normalized output.

Two CAD sources may produce the same Geometry Identity when their exact Model
and normalized Cartesian geometry are equal. Their source, design, and build
lineage remains different. This is intentional: geometry meaning is not its
producer history.

Selection requests carry an exact Geometry Identity digest plus an exact
Semantic Domain. The application resolves that Domain to the revision-local
`GeometryEntity`; a viewport and semantic table submit the same request.
Kernel face order, STEP entity rank, display name, mesh facet order, and SVG
primitive identity cannot select meaning.

## Tolerance ownership

The following values are separate and must never be silently substituted:

- STEP source uncertainty;
- CAD modeling and boolean tolerance;
- Geometry Identity classification tolerance;
- mesh quality threshold; and
- renderer tessellation deflection.

Only the first two belong to the CAD design/build lineage. Geometry Identity
already owns the third. Meshing and the Studio renderer own the remaining
policies. V1 performs no healing; an input or boolean result requiring repair
is rejected instead of being accepted under a modeling tolerance.

## Cross-revision association

V1 accepts two independently compiled and realized exact CAD plans and closes
an explicit total one-to-one association between their Model-bound Geometry
Identity, mesh, and correspondence revisions. Selection retention first
proves that the selection belongs to the exact source plan, then resolves only
the associated target Domain against the exact target plan.

Authoring a dimension edit as a typed Model transaction, previewing it as one
content-bound mutation plan, and committing that exact plan key remain future
editor work. The current Studio does not own or mutate the accepted Model,
design, or geometry, and does not author the retention association. Its
application state may hold one exact selection against the active plan.

Missing, split, merged, and ambiguous retention are separate typed outcomes.
V1 reuses `GeometryRevisionAssociationEnvelopeV1`, because its intended
vertical slice already regenerates the mesh and proves complete replay to the
physics boundary. A geometry-only retention artifact may be introduced only
when a later evidence case genuinely needs selection before meshing.

## Studio projection

The Geometry workspace renders a bounded tessellation tagged with exact
Geometry Identity and Semantic Domain references. Renderer state consists only
of camera, hover, and presentation preferences. Accepted selection lives in
the application state and can be changed through either the viewport or a
keyboard-accessible semantic entity table.

The inspector presents the Domain and parent, parent-outward role, mesh
membership, and attached physical boundary meaning. Native resolution remains
bound to a revision-local geometry entity, but V1 does not expose that local
entity rank as Studio selection identity. The semantic table does not depend
on a renderer primitive or WebGL picking path; full renderer-failure isolation
is not claimed. An event carrying a stale Geometry Identity is rejected before
state change.

## Falsifying evidence

Two registered cases close different parts of this decision. The new
[`geometry.cad-semantic-selection-box`](../verify/geometry/cad-semantic-selection-box/README.md)
case accepts one complete STEP-stock/intersection design through the ordinary
adapter, exact Model-bound design/build artifacts, Geometry Identity, a
six-tetrahedron correspondence, and the application projection. It proves
complete-source and adapter-identity drift, source-unit mismatch, distinct CAD
policy drift, open/multiple/non-planar/non-axis-aligned STEP topology, stale
selection, and selection from a foreign regeneration revision fail closed. It
also proves the viewport and table projection create the same
`(Geometry Identity, Domain)` request and resolve the same Relations, Ports,
and mesh membership.

The earlier
[`geometry.fixed-reference-interface-identity-2d`](../verify/geometry/fixed-reference-interface-identity-2d/README.md)
case remains the registered falsifier for missing, split, merged, and ambiguous
cross-revision associations and for stale Model/geometry/mesh/correspondence
resources. The CAD case reuses that unchanged artifact; it does not silently
claim independent topology-matching semantics.

Repair disposition and kernel face rank are type-level exclusions. V1 exposes
only `CadRepairDispositionV1::None`, and neither the public artifact/API wire
nor the Studio CAD sub-protocol has a field that can carry a kernel face
number. Ordering of renderer primitives therefore cannot become selection
identity: the only accepted selection request contains the exact Geometry
Identity digest and Semantic Domain. A pre-regeneration request is rejected by
the successor revision; retention is possible only through the explicit
accepted one-to-one association.

Studio's native bridge unit tests and local TypeScript/Playwright tests verify
the bounded accessible viewport/table interaction and stale-response reducer.
They are local Studio validation, not an additional Cargo evidence target in
the root verification registry. The registered Rust case owns the canonical
CAD, geometry, mesh, and projection claims above.

## Prior art and adapter choice

Open CASCADE provides broad STEP and boolean support, but its C++ runtime and
deployment boundary are larger than this first Rust slice. Truck separates
Rust-native topology/modeling, STEP I/O, and shape operations under
Apache-2.0. The admitted adapter pins `truck-stepio 0.3.0`,
`truck-modeling 0.6.0`, and `truck-topology 0.6.0`; these exact crates pass the
Rust 1.89 Linux gate. `truck-shapeops` is excluded because its current default
graph reaches known-vulnerable legacy compression and XML crates. The bounded
boolean is instead exact interval intersection over the closed AABB contract,
followed by reconstruction and validation as a six-plane Truck B-rep. This is
not a broad Truck boolean claim.

The slim Truck graph still contains unmaintained `cgmath 0.18.0` and
`proc-macro-error 1.0.4`, both without reported vulnerabilities or a safe
upstream upgrade. Their two exact advisories are documented and confined to
the optional L3 adapter; widening the adapter requires a fresh dependency and
platform audit. The choice remains outside Model meaning.

Modelica-style annotations, source names, and expandable connector behavior
are not used as geometry identity. CAD realization also does not alter the
typed physical Port and conserving-Connection contracts.

## Compatibility

This RFC adds no Semantic Kernel node, Domain kind, Model wire generation, or
change to Geometry Identity V1. Existing artifacts retain their exact bytes
and meaning. New CAD artifacts use new schema IDs and reject unknown fields or
versions. The adapter is optional at the public facade so the dependency-free
semantic and geometry core remains usable without a CAD kernel.

## Nonclaims

This slice does not claim general STEP support, universal persistent naming,
curved/NURBS Geometry Identity, a sketch constraint solver, arbitrary feature
history, union/subtraction suites, fillets, assemblies, production healing,
CAD-owned meshing, dimension-edit transaction/commit, renderer-failure
isolation, dynamic plugins, ALE, remeshing, shape optimization, or
distributed/GPU CAD.
