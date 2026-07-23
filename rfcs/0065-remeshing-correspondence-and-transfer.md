# RFC 0065: Remeshing correspondence and conservative FSI transfer

- Status: Implemented and verified for the bounded serial-host 2D slice
- Authors: Eqiora contributors
- Created: 2026-07-21
- Evidence: [`fsi.remeshing-transfer-2d`](../verify/fsi/remeshing-transfer-2d/README.md)
  (`verified`)
- Depends on: [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md),
  [RFC 0050](0050-fixed-reference-monolithic-fsi.md), [RFC
  0051](0051-durable-spatial-state-and-trajectory.md), and [RFC
  0064](0064-fixed-topology-ale-fsi.md)

## Summary

Remeshing is a zero-model-time re-realization transition between two exact
mesh revisions. It is not a Semantic Model edit, a time step, or an implicit
array-index correspondence.

The first accepted slice keeps the exact Model and physical Domain identities,
proves one-to-one semantic geometry retention, derives a many-to-many common
refinement between the old and new affine-triangle meshes, transfers the
bounded ALE FSI state through field-aware variational projections, and invokes
the unchanged target ALE finalizer only after the transferred state is
accepted.

```text
same canonical Model and physical Domains
  -> exact source/target Geometry Identity and mesh correspondences
  -> one-to-one semantic revision association
  -> many-to-many material/current common refinements
  -> field-aware constrained projection
  -> accepted target GeometryState / SpatialState
  -> remeshing-aware trajectory transition
  -> ordinary target fixed-topology ALE step
```

Semantic boundary split, merge, loss, or ambiguity is rejected. Mesh facets
may split or merge as part of the common refinement; confusing those two
notions would make legitimate remeshing impossible.

## Decision boundary

### Meaning stays unchanged

No remesh, mesh velocity, transfer, source-cell, target-cell, or interpolation
node is added to canonical meaning. The physical Relations, Fields, Ports,
Connections, supports, units, frames, and model time are unchanged across the
transition.

The first slice admits only an exact replay of the same canonical Model and
the same physical Domain inventory. Source and target mesh-bound Realizations
are distinct because their mesh artifact references differ. Numerical method,
material, time-step, solver, boundary, field-space, and scale choices must
otherwise be equal. A simultaneous solver or physics change is not a remesh
transition.

### Three relations remain distinct

1. `GeometryRevisionAssociationEnvelopeV1` proves total one-to-one retention
   of semantic bodies and boundaries. Its `Missing`, `Split`, `Merged`, and
   `Ambiguous` outcomes remain typed failures.
2. `GeometryMeshCorrespondenceEnvelopeV1` independently proves the complete
   body-cell and boundary-facet memberships of each exact mesh revision.
3. `SimplicialRevisionOverlap2d` proves geometric intersection and complete
   coverage between source and target mesh entities in one coordinate chart.

The overlap is not an index map. One source cell may intersect many target
cells and conversely. Equal local indices, equal counts, or coincident
centroids provide no identity evidence.

## Coordinate charts and transition order

The ALE state mixes two mathematically different charts and must transfer them
in dependency order.

1. Absolute solid displacement is a material/reference-chart quantity. It is
   projected on the source and target reference solid meshes.
2. The target harmonic mesh-motion Realization consumes only that accepted
   target displacement trace and derives the complete target current geometry.
3. Source and target current meshes then define the spatial common refinement
   used for fluid velocity and pressure. Solid velocity remains integrated in
   its reference material chart.

Applying the Cartesian reference correspondence directly to deformed current
coordinates is invalid. Resetting solid displacement to zero and rebasing the
current geometry would also change the absolute small-strain state and is not
admitted by this RFC.

The pre- and post-remesh states have the same exact `(step, time)`. A remesh has
no duration, creates no mesh velocity, and contributes no ALE geometric-rate
term. The next ordinary time step derives mesh velocity only within the target
fixed topology.

## Common-refinement contract

`eqiora-meshing` owns a two-dimensional affine-simplex overlap relation. It
accepts two already validated meshes and produces canonical positive-measure
intersection fragments.

Each volume fragment records, in canonical `(source cell, target cell,
fragment)` order:

- source and target cell identities;
- one positively oriented convex intersection polygon, canonically
  triangulated for integration;
- area and the coordinate moments needed to replay coverage; and
- the exact coordinate chart (`material` or `current-spatial`) in which it was
  derived.

For every retained semantic boundary, the corresponding facet relation records
positive-length source/target segment intersections and both parent-outward
orientations. A semantic interface remains one retained pair even when its
mesh facets form a one-to-many or many-to-many relation.

Admission requires:

