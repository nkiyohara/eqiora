# RFC 0064: Fixed-topology ALE fluid--structure interaction

- Status: Implemented and verified for the bounded serial-host 2D slice
- Authors: Eqiora contributors
- Created: 2026-07-21
- Evidence: [`fsi.fixed-topology-ale-monolithic-2d`](../verify/fsi/fixed-topology-ale-monolithic-2d/README.md)
  (`verified`)
- Depends on: [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md),
  [RFC 0050](0050-fixed-reference-monolithic-fsi.md), [RFC
  0051](0051-durable-spatial-state-and-trajectory.md), [RFC
  0053](0053-discrete-block-system.md), and [RFC
  0058](0058-portable-realization-and-execution-graphs.md)

## Summary

Eqiora's first moving-domain slice composes the existing conservative
transient incompressible Navier--Stokes Relations, dynamic linear-solid
Relations, and ordinary conserving mechanical Connection through one
fixed-topology arbitrary Lagrangian--Eulerian Realization. It adds no mesh
velocity Field, ALE Relation, FSI node, or configuration switch to Semantic
Model meaning.

The central contract is:

```text
immutable reference topology and memberships
  + consecutive accepted physical states
  + one mesh-motion Realization
  -> one sealed Geometry Action
       {current maps, mesh velocity, velocity gradient, GCL correction}
  -> one monolithic moving-domain residual and analytic linearization
  -> accepted physical state plus replayable geometry-state lineage
```

Coordinates, mesh velocity, swept geometric flux, and the geometric
conservation contribution are not independent inputs. They are projections of
one action. This prevents a caller from combining a valid moving mesh with an
unrelated velocity or adding a compensating GCL correction to conceal drift.

## Semantic boundary

The admitted fluid meaning is RFC 0053's conservative transient
Navier--Stokes subset:

```text
rho_f * derivative(fluid_velocity)
  + div(rho_f * outer_product(fluid_velocity, fluid_velocity))
  - div(fluid_stress)
  - grad(fluid_load_potential) = 0
div(fluid_velocity) = 0
```

The solid and mechanical boundary meaning are unchanged from RFCs 0048 and
0050. A distinct ALE canonical projection reuses the Navier--Stokes subdomain
recognizer and the common FSI Connection closure. It does not teach the
fixed-reference projection to accept optional advection or optional geometry.

`derivative` and conservative spatial flux remain physical meaning. Pullback,
mesh extension, path integration, time stepping, nonlinear solution, and
configuration selection are Realization decisions. The current bounded
Realization uses a reference configuration for the small-strain solid and a
current ALE geometry for the fluid. That distinction is explicit in the
portable graph; it is never inferred from which assembly function is called.

## Reference topology and geometry state

The existing `SimplicialMeshEnvelopeV1` remains an immutable reference
artifact. Its ordered connectivity, reference coordinates, quality policy,
and digest anchor:

- Domain and Boundary memberships from RFC 0049;
- Field coefficient ordering and support closure;
- the conforming interface trace quotient; and
- all future geometry-state coordinates.

An ALE step does not create another mesh revision and does not rerun Cartesian
geometry classification. A new fixed-topology geometry state contains no
connectivity. It binds the exact reference Model, Geometry Identity,
correspondence, mesh, ALE Realization, accepted step and time, predecessor
geometry-state digest, absolute current coordinates, current solid-
displacement snapshot, and recomputed quality evidence.

Current coordinates are reconstructed from reference coordinates plus the
absolute accepted solid displacement and its fluid-domain harmonic extension.
They are not formed by repeatedly accumulating coordinate increments. This
keeps replay independent of accumulated floating-point path drift. Consecutive
accepted states define a linear-in-time geometric path; mesh velocity is the
backward difference of those two states and cannot be supplied separately.

The state constructor rebuilds every affine simplex with the reference cell
order and original quality gate. It rejects non-finite coordinates,
non-positive orientation, insufficient mean-ratio quality, a changed vertex
inventory, and any topology-bearing payload. It additionally checks the
whole linear path for positive orientation. In two dimensions the signed
Jacobian is quadratic along an affine vertex path, so endpoint-only checks are
not treated as a path proof.

## Mesh-motion Realization

The first method is `P1HarmonicExtension`:

- solid coordinates are reference coordinates plus accepted absolute solid
  displacement;
- the fluid interface takes the same displacement through the exact
  conforming trace;
- the exterior fluid mesh boundary remains fixed; and
- every unconstrained fluid interior vertex solves the component-wise P1
  harmonic extension on the immutable reference topology.

The bounded evidence mesh has at least one unconstrained fluid interior
vertex, so this is an executed operator rather than a vacuous boundary copy.
The plan owns its exact driver Field and Connection identities, boundary
roles, quality gate, mesh-motion solver policy, GCL-compatible ALE pullback, and
nonlinear globalization policy. Physics coefficients, equations, and current
state values remain absent.

