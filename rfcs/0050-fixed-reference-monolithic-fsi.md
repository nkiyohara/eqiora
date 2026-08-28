# RFC 0050: Fixed-reference monolithic fluid--structure interaction

- Status: Implemented and verified for the bounded CPU reference slice;
  [`fsi.fixed-reference-monolithic-step-2d`](../verify/fsi/fixed-reference-monolithic-step-2d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0045](0045-fieldwise-mixed-realization-and-si-congruence.md),
  [RFC 0046](0046-power-conjugate-mechanical-boundaries.md),
  [RFC 0048](0048-dynamic-linear-solid-semantics.md), and
  [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md)

## Summary

Eqiora's first executable fluid--structure interaction slice lowers ordinary
fluid and solid Relations plus one ordinary conserving Connection to one
fixed-reference, conforming, monolithic operator. It introduces no FSI node,
interface callback, or tool-specific coupling object.

The admitted fluid meaning is inertial incompressible Newtonian flow without
advection:

```text
rho_f * derivative(fluid_velocity)
  - div(2 * mu_f * symmetric_part(grad(fluid_velocity))
        - isotropic_lift(fluid_pressure))
  - grad(fluid_load_potential) = 0

div(fluid_velocity) = 0
```

The solid retains RFC 0048's first-order displacement/velocity meaning. Their
two complete-exterior boundary laws expose the same nominal velocity/traction
Connector and one conserving Connection joins exactly one fluid side to one
solid side. The canonical projection is method-neutral; geometry, discrete
spaces, time integration, quotient degrees of freedom, and the solver remain
Realization choices.

## Canonical projection

The combined lowerer identifies exactly one inertial fluid body, exactly one
dynamic linear-solid body, and exactly one compatible live interface. It
retains the exact Domain, Field, Boundary, Port, Connector, and Connection
identities required by numerical finalization.

Recognition is identity-parametric and package-neutral. Direct Relations and
exact package elaboration must produce the same projection even though their
revision-local IDs and source provenance remain distinct. The whole Model is
closed by the union of both volume laws, all closed external boundary laws,
and the common two-Port interface. Any additional live Port or unconsumed
Relation fails before mesh access.

No new Semantic Kernel entity is introduced. “FSI” names this bounded lowering
and evidence case, not a second kind of Model meaning.

## Exact spatial witness

The Realization consumes one exact affine-simplex mesh artifact and RFC 0049's
content-bound body-cell and boundary-facet memberships. It never extracts two
independent coordinate-matched meshes.

The two distinct semantic interface Boundaries must match the canonical fluid
and solid roles and map to the same complete positive-measure facet set. Each
facet has one incident fluid cell and one incident solid cell. Parent-outward
orientation is derived by the geometry correspondence; the Realization accepts
no caller-authored normal sign.

Fluid velocity uses the two-dimensional MINI space `(P1 + bubble)^2`, fluid
pressure uses continuous P1, and solid velocity and displacement state use
vector P1. The bubble trace vanishes. Interface facet vertices therefore
induce an exact P1 velocity quotient without interpolation, mortar, penalty,
or Lagrange-multiplier coupling.

## Monolithic backward-Euler step

For one positive step `h`, backward Euler gives

```text
solid_displacement_next
  = solid_displacement_previous + h * solid_velocity_next.
```

The Realization eliminates `solid_displacement_next` exactly. The sole solved
unknowns are the shared fluid/solid velocity quotient and fluid pressure. The
velocity block is

```text
rho_f / h * M_f + A_f
  + rho_s / h * M_s + h * K_s,
```

with each subdomain contribution scattered only through its exact cell set.
The right-hand side contains the previous fluid and solid velocities and the
solid pre-displacement action `-K_s * displacement_previous`, plus admitted
body loads. Natural interface actions are absent from the right-hand side:
they cancel because the two subdomain weak forms share the same test row.

The complete system is symmetric indefinite and is captured exactly once as a
`CanonicalCsrSystemView`. Reference execution uses MINRES, identity
preconditioning, reproducible reductions, one host CPU thread, and offline
scheduling. Later CUDA execution must consume these same finalized bytes; it
cannot own another FSI lowering.

## Pressure closure

