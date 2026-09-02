# RFC 0048: First-order dynamic linear-solid semantics

- Status: Accepted; bounded implementation verified in
  [`solid.dynamic-linear-solid-semantics-2d`](../verify/solid/dynamic-linear-solid-semantics-2d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0039](0039-canonical-isotropic-elasticity-2d.md),
  [RFC 0041](0041-complete-exterior-port-families.md), and
  [RFC 0046](0046-power-conjugate-mechanical-boundaries.md)

## Summary

Eqiora admits one method-neutral first-order small-strain dynamic-solid
meaning. Displacement and velocity are distinct continuum Fields on the same
two-dimensional body:

```text
derivative(displacement) - velocity = 0
density * derivative(velocity)
  - div(2 mu sym(grad(displacement)) + lambda I div(displacement))
  - grad(load_potential) = 0
```

The exact `Eqiora.Solid.LinearElasticity@0.4.0` package adds these volume
Relations and a separate complete-exterior interface using the already
released
`Eqiora.Mechanics.Interfaces@0.1.0::VelocityTractionBoundary`. The package
contains no time step, mass matrix, element, solver, or coupling method.

A package-neutral lowerer recognizes the exact Field roles, kinematic and
momentum structure, positive density, elastic coefficients, load tape, and
boundary meaning. It retains live velocity/traction Ports for a later
Realization but performs no spatial or temporal discretization.

## Why first-order form is canonical

The second-order statement `rho * derivative(derivative(d)) = ...` would
require a special nested derivative vocabulary and would hide the quantity
that a fluid shares at an interface. Instead, the canonical Model introduces
velocity explicitly:

```text
v = derivative(d).
```

This is still one network of implicit Relations. It does not classify a
separate solver workflow or prescribe how a Realization stores or eliminates
either Field. A later backward-Euler Realization may use
`d_next = d_previous + step * v_next`; another method may keep both unknowns.
That choice is not Model meaning.

The distinction is also dimensional. Elastic stress depends on displacement,
whereas mechanical power on a boundary pairs traction with velocity. The
older `QuasistaticMechanicalBoundary` therefore remains nominally distinct
and cannot be converted implicitly.

## Exact package release

Version `0.4.0` preserves the connector and four Component declarations from
the immutable `0.3.0` release in the same semantic order. It adds an exact
dependency on the neutral mechanics package and two Components.

### `IsotropicElastodynamicsWithPotential2d`

The Component requires one exact 2D volume, occurrence-bound displacement,
velocity, and conservative-load-potential Fields, one density Parameter, and
two Lamé Parameters. It contributes only the kinematic and momentum Relations;
the package declaration supplies dimensions, while the package-neutral
lowerer proves finite `density > 0`, `mu > 0`, and `lambda + mu > 0` before
admitting the canonical subset.
The enclosing Model owns the Fields, their initial-data lineage when one is
eventually defined, and the independent load-potential definition.

Density has coherent-SI dimension `kg / m^3`. The current intrinsic-2D
meaning is per unit out-of-plane thickness; no plane-stress or plane-strain
reduction is implied.

### `ElastodynamicMechanicalInterface2d`

For every exact member of the complete exterior it contributes

```text
trace(velocity) - trace(port) = 0
normal(stress(displacement)) - flux(port) = 0.
```

`normal` uses the Boundary's exact parent-outward orientation. When a future
fluid Port is joined through an ordinary conserving Connection, equal trace
and summed outward flux express velocity continuity and traction balance.
There is no FSI-specific Connector, sign flag, or package dependency between
fluid and solid libraries.

## Method-neutral lowering

`IsotropicElastodynamicsCartesianModel<2>` records only:

- exact Domain and displacement, velocity, and load-potential Field IDs;
- coherent-SI Cartesian bounds;
- immutable scalar tapes for density, Lamé coefficients, and load potential;
- the exact four-side package-neutral boundary inventory.

The private elasticity boundary recognizer takes two explicit roles:

```text
trace_field
stress_displacement
```

Static elasticity supplies displacement for both. Dynamic elasticity supplies
velocity for the trace and displacement for stress. This is one structural
recognizer with explicit mathematical roles, not two copied implementations
or a universal physics trait.

The dynamic lowerer requires exactly three continuum Fields and three volume
Relations. It recognizes the kinematic relation and exact first-order
momentum tree modulo a global sign reversal of either dynamic residual:
`R = 0` and `-R = 0` produce the same dynamic-solid projection. This does not
rewrite Model identity or canonical bytes. The lowerer does not attempt general
symbolic equivalence or accept arbitrary nonzero scaling. It proves constant
finite `density > 0`, `mu > 0`, and `lambda + mu > 0`, and checks
boundary/volume coefficient agreement. Whole-Model closure rejects every node
not consumed by those facts.

## Falsifying verification

The registered direct and exact-package Models use a `2 m` by `1 m` body,
`rho=5 kg/m^3`, `mu=3 Pa`, `lambda=4 Pa`, and a nonconstant linear load
potential. Four zero-velocity closures must normalize to `TraceZero` while
the package interface still proves displacement-derived outward traction.

