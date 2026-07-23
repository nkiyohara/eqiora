# RFC 0046: Power-conjugate mechanical boundaries and Port-closed Stokes

- Status: Accepted; bounded implementation verified in
  [`fluid.port-closed-si-mini-stokes-2d`](../verify/fluid/port-closed-si-mini-stokes-2d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0035](0035-field-valued-boundary-interfaces.md),
  [RFC 0041](0041-complete-exterior-port-families.md),
  [RFC 0044](0044-packaged-steady-incompressible-newtonian-2d.md), and
  [RFC 0045](0045-fieldwise-mixed-realization-and-si-congruence.md)

## Summary

Eqiora adds one nominal velocity/traction boundary Connector in the exact
`Eqiora.Mechanics.Interfaces@0.1.0` Model Package and consumes it from
`Eqiora.Fluid.Incompressible@0.2.0`. The fluid package exposes the complete
exterior of one steady Newtonian body without owning a terminal condition or
any numerical choice.

After ordinary package elaboration, one identity-parametric Stokes lowerer
normalizes direct and Port-closed boundary meaning into

```text
TraceZero | FluxZero | PortBinding { connection, port }.
```

The first executable slice admits only four `TraceZero` sides and carries that
Model through the existing Field-wise coherent-SI MINI Realization. `FluxZero`
and a live `PortBinding` remain valid canonical meaning but fail before mesh
inspection at this narrower Realization gate.

This is deliberately not an FSI RFC. The existing quasistatic solid Connector
pairs displacement with traction; the new Connector pairs velocity with
traction. Nominal identity and physical dimensions prevent an implicit
conversion between them.

## Motivation

RFC 0045 connects a closed canonical Stokes Model to stable mixed execution,
but the Model spells four zero velocity traces directly. A reusable fluid law
therefore has no public physical boundary that a future terminal, fluid
subdomain, or dynamic solid can connect to.

Putting a fluid-specific array or mesh facet handle at this seam would bypass
the Semantic Model. Reusing the solid displacement Connector would instead
erase a dimensionally meaningful distinction. The smallest coherent path is

```text
neutral nominal Connector
        |
        v
fluid-owned trace/traction law over exact Boundary Domains
        |
        v
ordinary conserving Connection sets and zero terminals
        |
        v
package-neutral boundary inventory
        |
        v
existing all-essential MINI Realization
```

Every arrow is independently rejectable. No package name reaches numerical
dispatch.

## Exact package contracts

### Neutral mechanical interface

`Eqiora.Mechanics.Interfaces@0.1.0` exports

```text
VelocityTractionBoundary
  trace velocity : m / s
  flux  traction : kg / (m s^2)
  shape           : spatial vector
  frame           : spatial Cartesian
  pairing         : Euclidean boundary duality
```

The pointwise dual product, integrated on the boundary, is mechanical power.
The package also exports `ZeroVelocity2d` and `ZeroTraction2d`. Each terminal
owns one occurrence-bound Boundary Port and one zero Relation only.

The Connector is nominal. Matching dimensions, shape, and frame do not make a
different Connector substitutable. Its release contains no physics law,
Domain, Field, mesh, transfer, or execution policy.

### Incompressible fluid interface

`Eqiora.Fluid.Incompressible@0.2.0` depends exactly on the neutral package. It
retains `SteadyStokesWithPotential2d` unchanged and adds
`NewtonianMechanicalInterface2d` with occurrence obligations for one 2D body,
its exact complete exterior, velocity, pressure, and positive dynamic
viscosity. For every exterior member it contributes

```text
trace(u) - trace(port) = 0,
normal(2 mu symmetric_part(grad(u)) - isotropic_lift(p))
  - flux(port) = 0.
```

The normal is the exact parent-outward orientation of the bound Boundary. The
package owns neither a boundary terminal nor a discretization. Its volume and
boundary Components remain separately reusable.

## Canonical lowering contract

The shared lowering vocabulary records only exact boundary meaning:

```text
PhysicalBoundaryDisposition
  TraceZero
  FluxZero
  PortBinding { exact Connection, exact local Port }
```

It is intentionally phrased as trace and flux, not essential and natural;
the latter names describe how a particular weak Realization treats the laws.
One small private implementation seam may share these structural operations
between elasticity and Stokes:

- enumerate the exact four Cartesian sides;
- validate one field-physical Port on the exact Boundary;
- validate one closed conserving Connection set with a common nominal
  Connector;
- recognize an exact two-Port zero trace or zero flux terminal; and
- retain the complete admitted Relation, Port, Connection, and Connector
  Domain identities for whole-Model closure.

Equation meaning is not shared. The elasticity lowerer continues to recognize
its displacement and isotropic elastic stress. The Stokes lowerer separately
recognizes velocity and
`2 mu sym(grad(u)) - p I`, including coefficient and pressure-sign agreement
with the volume Relation.

Direct `trace(u) = 0` and the flattened fluid-interface/zero-velocity terminal
must produce the same side dispositions. Source names, dependency aliases,
Component names, file order, declaration order, family-member order, and
Connection-member order are not recognition keys.