The accepted portable graph adds one typed geometry action and one
`GclCompatibleAlePullback` transformation. Mesh motion and GCL are not two
optional nodes. Domain discretizations declare `ReferenceConfiguration` for
the solid and `CurrentAleGeometry` for the fluid. The ALE Jacobian is general;
the fixed-reference symmetric MINRES plan cannot be reused.

Existing Realization v1--v3 bytes remain frozen. The moving coupled plan is a
closed Realization v4 payload rather than an optional tail on v3.

## Exact affine Geometry Action

For one reference simplex, let the accepted linear path be

```text
chi(theta) = chi_0 + theta * (chi_1 - chi_0),  theta in [0, 1]
F(theta)   = grad_X chi(theta)
J(theta)   = det F(theta)
w          = (chi_1 - chi_0) / h.
```

The Geometry Action derives the mesh velocity and its current spatial
gradient:

```text
w_X       = (chi_1 - chi_0) / h
grad_x(w) = grad_X(w_X) F_1^-1.
```

It also derives, rather than accepts, the current metric rate

```text
dJ_dt_at_1 = cofactor(F_1) : grad_X(w_X)
```

and proves the affine geometric conservation identity

```text
dJ_dt_at_1 = J_1 * div_x(w).
```

The fully discrete method then provides the stronger operational check:
constant transported coefficients remain a free stream on a moving mesh. The
implementation never accepts coordinates, mesh velocity, its divergence, or
a compensating GCL term independently. This follows the differential ALE
finite-element/DG practice of discretizing time first and evaluating every
weak form and mass matrix on the deformed end-of-step geometry; that design
preserves free streams without a separately evolved volume equation
([Förster, Wall, and
Ramm](https://doi.org/10.1002/fld.1093), [Fehn et
al.](https://arxiv.org/abs/2003.07166)).

The first claim is constant-state/free-stream preservation, not a
space--time finite-volume conservation theorem. Polynomial-in-time
space-conservation schemes remain a compatible later Realization; see
[Ivancic, Sheu, and Solovchuk](https://arxiv.org/abs/1809.06553). Higher
transport-polynomial exactness is also separate: the classical GCL alone does
not preserve higher-degree moments under arbitrary mesh motion ([Cai et
al.](https://arxiv.org/abs/2602.09729)).

## GCL-compatible differential ALE action

The old MINI coefficients are transported by the identical reference basis.
Every mass, stress, constraint, and convection form is evaluated on the
accepted `chi_1` geometry. The backward-Euler fluid action is

```text
rho * integral(
    (u_1 - transported(u_0)) / h * v
  + 0.5 * (((u_1 - w) . grad_x) u_1) * v
  - 0.5 * (((u_1 - w) . grad_x) v) * u_1
  + 0.5 * div_x(w) * u_1 * v
) dx_1.
```

The last term is the geometric correction paired with the energy-skew
relative convection. At zero mesh motion, this is exactly the fixed-domain
energy-skew Realization. For constant transported velocity, the relative
convection and geometric correction cancel under the admitted compatible
boundary trace. Omitting or changing the correction therefore produces a
nonzero free-stream residual rather than a small metadata discrepancy.

Viscous stress, pressure coupling, incompressibility, and body force use the
same accepted current geometry.

The nonlinear local operator owns both the residual and its analytic JVP.
Geometry is algebraically derived from the candidate solid displacement
through the harmonic action, so the monolithic Newton linearization includes
the geometry dependency. A trial geometry is quality-checked before any local
physics action; a line-search trial that inverts or degrades a cell is rejected
without publishing partial assembly.

## Monolithic step and configuration bridge

Backward Euler retains the exact solid kinematic elimination

```text
d_s,1 = d_s,0 + h * v_s,1.
```

The candidate `d_s,1` drives the mesh-motion action inside the same nonlinear
residual. The fluid and solid velocity traces remain one algebraic quotient.
The fluid weak action is evaluated in current ALE geometry, while the
small-strain solid remains in its reference configuration. Interface forces
are compared as physical nodal actions through the explicit
reference/current boundary transformation; a current fluid normal must never
be inserted into the fixed-reference solid form without that transformation.

Reference-host execution uses one bounded damped Newton method and the common
general nonsymmetric linear-solver contract. The converged residual is
independently reassembled. The accepted step checks weak incompressibility,
solid kinematics, interface trace continuity, interface action/power balance,
GCL, geometry quality, and model-time progression before exposing Fields.

## Artifact DAG and moving trajectory

Fixed `SpatialStateEnvelopeV1` and trajectory v1 remain fixed-geometry
contracts. Their `geometry_sha256`, `correspondence_sha256`, and `mesh_sha256`
fields are not reinterpreted.

The moving artifact DAG is acyclic:

```text
DiscreteField
  -> FieldSnapshot
  -> GeometryState (driver snapshot, predecessor, current coordinates)
  -> SpatialStateV2 (same complete snapshots plus GeometryState)
  -> SpatialTrajectoryV2
  -> Run output
```

The moving spatial context freezes the reference Model, Geometry Identity,
correspondence, mesh topology/order, Realization, and complete Field
inventory. Each state adds one exact geometry-state digest. Segment and root
validation require a continuous predecessor chain as well as strict step/time
continuity. A geometry state and a spatial state may reference the same solid-
displacement snapshot without referencing one another cyclically.

Field snapshots continue to store coefficients over immutable reference
entity order. Their physical coordinates are supplied only by the bound
geometry state. Dataset views over moving or remeshed trajectories remain
under RFC 0067.

## First vertical slice

The manifest is registered as `verified`. Its named Cargo integration target
passes the complete ordinary execution path and every acceptance/falsifier
below; contract acceptance or isolated layer tests are not substitutes for
that evidence gate.

The registered CPU reference case uses one refined conforming 2D fluid/solid
triangle topology with a nonempty fluid-interior vertex set. It executes at
least two accepted monolithic steps from one nontrivial initial solid state.
The interface and fluid interior move, topology and memberships remain exact,
and every accepted state is published through the moving geometry/state
lineage.

The slice must prove:

- direct canonical V5 source lowers to the complete typed ALE roles;
- static geometry reduces exactly to the fixed-domain local action;
- each moving cell satisfies the analytic metric identity and the complete
  operator preserves the compatible zero-trace constant-stream probe;
- current interface coordinates equal reference coordinates plus the accepted
  solid trace, and mesh velocity equals the consecutive coordinate difference;
- the numerical trajectory independently reapplies the harmonic interior
  action before publication;
- weak incompressibility, solid kinematics, interface trace, force, and power
  balances close after every step;
- the final nonlinear residual and every analytic Jacobian column agree with
  independent reassembly and centered differences;
- all cells retain positive orientation and the declared mean-ratio quality;
- GeometryState replays exact lineage, topology, quality, path evidence, and
  driver identity, while the moving trajectory replays its complete immutable
  predecessor/prefix chain exactly;
  and
- step refinement shows first-order behavior in a common reference-topology
  mass norm for the bounded manufactured/independent oracle.

## Falsifiers

The slice rejects before accepted evidence:

- stale or substituted Model, reference geometry, correspondence, mesh,
  Realization, predecessor geometry state, driver Field, or Run;
- changed connectivity, cell order, vertex count, or any silent remesh;
- independently supplied or altered mesh velocity;
- missing, duplicated, or changed geometric correction;
- a geometry state not equal to reference coordinates plus the replayed
  absolute motion;
- discontinuous fluid/solid interface coordinates or incompatible interface
  velocity;
- an inverted, degenerate, low-quality, or path-inverting trial geometry;
- an incomplete harmonic interior solve or changed fixed boundary;
- stale pressure closure, non-finite residual/Jacobian, nonlinear
  nonconvergence, or a symmetric-solver substitution; and
- a moving SpatialState or trajectory that omits, reorders, forks, or
  substitutes its geometry-state chain.

## Alternatives considered

### Add mesh velocity to Semantic Model

Rejected. Mesh velocity describes a coordinate realization, not physical
meaning. It would make two mesh-motion algorithms different Models.

### Emit one mesh revision and correspondence per step

Rejected. The existing mesh digest includes coordinates, while correspondence
classifies the reference Cartesian geometry. Rebuilding both would conflate
fixed-topology evolution with the RFC 0065 remeshing boundary and invite
tolerance-based
reclassification of semantic selections.

### Accept coordinates, velocity, and a GCL correction independently

Rejected. A correction can compensate a wrong grid velocity and produce a
small reported defect. One sealed Geometry Action derives all quantities.

### Use a path-averaged conservative grid flux first

Deferred. Integrating the polynomial-in-time map and contravariant grid flux
is attractive, but pairing it with endpoint velocity and continuity requires
a complete space--time conservation contract. The bounded endpoint
differential formulation already provides a replayable geometry action,
free-stream GCL, and exact zero-motion reduction without claiming that larger
method.

### Start with lagged or partitioned coupling

Rejected as the first proof. It adds relaxation, transfer, and iteration
defects before the geometry/GCL boundary is established. A later partitioned
Realization may reuse the same Geometry Action and state artifacts without
changing this monolithic reference.

## Nonclaims

This RFC does not claim topology change, remeshing, AMR, contact, finite-
strain structure, production mesh smoothing, nonmatching transfer, higher
transport-polynomial exactness, higher-order time integration, GPU or MPI ALE,
multiple nonlinear/mesh-motion algorithms, checkpoint/restart, fault recovery,
performance, scale, an exact moving-volume discrete energy identity, ALE
sensitivity, FSI adjoints, shape optimization, or CAD regeneration. Those
require separate typed contracts and evidence.
