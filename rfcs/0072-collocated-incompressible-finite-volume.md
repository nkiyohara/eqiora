# RFC 0072: Collocated incompressible finite-volume realization

- Status: Implemented; bounded 2D execution and same-Program comparison verified
- Authors: Eqiora contributors
- Created: 2026-07-22
- Depends on: [RFC 0045](0045-fieldwise-mixed-realization-and-si-congruence.md),
  [RFC 0058](0058-portable-realization-and-execution-graphs.md), and
  [RFC 0069](0069-conservative-cell-centered-transport.md)

## Summary

One fixed-domain two-dimensional incompressible Navier--Stokes Model may be
realized by a collocated, cell-centered finite-volume method without changing
its canonical meaning. Velocity and pressure use cell-constant spaces on one
generated orthogonal Cartesian mesh. Backward Euler, implicit centered
momentum convection, centered Newtonian face traction, and a linearly exact
momentum-weighted pressure--velocity face flux form one monolithic nonlinear
operator. The sole face mass flux is reused by continuity and momentum
convection.

## Motivation

RFC 0043 and the existing transient path establish a conforming MINI/P1
finite-element realization. A finite-volume realization of the same Model is
needed to test the two-layer architecture rather than introduce a second fluid
meaning. A naive collocated discretization admits pressure checkerboards. A
projection, SIMPLE, or PISO loop would additionally require a split-system
execution graph that the current monolithic solver contract does not own.

The bounded reference therefore needs an explicit pressure--velocity coupling
contract. It must be inspectable in the Realization graph, have an analytic
linearization, and be falsified independently of whether a selected nonlinear
solve happens to converge.

## Canonical meaning

This RFC adds no Semantic Kernel node, source construct, boundary condition,
or alternate fluid equation. It consumes the exact method-neutral lowering
already recognized as

```text
rho * derivative(u)
  + div(rho * outer_product(u, u))
  - div(2 * mu * sym(grad(u)) - I * p)
  - grad(force_potential) = 0

div(u) = 0.
```

Density, viscosity, body-force potential, Domain, Fields, Relations, initial
state, and complete physical boundary inventory remain Model meaning. Cell
placement, interpolation, pressure stabilization, time integration, nonlinear
and linear solvers, scaling, and execution placement remain Realization.

## Realization contract

The admitted spatial tuple is exact:

```text
CellCenteredFiniteVolume
+ GeneratedUniform<2>
+ CellCentroid
+ CellConstant(velocity)
+ CellConstant(pressure)
+ ZeroIntegral(pressure).
```

The velocity Field retains its two semantic components; the scalar space is
applied component-wise. The plan composes this tuple with exactly:

- one `BackwardEulerRelationStep` on the momentum Relation and velocity;
- `ImplicitCentered` momentum convection;
- centered Cartesian Newtonian traction on velocity and pressure;
- `MomentumWeightedLinearExactCoupling` collocated pressure--velocity coupling
  with `BackwardEulerMassAndLocalNewtonian` positive scaling and
  `Bdf1PreviousAccepted` face-flux history; and
- one bounded damped-Newton policy whose linearizations are general.

The exact method choices are separate typed transformations in the portable
Realization graph. A generic field-wise capability is insufficient: an adapter
must explicitly claim this convection, traction, and coupling composition.
No method may be substituted during finalization.

## Unique face mass flux

Every interior face has one lower-to-upper orientation, area `A_f`, normal
`n_f`, center distance `d_PN`, interpolated velocity `u_bar_f`, and positive
momentum inverse scale `d_f`. Define the cell pressure gradient by centered
Cartesian differences with the exact boundary closure. For BDF1, the face
volume flux is

```text
phi_f / A_f = u_bar_f . n_f
  - d_f * (
      (p_N - p_P) / d_PN
      - avg(grad_h(p))_f . n_f
      - rho / dt * (phi_f_previous / A_f - u_bar_f_previous . n_f)
    ).
```

`d_f` is obtained symmetrically from the positive current-step cell scale
`d_P = V_P / a_P`, where `a_P` is exactly BDF1 mass plus the local normal
Newtonian contribution used by this bounded Cartesian profile. Convection and
nonlocal reconstructed-gradient terms are deliberately excluded; this is an
interpolation scale, not a claim to equal the complete Newton diagonal. The
previous accepted face flux is Realization state. Its BDF1 term removes the
otherwise spurious time-step dependence of the pressure coupling. The
pressure-only part vanishes to roundoff for constant and affine pressure on
the admitted Cartesian mesh and has nonzero action on alternating checkerboard
pressure modes.

Each interior `phi_f` is constructed once, scatters equal and opposite
continuity flux, and is the only volume flux used by centered momentum
convection. Boundary face flux comes from the canonical velocity trace in the
initial complete-zero-trace slice. Recomputing distinct continuity and
convection fluxes is invalid.

