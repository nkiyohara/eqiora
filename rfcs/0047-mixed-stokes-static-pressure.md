# RFC 0047: Mixed Stokes static pressure and boundary-determined pressure

- Status: Accepted; bounded implementation verified in
  [`fluid.mixed-static-pressure-mini-stokes-2d`](../verify/fluid/mixed-static-pressure-mini-stokes-2d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0043](0043-simplicial-mini-stokes-realization.md),
  [RFC 0045](0045-fieldwise-mixed-realization-and-si-congruence.md), and
  [RFC 0046](0046-power-conjugate-mechanical-boundaries.md)

## Summary

Eqiora admits one mixed steady-Stokes boundary family: zero velocity on a
nonempty part of the exterior and normal pressure on another nonempty part.
The pressure load is canonical Model meaning. Partial velocity elimination,
the absence of a pressure gauge, and exact P1 facet assembly are Realization
choices.

The exact `Eqiora.Mechanics.BoundaryLoads@0.1.0` package depends on the
existing nominal velocity/traction interface and provides
`NormalPressureTraction2d`. It consumes a distinct, root-owned
pressure-valued continuum Field. No package name reaches lowering or
numerical dispatch.

The numerical MINI implementation separates three native local relations:

```text
volume momentum/continuity       11 x 11
zero-integral pressure constraint 4 x 4  (all-velocity path only)
constant-traction P1 facet        4 x 4  (mixed path only)
```

The mixed path has no `ZeroIntegral` constraint, constraint-multiplier scale,
or gauge degree of freedom. The traction boundary fixes the constant pressure
mode.

## Semantic pressure boundary

The neutral package component is

```text
NormalPressureTraction2d
  body       : exact 2D volume support
  face       : exact Boundary of body
  pressure   : occurrence-bound invariant scalar Field on body [Pa]
  mechanical : VelocityTractionBoundary Port on face

  flux(mechanical) - normal(isotropic_lift(pressure)) = 0.
```

The sign follows the complete conserving junction, not a local naming
convention:

```text
fluid interface:       sigma n - f_fluid = 0
conserving Connection: f_fluid + f_terminal = 0
pressure terminal:     f_terminal - p_ext n = 0
------------------------------------------------
                       sigma n = -p_ext n.
```

Positive `p_ext` is therefore compressive. The direct equivalent is

```text
normal(sigma) + normal(isotropic_lift(p_ext)) = 0.
```

`p_ext` must be a Field distinct from solution pressure and force potential.
Reusing solution pressure would cancel the pressure term instead of fixing its
constant mode. The enclosing Model supplies exactly one independent
definition Relation for the coefficient Field. This uses existing Field
support, `isotropic_lift`, and `normal` contracts; no Kernel operator or Model
wire changes.

## Package-neutral boundary closure

The shared boundary normalizer owns only exact junction structure. Besides
the homogeneous trace and flux closures accepted by RFC 0046, it records a
closed prescribed trace or flux law by its exact terminal Relation. It proves:

- one exact two-Port conserving Connection;
- one terminal peer using the common nominal Connector and Boundary;
- exactly one terminal Relation involving that peer; and
- references to only trace or only flux of the peer, never both or another
  Port.

It does not interpret pressure, stress, sign, or coefficient expressions. The
Stokes lowerer alone recognizes the direct and terminal normal-pressure forms,
requires exact Newtonian stress agreement, lowers the coefficient Field's
definition to one `ScalarSpatialExpression`, and retains the Field and
Relation identities for whole-Model closure.

The method-neutral Stokes boundary condition is

```text
VelocityTraceZero
NormalPressure(ScalarSpatialExpression)
PortBinding { exact Connection, exact fluid Port }
```

Zero traction is `NormalPressure(0)`. The first executable mixed slice admits
only spatially constant normal pressure. Coordinate-dependent tapes remain
valid Model meaning but require an explicit facet-quadrature Realization.

## Pressure policy

Pressure treatment is derived from admitted boundary meaning; it is not a
second fluid-specific switch.

| Boundary closure | Field-wise constraint | Algebraic multiplier |
|---|---|---|
| all velocity trace | exactly one `ZeroIntegral(pressure)` | present |
| nonempty velocity plus nonempty normal pressure | none | absent |
| all pressure / live Port / another family | unsupported in this slice | absent |

The existing `FieldwiseSpatialDiscretization.constraints` list is the sole
serialized pressure-policy source. Scaling covers exactly the blocks implied
by that list. An injected old gauge or a missing all-velocity gauge makes the
resolved plan unequal to the exact reconstructed plan.

A connected fluid Domain remains required. The mixed slice also requires a
positive-measure velocity boundary so velocity rigid modes are outside this
bounded claim.

## Numerical boundary plan

The numerical boundary plan classifies complete mesh boundary facets as
velocity-trace or traction facets. Essential vertices are the closure of the
velocity facets. This avoids deciding a corner from coordinates alone: a
corner incident to one velocity side is constrained even when its other side
carries traction.