The standalone all-essential Stokes zero-integral constraint is not inherited.
On the admitted coupled problem, a constant fluid pressure acts on the free
normal interface trace and loads the anchored solid. The complete coupled
operator therefore determines absolute pressure.

The finalizer proves this decision from the assembled operator. It requires a
positive-measure interface, a free normal interface trace, a nonzero assembled
constant-pressure action, and a nonsingular admitted saddle operator. Adding a
zero-integral gauge to that operator is rejected. If those facts do not close
the constant mode, gauge-free finalization is also rejected. Pressure policy
is a property of the complete coupled operator, not a copied fluid-boundary
label.

## Dimensional scaling

Coherent physical quantities are mapped directly to a dimensionless local
operator using one positive length scale `L`, one shared interface-velocity
scale `U`, one pressure scale `P`, and weak-functional scale `P U L`. The
fluid and solid velocity Fields must use the same `U` because they share one
algebraic trace.

The time step, densities, viscosity, elastic coefficients, previous velocity,
and previous displacement are converted before assembly. The implementation
does not assemble a dimensionally mixed SI matrix and repair it afterward.
The positive diagonal congruence preserves symmetry and inertia.

## Realization and result boundary

The existing field-wise Realization v2 is single-Domain and cannot honestly
describe this operator. This slice uses a small physics-neutral multi-Domain
field-wise contract. It binds exact Domains and Fields, the exact imported mesh
and conserving Connection, one conforming trace quotient, the time method and
step, scaling, operator properties, solver, target, and schedule. It contains
no equation or material law.

The accepted in-memory solution reconstructs values against the exact
canonical Field identities. This RFC deliberately introduces no durable
fixed-mesh spatial snapshot or trajectory wire. Its Run records input lineage
only and is not a durable trajectory attestation.

## Falsifying verification

The registered `fsi.fixed-reference-monolithic-step-2d` case must prove:

- direct and exact-package authoring lower to equivalent typed roles and the
  same finalized dimensionless operator;
- the exact Model, geometry, correspondence, mesh, Realization, and Run inputs
  replay without drift;
- the interface velocity is bit-identical because it is one quotient degree of
  freedom;
- fluid and solid body-cut weak actions are opposite on every free interface
  row;
- weak incompressibility and backward-Euler solid kinematics close;
- independently reapplying the captured CSR reproduces its right-hand side;
- a nonzero prestrained step satisfies the complete backward-Euler energy
  identity, including both mass increments, elastic increment, and viscous
  dissipation; and
- canonical finalized-operator identity is deterministic.

It must reject before accepted evidence:

- stale or cross-wired Model, geometry, correspondence, mesh, step, state,
  Realization, solver, or execution inputs;
- wrong parents, orientation, partial facets, unmatched trace topology, or an
  accidental point-only interface;
- incompatible Connector, shape, support, unit, or frame;
- a missing, additional, or unrepresented live Port;
- a missing fluid or solid mass block, broken kinematic update, duplicated
  interface degree of freedom, or sign-flipped body action; and
- a stale pressure gauge or an unclosed constant-pressure nullspace.

## Alternatives considered

### Add fluid mass only in the Realization

Rejected. That would execute physics absent from the Semantic Model and make
the kinetic-energy claim dishonest. Inertial fluid momentum is an ordinary
Relation using the existing derivative expression.

### Keep displacement as an independent algebraic unknown

Rejected for this first slice. The four-block first-order system obscures the
shared velocity interface and loses the direct symmetric-indefinite MINRES
handoff. Exact backward-Euler elimination retains the same time-discrete
meaning with a smaller operator.

### Couple two independently solved submeshes

Rejected. Coordinate matching, transferred tractions, and callback iteration
would bypass RFC 0049 identity and conceal interface power defects. Partitioned
execution remains a later independent Realization.

### Reuse the standalone Stokes pressure gauge

Rejected. The coupled solid changes the pressure nullspace. Only the complete
operator may select a pressure reference.

## Nonclaims

This RFC does not implement or claim Navier--Stokes advection, multiple time
steps, durable trajectories, moving geometry, ALE, remeshing, nonmatching
transfer, nonlinear structure, partitioned coupling, production
preconditioning, GPU, MPI, adjoint or shape sensitivity, CAD, or topology
optimization.