- positive orientation and finite coordinates for both meshes;
- adaptive-precision orientation predicates for topology decisions;
- deterministic fragment construction and ordering;
- complete source and target cell coverage in both directions;
- complete retained boundary-facet coverage in both directions;
- no positive-measure overlap between different retained body Domains;
- no unexplained positive area, length, duplicate fragment, or uncovered
  entity; and
- explicit ambiguity rejection for degenerate or numerically uncertifiable
  intersections.

Topology is decided with robust predicates, while constructed coordinates use
a separate exact-rounding contract. Every finite binary64 input coordinate is
lifted to its exact rational value; a proper crossing is constructed in exact
rational arithmetic and rounded once to a canonical binary64 point. A retained
facet that was produced by an earlier rounded geometry action is admitted as
the same line only when each endpoint's binary64 rounding cell intersects both
exact rational lines. Source/target exchange and edge reversal must produce
bit-identical fragments. This is a proof about representable rounding cells,
not a tunable geometric tolerance: a perturbation outside the certified cells
is rejected.

Exact crossing coordinates are retained through hull construction. Distinct
exact crossings are never merged merely because their rounded coordinates are
adjacent or their rounding cells touch; if the final binary64 fragment has no
positive finite measure after canonical nearest rounding, construction fails
closed. Sub-binary64 microgeometry and exact-rational fragment artifacts are
not claimed by v1; adding them requires a new wire and numerical-conversion
contract rather than directed rounding that invents area locally. A positive
overlap fragment may nevertheless be arbitrarily slender: it is an integration
region, not a replacement mesh cell. Numerical consumers therefore use its
admitted robust measure and a forward-only affine quadrature map. Mesh-cell
inverse-map rank and quality gates remain confined to the validated source and
target parent cells and are not misapplied to common-refinement fragments.

The first reference implementation may use a quadratic broad phase. That is a
performance nonclaim, not permission to weaken coverage. Search acceleration
is private mechanism and cannot change accepted overlap bytes.

## Variational transfer

The common refinement supplies geometry, not a universal array operation. A
Field's realized space and physical role select its transfer law.

For source basis `phi^-`, target basis `phi^+`, and the field's integration
measure `omega`, the projection assembles

```text
M+_ab = integral omega phi+_a phi+_b
C_ai  = integral omega phi+_a phi-_i
```

over the canonical overlap triangulation. The target coefficients minimize the
weighted L2 difference. When obligations are present, the accepted solve is
the implicit relation

```text
[ M+  A^T ] [ u+     ] = [ C u- ]
[ A    0  ] [ lambda ]   [ c-   ]
```

The existing common solver plan, operator properties, true-residual
acceptance, and report vocabulary are used directly. A remesh-specific solver
configuration is not introduced.

All projection algebra is dimensionless. The transfer plan retains the exact
typed characteristic length `L`, velocity `U`, and pressure `P` of both ALE
Realizations; admission rejects a source, target, or embedded-plan mismatch.
With `rho* = max(rho_fluid, rho_solid)`, assembly uses

```text
x_hat = x / L
d_hat = d / L
v_hat = v / U
p_hat = p / P
rho_hat = rho / rho*
```

before constructing mass, mixed-mass, trace, divergence, momentum, or KKT
rows. Solver residuals and right-hand-side norms therefore describe the same
dimensionless algebra. Physical replay remains separate: raw L2 errors and
raw momentum/pressure functionals may be retained for interpretation, while
accepted defects are normalized by `L`, `U`, `U L`, `rho* U L^2`, or `P L^2`
as appropriate. A raw dimensional value is never compared directly with a
dimensionless solver tolerance.

### Absolute solid displacement

Solid displacement uses the reference solid measure. The target interface and
physical-boundary trace is replayed from the retained semantic boundary
relation; free target solid coefficients use L2 projection. The target current
geometry is then reconstructed solely by the existing harmonic mesh-motion
action.

The accepted obligations are trace continuity, finite projection error,
positive target quality, and exact harmonic replay. Displacement integral is
not called a conserved physical quantity.

### Shared fluid/solid velocity

Velocity is transferred as one coupled field, not as independent fluid and
solid arrays. The target space retains one shared P1 trace, fluid MINI bubbles,
and solid P1 coefficients. Fluid terms use the current spatial measure and
fluid density; solid terms use the reference material measure and solid
density.

The first slice constrains:

- the retained shared interface trace;
- the complete homogeneous physical-velocity exterior;
- target weak incompressibility, with a canonical independent constraint set
  and a complete residual replay; and
- density-weighted total momentum in each spatial component.

Separate fluid and solid momentum may be recorded but is not required by the
first claim. Interface transfer is a representation change and must not create
an impulse.