The volume cell contains only MINI velocity and P1 pressure unknowns. A
separate pressure-integral cell supplies P1 mass weights and the one global
multiplier only for the all-velocity path. A constant-traction facet contains
the two endpoint velocity bases and integrates exactly:

```text
b_endpoint = length(facet) * traction / 2.
```

Bubble, pressure, interior, and gauge rows receive no facet load. Cell,
constraint, and facet packets enter one ordered assembly plan. Reduced loaded
and unconstrained full loaded systems remain the solve and reaction sources.
A full volume-only target may be retained as an explicit RHS-cut evidence
projection; it is never a second solve path.

The solution exposes pressure-reference evidence explicitly. Gauge absence is
represented by absence, not by a fabricated zero value. The existing
misnamed numerical `pressure_mean` observation becomes `pressure_integral`.
Applied traction resultant is retained independently from body force and
essential reaction.

## Coherent-SI lowering

For characteristic `L`, `U`, and `P`, the existing symmetric congruence gives

```text
mu_hat = mu U / (P L),
f_hat  = L f / P,
t_hat  = t / P.
```

The intrinsic-2D weak-functional scale remains `P U L`. Physical integrated
body force, traction, and reaction reconstruct with `P L`; pressure integral
reconstructs with `P L^2`. No heterogeneous-dimensional raw matrix is formed.

## Falsifying verification

The registered `fluid.mixed-static-pressure-mini-stokes-2d` case uses
`Omega=(0,4)x(0,2) m`, `mu=6 Pa s`,
`q=0.75 Pa/m (x-2 m)`, zero velocity on the left, bottom, and top, and
`p_ext=4.5 Pa` on the right. Its exact solution is

```text
u = 0,
p = q + 3 Pa = 0.75 x + 1.5 Pa.
```

It must independently accept, per unit out-of-plane thickness:

- pressure integral `24 Pa m^2`;
- body force `(6,0) N/m`;
- applied traction `(-9,0) N/m`;
- essential reaction `(3,0) N/m`;
- reaction + body + traction equal to zero;
- right midpoint facet action `(-4.5,0) N/m` on the 4 by 2 mesh; and
- no gauge row, value, constraint, or scale.

Direct and exact-package Models must have the same boundary roles, finalized
CSR/RHS, fields, and balance. Two scale profiles with common `L` and `U/P`
ratio must produce bit-identical dimensionless matrices, distinct scaled RHS
values, distinct Realization identities, and equal reconstructed physical
evidence.

The case also rejects wrong terminal or direct signs, a stale gauge, missing
all-velocity gauge, wrong side/parent/support/shape/frame/dimension, reuse of
solution pressure, missing or duplicate coefficient definition, nominal
Connector drift, nonbinary or nonconserving closure, live Ports, facet loads
on forbidden rows, semantic/mesh side mismatch, nonfinite data, and artifact
or plan drift. Zero traction is a secondary no-gauge regression; it is not the
facet-assembly claim.

## Alternatives considered

### Execute only zero traction

Rejected as the headline case. It can prove gauge removal but its zero facet
vector cannot falsify sign, outward normal, P1 load placement, or SI traction
scaling.

### Put ambient pressure in Realization

Rejected. Boundary loading is physical Model meaning. A callback beside a
canonical zero-flux law would let the numerical layer mutate the problem.

### Add an outward-normal vector operator or mesh array API

Rejected. Existing `normal(isotropic_lift(Field))` already carries support,
shape, dimension, frame, and parent orientation. A new operator or array path
would duplicate or bypass stronger contracts.

### Add the terminal to `Mechanics.Interfaces@0.2.0`

Rejected. Changing that exact release changes nominal Connector identity and
makes the existing fluid `0.2.0` package incompatible. A separately versioned
boundary-load package composes against the immutable interface release.

### Retain the zero-integral gauge with traction

Rejected. Static pressure makes the absolute pressure observable. An added
zero-mean constraint changes the physical problem and creates an artificial
multiplier.

### Keep the 12 by 12 cell and pin a dummy gauge to zero

Rejected. Absence is part of the accepted algebra. The constraint is its own
mathematical relation and therefore its own local operator.

## Compatibility

The exact interface and fluid package releases from RFC 0046 remain unchanged.
This RFC adds one package and widens 0.x Rust projections. It changes no
serialized Model, package, Realization, or Run schema. Existing all-velocity
Models reconstruct the same exact plan and numerical result after the local-
operator refactor.

## Nonclaims

This RFC does not implement or claim:

- arbitrary vector or coordinate-varying traction execution;
- open/outflow stabilization, slip, periodic, Nitsche, or weak Dirichlet laws;
- live Port execution, trace transfer, fluid-fluid coupling, or FSI;
- pure-traction velocity problems or multiple pressure components;
- transient or Navier--Stokes flow, ALE, turbulence, or moving geometry;
- another mixed pair, preconditioner, production solver, or distributed
  assembly; or
- a general CAD, geometry-healing, or remeshing path.