Evidence compares bounds, all three Field roles, coefficient values and
identity-retaining Parameter counts, load-tape samples and coordinate JVPs,
and four boundary dispositions. It also verifies exact package and source
digests, immutable `0.3.0` declaration prefixes, dependency alias invariance,
Cartesian boundary declaration/family-order invariance, and
Connection-member-order invariance. A zero-traction terminal
remains `FluxZero`, and a compatible unresolved velocity connection remains
an exact `PortBinding`; neither selects an execution policy.

The case accepts an exact global sign reversal of the kinematic and momentum residuals and
rejects before mesh access:

- missing or malformed displacement/velocity kinematics;
- inertia on displacement or stress on velocity;
- zero or negative density, `mu <= 0`, or `lambda + mu <= 0`;
- mismatched volume and boundary Lamé coefficients;
- an equal-shaped but nominally distinct velocity/traction Connector; and
- an additional volume Relation that lowering would otherwise ignore.

Compiler typing independently rejects dimension, shape, frame, support, and
Connector mismatches.

## Fixed-reference FSI follow-on

The first coupled Realization will use distinct fixed fluid and solid Domains
and distinct fluid and solid Ports of the same neutral Connector, joined by
one ordinary conserving Connection and a matching monolithic shared velocity
trace. The reference formulation follows fixed-interface
Stokes/linear-elastic systems that use fluid velocity and solid
time-derivative velocity in one constrained test space, so the two natural
traction terms cancel without an interface-force callback
([Du et al., DOI 10.1137/S0036142903408654](https://doi.org/10.1137/S0036142903408654)).
Its energy law includes fluid inertia. Therefore the next FSI slice must add
transient fluid semantics; it may not combine the current steady-Stokes slice
with this dynamic solid and claim the cited law. Interface traction-power
cancellation, kinetic/elastic energy, and viscous dissipation become
independent acceptance evidence
([He and Shen, DOI 10.4208/nmtma.2014.1307si](https://doi.org/10.4208/nmtma.2014.1307si)).

Before that slice,
[RFC 0049](0049-geometry-identity-and-mesh-correspondence.md) must close the
narrow oriented Geometry Identity and geometry-to-mesh correspondence needed
to pair the two exact semantic boundaries. That prerequisite is not a CAD
kernel.

The coupled Realization must also derive pressure treatment from the complete
operator. An FSI interface is not a prescribed-velocity wall, and a constant
fluid pressure can act on the solid. The standalone Stokes zero-integral gauge
must not be inherited mechanically.

## Alternatives considered

### Reuse the quasistatic Connector

Rejected. Displacement/traction represents virtual work; velocity/traction
represents power. Equal shapes do not erase different dimensions or nominal
identity.

### Add an implicit displacement-to-velocity Port adapter

Rejected. Differentiation is a Model Relation with initial-condition meaning,
not a connection-set coercion. Hiding it in Port normalization would make
time semantics depend on who consumes the Port.

### Put backward Euler in the package

Rejected. A time step and previous-state binding are Realization and Run
choices. Encoding them in a physics package would break the Semantic
Model/Realization split.

### Execute the continuum Fields through scalar time lowering

Rejected. The current scalar Operator-IR time executor has neither shaped
spatial-state storage nor spatial assembly. Ad hoc flattening would bypass
mesh, Field identity, scaling, and artifact lineage.

### Create an FSI package or Connector now

Rejected. Fluid and solid packages independently consume the neutral
mechanical interface. The root Model's ordinary conserving Connection is the
coupling meaning; monolithic, partitioned, mortar, and ALE policies belong to
later Realizations.

## Compatibility and security

The `0.4.0` release has a new exact identity and does not mutate any frozen
`0.1.0`--`0.3.0` release. Existing verification cases prepare their frozen
versions from their own closed inventories. The current public package path
is the newest authoring release only.

Package loading remains exact, offline, digest-bound, and resource-bounded.
The new lowerer allocates only collections bounded by the validated Model and
admits no callbacks, filesystem discovery, mesh arrays, or dynamic code.
Failure returns no partial lowered model or execution evidence.

## Nonclaims

This RFC does not implement or claim:

- a shaped initial-field artifact or structural-dynamics execution;
- a mass matrix, damping model, time method, modal or harmonic analysis;
- coefficient-sensitive boundary JVP/VJP before Parameter-binding identity is
  closed by [RFC 0055](0055-component-parameter-terms.md);
- transient fluid semantics, FSI assembly, pressure policy, or solve;
- nonmatching trace transfer, mortar, Nitsche, or partitioned coupling;
- moving geometry, ALE, geometric conservation, remeshing, or CAD;
- plane-stress/plane-strain reduction, nearly incompressible locking control,
  anisotropy, nonlinear kinematics, contact, or general 3D; or
- a universal PDE, boundary, time, or plugin abstraction.