The MINI cell block is a coefficient of a normalized cubic bubble basis. It is
never interpreted as a cell average or transferred by P0 overlap weights.
Mixed source/target basis products are integrated with a rule exact through at
least total degree six; the existing degree-eight triangle rule is sufficient.

### Fluid pressure

Pressure uses an absolute P1 L2 projection on the current fluid domain. The
current FSI operator closes its constant-pressure action through the coupled
boundary problem; the transfer must not silently subtract a mean or invent a
gauge. Zeroth-moment reproduction follows from the admitted target constant
test function and is recorded as projection evidence, not as a physical
conservation law.

### Excluded field classes

No positivity, maximum-principle, monotonicity, boundedness, entropy, or
kinetic-energy preservation is implied. A density, mass fraction, volume
fraction, history variable, or other field with those obligations requires a
separate typed policy and falsifier before admission.

## Realization and numerical lifecycle

The remesh plan binds:

- exact source and target Model, geometry, correspondence, mesh, and
  fixed-topology ALE Realization identities;
- the semantic revision association;
- source and target Field-space identities;
- material and current overlap identities;
- quadrature exactness and constraint inventory;
- the exact common typed `L`, `U`, and `P` normalization profile;
- the common linear-solver plan and tolerances; and
- accepted projection, coverage, trace, momentum, incompressibility, quality,
  and error evidence.

The target output is exposed as `AleFsiInitialPhysicalState2d` only after all
obligations pass. The existing target
`finalize_resolved_fixed_topology_ale_fsi_2d` then independently derives the
partition, harmonic action, geometry, boundary closure, and finalized
operator. One ordinary target ALE step must succeed through that unchanged
path. A transfer-specific time integrator or physics lowerer is forbidden.

## Artifact DAG

Existing mesh, correspondence, FieldSnapshot, GeometryState v1, SpatialState
v2, and SpatialTrajectory v2 bytes remain unchanged.

The remeshing DAG is acyclic:

```text
source SpatialStateV2 / source trajectory root
  + target mesh/correspondence/Realization
  + semantic revision association
    -> target solid-displacement snapshot
    -> target GeometryStateV2 (remesh origin, same step/time)
    -> MeshRevisionOverlapV1 (material + current relations)
    -> target velocity/pressure snapshots + transfer receipt
    -> SpatialStateV3
    -> SpatialTrajectorySegmentV3
    -> SpatialTrajectoryV3 immutable root
```

`GeometryStateEnvelopeV2` has a closed origin:

- `continuous`: a predecessor on the same mesh revision, with the usual
  positive-duration fixed-topology action; or
- `remesh`: an exact source GeometryState/SpatialState, exact target mesh and
  displacement snapshot, identical accepted step/time, and no mesh velocity.

It never references the later transfer receipt or trajectory segment.

`MeshRevisionOverlapEnvelopeV1` binds both reference mesh revisions, both
geometry-to-mesh correspondences, the semantic revision association, source
and target geometry states, and the canonical material/current overlap
outcomes.

`SpatialStateEnvelopeV3` binds the exact target context, GeometryState v2,
complete target Field snapshots, source state, overlap, and per-Field transfer
receipt. The receipt identifies the transfer law, integration chart, source
and target snapshots, solver/operator identity where applicable, conserved
functional, constraint residuals, and projection errors.

The first `SpatialTrajectorySegmentEnvelopeV3` begins with a same-time remesh
pair. Later states on the target mesh advance normally. The v3 root retains an
exact v2 source prefix plus immutable v3 append prefixes; it never flattens
mesh-local arrays into a fictitious common index space.

Every new decoder is bounded. Unknown origins, transfer laws, chart kinds,
field roles, fragment shapes, or stale digests fail before state or trajectory
acceptance.

## First registered evidence

The reference case starts from a nonzero accepted state of the RFC 0064 model
and constructs a second conforming affine-triangle mesh with different vertex,
cell, and facet counts. At least one old interface facet is split into multiple
target facets and at least one volume overlap is genuinely many-to-many; pure
renumbering is insufficient.

The state includes nonconstant absolute pressure, nonzero MINI bubbles,
nonzero shared velocity, and nonzero absolute solid displacement. The case
must prove:

- exact source/target Model, Geometry Identity, correspondence, mesh, and
  Realization linkage;
- one-to-one semantic body/boundary/interface retention while mesh facets
  split;
- bidirectional material/current cell and retained-facet coverage;
- constant and affine reproduction by the mixed-mass action;
- a pure-bubble witness that fails if the cell block is treated as P0;
- density-weighted total-momentum conservation;
- target weak incompressibility and exact shared/exterior velocity traces;
- absolute-pressure L2 and zeroth-moment evidence without mean subtraction;
- target geometry derived only from transferred absolute displacement;
- positive target reference/current quality;
- independent replay of the same overlap connectivity and physical constraint
  roles/counts under a distinct characteristic scale profile, with each
  profile satisfying its dimensionless acceptance contracts and
  reconstructed physical Fields compared only against the case's explicit
  non-semantic observation bound (bit identity of iterative solver stopping
  points is not required);