## Bounded MINI Realization

The existing MINI implementation fixes every boundary velocity vertex and
uses one zero-integral pressure constraint with its gauge multiplier. This RFC
does not obscure that numerical truth behind a generic boundary API.

`steady_stokes_mini_plan_2d` and finalization both require every exact side to
be `TraceZero`. The finalizer repeats the check before mesh normalization, so
an invalid mesh cannot mask an unsupported semantic boundary. Once admitted,
the existing exact Field identities, P1-bubble/P1 spaces, coherent-SI
congruence, reference MINRES, artifact lineage, and physical reconstruction
remain unchanged.

`FluxZero` is retained by canonical lowering but rejected by this Realization.
A traction boundary normally removes the constant-pressure nullspace, so
blindly keeping the current `ZeroIntegral` constraint would change the
physical problem. Natural-boundary execution must introduce its facet terms,
partial essential mask, and pressure-constraint policy together in a later
RFC.

Likewise, `PortBinding` is retained but rejected until a Realization owns an
exact trace-space map and coupling policy.

## Falsifying verification

The registered `fluid.port-closed-si-mini-stokes-2d` case must prove:

1. the public neutral and fluid packages prepare and resolve by exact version
   and semantic digest without privileged search or dispatch;
2. direct zero traces and exact-package zero-velocity terminals lower to four
   `TraceZero` dispositions on the corresponding exact Boundary roles;
3. both forms produce equal dimensionless CSR/RHS data and equal reconstructed
   physical velocity, pressure, gauge, force, and reaction evidence through
   the ordinary Field-wise Realization;
4. changing dependency aliases, source/declaration order, boundary-family
   order, or Connection-member order does not change admitted meaning;
5. velocity/traction dimension, shape, frame, nominal Connector, exact parent,
   stress factor, viscosity, pressure sign, and side completeness fail closed;
6. an exact zero-traction terminal lowers as `FluxZero` but is rejected by the
   all-trace MINI plan before mesh inspection; and
7. an unresolved compatible Port lowers as `PortBinding` but is rejected at
   the same Realization boundary.

The case reuses RFC 0045's already verified numerical oracle rather than
claiming a second MINI implementation. Existing RFC 0044 and 0045 cases must
remain unchanged and pass.

## Alternatives considered

### Reuse the quasistatic solid Connector

Rejected. Displacement/traction is a virtual-work pairing; velocity/traction
is power-conjugate. Their trace dimensions differ, and an implicit derivative
would introduce time semantics that neither steady Model owns.

### Define the Connector inside the fluid package

Rejected. A future dynamic solid should not depend on a fluid library merely
to share the physical interface identity. The neutral package has an immediate
consumer and a closed payload, so it is not an empty registry abstraction.

### Add a universal PDE boundary trait or crate

Rejected for now. Two equation consumers justify a small private structural
helper, not a new public extension framework. Equation-specific stress
recognition remains explicit.

### Execute zero traction in the same slice

Rejected. Partial velocity constraints, facet traction assembly, and pressure
nullspace policy form one mathematical contract. Adding only a boundary mask
would make the current gauge silently wrong.

### Couple current steady solid and fluid Models

Rejected. Their trace variables differ, no structural velocity or time
relation exists, and moving geometry/ALE is undefined. A matrix coupling would
look like FSI while omitting its kinematics.

## Compatibility and migration

The change adds Model Packages and a package-neutral lowered boundary
inventory. It changes no Kernel node, Model wire, package wire, Realization
wire, or Run wire. The existing immutable fluid `0.1.0` fixture remains valid.
The public `0.2.0` package has an exact dependency and is selected only when an
author requests that exact release.

Renaming the previous equation-specific `EssentialZero`/`NaturalZero` lowered
variants to neutral `TraceZero`/`FluxZero` is an intentional 0.x Rust API
correction. No serialized schema uses those Rust names.

## Security and failure ordering

Package loading retains the existing capability-rooted, bounded, exact offline
path. Boundary normalization allocates only sets bounded by the validated
Model. It admits no callbacks, dynamic library, filesystem discovery, or
runtime package dispatch.

Unsupported dispositions fail before mesh access. A failed package,
normalization, plan, artifact, or finalization gate produces no accepted
solution evidence and performs no fallback.

## Nonclaims

This RFC does not implement or claim:

- live fluid Port execution or fluid-fluid coupling;
- natural, open, slip, periodic, or nonzero boundary execution;
- a displacement-to-velocity adapter or implicit Connector conversion;
- structural dynamics or a solid velocity/traction boundary;
- monolithic or partitioned FSI, trace transfer, mortar/Nitsche coupling;
- moving geometry, ALE, shape update, contact, or remeshing;
- transient or Navier--Stokes flow, turbulence, or multiphase physics;
- another mixed space, pressure policy, preconditioner, or production solver;
  or
- a general runtime plugin or package registry.

The intended next evidence order is mixed-boundary Stokes with an exact
pressure policy, dynamic linear-solid semantics using the same neutral
Connector, one fixed-reference small-deformation implicit FSI step, and only
then alternative monolithic/partitioned Realizations and ALE.