The pressure action follows the momentum-weighted interpolation family used by
fully coupled collocated finite-volume methods, with linear exactness made an
explicit invariant rather than relying on a name. The transient-history term
and positive coefficient follow the time-consistent formulation of
[Bartholomew et al.](https://doi.org/10.1016/j.jcp.2018.08.030). The
mathematical need for collocated stabilization and compatible discrete
gradient/divergence is
described by Eymard, Herbin, and Latché
([ESAIM](https://www.numdam.org/item/M2AN_2006__40_3_501_0/),
[SIAM](https://doi.org/10.1137/040613081)); a modern fully coupled
momentum-weighted treatment is given by Denner, Evrard, and van Wachem
([JCP](https://doi.org/10.1016/j.jcp.2020.109348)).

## Momentum and continuity residual

For each cell, the endpoint residual contains:

- backward-Euler momentum mass;
- centered convection using the unique endpoint face mass flux;
- the Cartesian face sum of `-(2 mu sym(grad(u)) - I p) n`;
- the exact canonical conservative body force; and
- continuity as the oriented sum of the same face mass fluxes.

The reference implementation supplies residual and analytic JVP from one
retained face/operator structure. Damped Newton solves the complete
velocity--pressure--gauge system. It does not differentiate solver iterations,
freeze an undeclared coefficient, or construct a pressure-correction solve
outside the graph.

## Boundary and gauge scope

The first slice accepts the exact complete zero-velocity trace inventory
already recognized by the canonical transient lowerer. It imposes that trace
through face closure and fixes pressure only by one Realization-owned
zero-integral constraint. It imposes no independent semantic pressure boundary
value. For Newtonian boundary traction, the Realization reconstructs the face
pressure from the admitted cell stencil as

```text
p_f = p_P + (x_f - x_P) dot grad_h(p)_P.
```

The one-sided Cartesian gradient is the same cached linear action used by the
momentum-weighted pressure coupling, and the analytic JVP applies that action
to the pressure direction. This is a numerical trace reconstruction, not a
new physical boundary condition.

Open velocity/traction boundaries, prescribed nonzero velocity, boundary-
determined absolute pressure, periodicity, and moving geometry require their
own exact closure and evidence. Failure to match the complete admitted
inventory occurs before assembly.

## Verification

The registered case must prove all of the following:

- the source bytes, Model digest, semantic revision, and canonical lowered
  fluid identities are unchanged from the existing method-neutral path;
- at least two positive backward-Euler steps advance one nonzero, no-slip,
  discretely divergence-free initial velocity derived from a Cartesian
  streamfunction;
- independent momentum and continuity residual replay agrees with the
  finalized operator at every accepted state;
- every analytic JVP column agrees with a centered finite-difference oracle;
- every interior face has one packet and exact equal-and-opposite continuity
  scatter, and convection consumes the same stored flux;
- global mass balance, zero-integral pressure, gauge residual, and nonlinear
  acceptance hold separately;
- constant and affine pressure give roundoff-zero momentum-weighted correction;
- both Cartesian checkerboard pressure modes give a nonzero correction and
  the unstabilized omission is an active falsifier;
- coordinate reflection and velocity reversal preserve the oriented method;
- non-unit length, velocity, pressure, gauge, and weak-functional scales
  preserve reconstructed physical results within tolerance;
- backward-Euler step refinement shows bounded first-order temporal behavior;
  and
- identity, space, constraint, mesh, quadrature, transformation, solver,
  placement, duplicate-packet, and orientation drift fail closed.

Evidence compares physical cell fields and physical conservation laws, not
method-specific coefficient-vector order.

The registered composition case additionally compiles one affine-load Model
once and borrows that exact `KernelProgram` into the MINI/P1 and collocated
paths. Both distinct spatial Realizations must recover the common closed-form
zero-velocity, affine-pressure equilibrium at shared physical locations while
retaining equal Model, Field, boundary, and time identity. This is a bounded
method-independence witness, not a general cross-mesh projection claim.

## Alternatives considered

- **Unstabilized collocated centered flux.** Rejected because checkerboard
  pressure lies in the discrete nullspace.
- **A plain pressure Laplacian.** Rejected because its coefficient and
  consistency are detached from the realized momentum action; linear exactness
  and momentum scaling would be accidental.
- **SIMPLE, PISO, or projection.** Valuable future Realizations, but they own
  multiple systems, correction state, and schedule. Encoding them inside one
  current nonlinear solve would make the graph false.
- **Staggered/MAC unknown placement.** Mathematically attractive and a valid
  future sibling, but it cannot be represented by the current one-space-per-
  Field field-wise contract without first adding typed entity association.
- **Reuse scalar TPFA and transport transformations.** Rejected: Newtonian
  vector traction, shared pressure--velocity flux, and endpoint nonlinear
  convection have different invariants.
- **Change the canonical Relation.** Rejected because pressure stabilization is
  numerical realization, not fluid meaning.

## Compatibility and migration

This RFC adds sibling Rust contracts and portable in-memory graph variants. It
does not change canonical Model bytes, artifact schemas, package identity, or
the existing MINI/P1 and scalar-transport APIs. No serialized Realization wire
is introduced. Before a future wire publishes these variants, its schema and
compatibility policy require a separate versioned decision.

## Security, safety, and governance

The reference path is safe Rust and bounded host memory. Mesh and iteration
limits are validated before allocation or solve. The numerical claim is
evidence-gated; convergence alone cannot certify pressure coupling.

## Nonclaims

This slice does not claim unstructured/nonorthogonal grids, skewness
correction, higher-order momentum reconstruction, turbulent or compressible
flow, open/nonzero/periodic boundaries, staggered storage, SIMPLE/PISO,
production preconditioning, parallel/GPU execution, ALE, remeshing, FSI,
adjoints, performance, or a general finite-volume component library.