- equal pre/post step and model time, immutable artifact replay, and exact
  source/target mesh identities in the v3 trajectory; and
- one accepted ordinary target ALE step after transfer acceptance.

## Falsifiers

The slice rejects:

- stale, swapped, or omitted source/target Model, geometry, correspondence,
  mesh, Realization, state, overlap, snapshot, or receipt identity;
- implicit same-index copy, equal-count inference, or reordered local entity
  substitution;
- one missing/duplicated overlap fragment, incomplete bidirectional coverage,
  unexplained overlap, or positive cross-material overlap;
- semantic `Missing`, `Split`, `Merged`, or `Ambiguous` interface retention;
- missing retained-facet coverage or wrong parent-outward orientation;
- point interpolation or P0 treatment substituted for the admitted Field
  projection, especially for a pure MINI bubble;
- momentum, weak-divergence, shared-trace, exterior, pressure-moment, or
  projection-residual failure;
- displacement reset, independently supplied target current coordinates, or
  failed harmonic replay;
- inverted, degenerate, low-quality, or non-finite target geometry;
- a remesh that advances/resets model time or supplies zero-duration mesh
  velocity;
- target continuation invoked before transferred-state acceptance; and
- a v3 state or trajectory missing either mesh revision, overlap, transfer
  evidence, source prefix, or immutable predecessor edge.

## Alternatives considered

### Nodal interpolation for every Field

Rejected. It is inexpensive but generally does not preserve integrals or weak
constraints, and it mistakes a MINI bubble coefficient for a point or cell
value.

### P0 cell-overlap transfer for every stored cell block

Rejected. Cell storage association does not define finite-element meaning.
P0 overlap is appropriate for true cell averages, not for the current MINI
basis coefficient.

### Plain component-wise L2 projection

Rejected as the accepted FSI handoff. It is an important building block but
does not by itself preserve the shared trace, weak incompressibility, and
momentum obligations of the coupled state.

### Admit only a nested refinement forest

Rejected as the owning contract. Parent-child refinement is a useful producer
and test oracle, but making ancestry authoritative would exclude independent
remeshers, edge flips, and unrelated numbering. The common refinement remains
the mathematical source of transfer weights.

### Rebase current geometry and reset displacement

Rejected. The current solid displacement is absolute semantic state and the
sole geometry driver. A finite-strain chart change or stress/history-variable
rebase requires a separate contract.

### Mutate SpatialState v2 with optional remesh fields

Rejected. V2 intentionally means one immutable reference mesh and one
GeometryState-v1 predecessor chain. A new explicit version preserves old
bytes, closes the origin grammar, and prevents half-populated states.

## Prior art

The design follows the separation between supermesh geometry and variational
projection established by Farrell and Maddison's
[local Galerkin projection](https://doi.org/10.1016/j.cma.2010.07.015) and
[supermesh construction](https://doi.org/10.1016/j.cma.2009.03.004). The
constrained formulation is informed by Maddison and Hiester's
[optimal constrained interpolation](https://doi.org/10.1137/15M102054X).
The implementation uses adaptive-precision orientation decisions following
[Shewchuk's robust predicates](https://people.eecs.berkeley.edu/~jrs/papers/robust-predicates.pdf).

These references justify the mathematical decomposition; they do not become
Eqiora artifact schemas, provider types, or capability claims.

## Compatibility and security

The contract adds no canonical node, Relation, activation, Port, Field, or
package meaning. Existing artifact versions and digest preimages remain
unchanged. New overlap/state/trajectory decoders reject unknown fields and
enforce byte, fragment, entity, Field, state, and aggregate-work limits before
allocation or replay.

Adaptive predicates decide topology, but accepted construction still requires
finite canonical coordinates, positive fragments, complete coverage, and
independent moment replay. A robust sign does not excuse an uncertifiable
intersection coordinate. Uncertain or near-degenerate construction fails
closed rather than healing the mesh.

## Nonclaims

This RFC does not claim a production remesher, mesh-generation policy, AMR,
coarsening strategy, CAD regeneration/healing, curved or high-order geometry,
three-dimensional overlap, contact, nonmatching physical interface coupling,
mortar/Nitsche realization, positivity or monotonicity preservation,
history-variable transfer, turbulence-state transfer, distributed/GPU
remeshing, repartitioning, performance, scale, checkpoint/restart, fault
recovery, remesh sensitivity, ALE sensitivity, FSI adjoints, or shape
optimization.
