# RFC 0043: Simplicial MINI Stokes numerical realization

- Status: Accepted; bounded implementation verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0006](0006-spatial-realization-contracts.md),
  [RFC 0017](0017-replicated-linear-execution.md), and
  [RFC 0018](0018-ordered-assembly-execution.md)

## Summary

Eqiora admits one nondimensional, two-dimensional steady incompressible
Stokes numerical realization on connected affine triangular meshes using the
stable MINI pair `(P1 + cell bubble)^2 / P1`, positive degree-four assembly
and degree-six error quadrature, a global mean-pressure gauge, ordered
reduced/full assembly, and a reference symmetric-indefinite MINRES solve.

## Motivation

The solid path already exercises shaped fields, tensor structure, local
operators, and conforming assembly. A future fluid package and fluid--solid
interface additionally need a stable mixed velocity--pressure discretization,
an explicit pressure nullspace policy, and honest saddle-point solver
properties. Adding those concerns first through canonical fluid recognition
would couple unresolved language and package questions to the numerical
stability proof.

This RFC therefore closes the smallest falsifiable numerical seam:

```text
accepted affine triangle mesh
        |
        | numerical body force and complete essential velocity
        v
continuous (P1 + bubble)^2 velocity / continuous P1 pressure
        |
        | one global zero-mean pressure constraint
        v
symmetric indefinite reduced CSR ──> reference MINRES
        |
        v
velocity, pressure, reactions, gauge, residual, and convergence evidence
```

It deliberately does not introduce fluid meaning into mesh, discrete-space,
assembly, or solver contracts. It also does not claim that a Semantic Model,
ModelPackage, or artifact selects this realization.

## Mathematical boundary

On the unitless domain `Omega`, the admitted problem is

```text
-div(2 mu sym(grad(u)) - p I) = f,
 div(u) = 0,
 u = g on the complete boundary,
 integral_Omega p = 0,
```

where `mu` is finite and positive. Every quantity in this first slice is
nondimensional. Raw mixed matrix entries must not be interpreted as a
unit-consistent SI algebra; field congruence scaling and durable unit-aware
mixed-solve identity remain separate work.

The weak algebra uses the symmetric form

```text
a(u, v)       = integral 2 mu sym(grad(u)) : sym(grad(v)),
c(v, p)       = -integral p div(v),
c(u, q)       = -integral q div(u),
m(p, eta)     = eta integral p,
m(q, gamma)   = gamma integral q.
```

The scalar `gamma` is a global Lagrange multiplier for the pressure-mean
constraint. For compatible incompressible boundary data its accepted value
must be zero to the residual-scaled tolerance. It is not a physical pressure,
source term, stabilization coefficient, or local cell unknown.

## Discrete realization

### Mesh and spaces

The mesh is one nonempty connected, intrinsic two-dimensional simplicial mesh
with affine triangle geometry. Connectivity is a numerical admission rule for
this version: one connected component has exactly one constant-pressure
nullspace and therefore exactly one global gauge. Disconnected meshes fail
before assembly rather than receiving an insufficient constraint.

Velocity uses continuous P1 vertex coefficients enriched by one normalized
interior bubble coefficient per cell and component:

```text
b(lambda_0, lambda_1, lambda_2) = 27 lambda_0 lambda_1 lambda_2.
```

The bubble is one at the triangle barycenter and zero on every edge, so the
complete essential trace is represented solely by P1 vertex values. Pressure
uses continuous P1 vertex coefficients. The shared `DiscreteSpace` contract
owns both scalar basis tabulations; the Stokes operator only forms their
mixed vector/scalar product and does not create a second basis system.

MINI is selected over Taylor--Hood for this first slice because it proves a
stable mixed method while retaining a P1 velocity trace suitable for a later
comparison with the existing conforming P1 solid trace. This is not a claim
that MINI is the final high-order or FSI realization.

### Quadrature

The accepted assembly rule is a positive Duffy transform of three-point
Gauss--Legendre quadrature on each reference-square axis. It is declared exact
for triangle polynomials through total degree four. Degree four is required
because products of gradients of cubic MINI bubbles reach that degree. A
lower-order rule is rejected before assembly; the implementation does not
silently underintegrate the bubble block.

Continuous manufactured error norms use a separate positive Duffy rule with
four Gauss--Legendre points per source-square axis, declared exact through total
degree six. The squared velocity error can have degree six because the exact
velocity is quadratic and the discrete MINI velocity contains a cubic bubble.
Using the stronger independent rule prevents the assembly admission threshold
from silently becoming the error oracle's integration threshold.

### Essential data and compatibility

The body force and complete essential velocity are finite numerical callables.
The boundary callable is sampled at boundary vertices, defining the P1 trace.
Before assembly, the implementation integrates that trace against the
topology-derived parent-outward normal on every boundary edge and requires

```text
integral_boundary g dot n = 0
```

to a scaled floating-point tolerance. This gate is essential: without it, the
global gauge multiplier could absorb incompatible prescribed flux and make an
invalid incompressible problem appear solvable.

### Ordered mixed assembly

Each triangle produces one ordinary local contribution over

```text
8 velocity unknowns + 3 pressure unknowns + 1 global gauge occurrence.
```

The eight velocity entries are three P1 bases plus one bubble, each with two
components. The same cell packet maps to two ordered targets:

1. a reduced system with complete-boundary P1 velocity eliminated, all bubble
   velocity coefficients, all pressure coefficients, and the global gauge;
2. a full system retaining boundary P1 velocity for reaction recovery.

The gauge occurrence on every cell scatters to one global algebraic identity.
The local operator contains no global numbering and the assembly maps contain
no fluid meaning. The full system is not a second solve; it applies the same
assembled cell algebra to the reconstructed field to recover constrained
boundary reactions.

## Symmetric-indefinite solve contract

The reduced KKT matrix is asserted as `SymmetricIndefinite`. Exact numerical
symmetry is independently checked by the registered evidence. The accepted
solver tuple is:

```text
backend         eqiora.reference
algorithm       MinimumResidual
preconditioner  Identity
reduction       Reproducible
scalar          f64
relative tol    1e-11
absolute tol    1e-13
iterations      at most 10000
```

The reference algorithm is MINRES and independently reapplies the operator to
require the true residual to meet the selected target. Conjugate gradient is
rejected by exact capability matching because the operator is not positive
definite. No fallback to a general solver or another backend is permitted.

This is the first executable use of the symmetric-indefinite property and
reference MINRES seam. It does not claim faer support, a block preconditioner,
a Schur complement, or artifact-v1 persistence of this solver selection.

## Falsifying verification

The registered
[`fluid.simplicial-mini-stokes-2d`](../verify/fluid/simplicial-mini-stokes-2d/README.md)
case uses uniform affine triangles on the unit square with `mu = 1`, the
degree-four assembly rule, the separate degree-six error rule, and

```text
u = (x^2, -2 x y),
p = x - 1/2,
f = (-1, 0).
```

The exact velocity is divergence-free, supplies the complete essential
boundary trace, and the exact pressure already has zero mean. For refinements
`n = 2, 4, 8`, every consecutive pair must show velocity L2 order greater
than `1.75`, velocity H1-seminorm order greater than `0.85`, pressure L2 order
greater than `0.85`, and discrete-divergence L2 order greater than `0.85`.

Every accepted level must additionally prove:

- exact symmetry of every reduced CSR entry;
- independently accepted true residual;
- the weak continuity residual `||B u||_2` independently of both the mixed
  residual and the strong `||div(u)||_L2` diagnostic;
- pressure mean and gauge multiplier below `2e-10` in magnitude; and
- componentwise boundary-reaction plus integrated-body-force balance below
  `2e-9` in absolute magnitude.

On the middle mesh, one-worker reference assembly and four-worker ordered
Rayon assembly must produce bit-identical reduced/full systems, algebraic
solution, and reconstructed fields. Their reports must retain their distinct
execution identities; numerical identity must not erase provenance.

The case rejects incompatible P1 boundary flux, a disconnected mesh under one
gauge, degree-zero centroid quadrature, nonpositive viscosity, non-finite body
force or essential velocity, a conjugate-gradient request, and MINRES with an
unimplemented Jacobi preconditioner. One- and three-dimensional simplex meshes
also fail at the explicit two-dimensional realization boundary.

## Alternatives considered

### Taylor--Hood P2/P1

Taylor--Hood is mathematically natural and attractive for a later higher-order
fluid realization. It also requires a conforming quadratic simplex velocity
space and introduces edge trace degrees of freedom that the current P1 solid
interface does not share. MINI gives the smaller first experiment while still
exercising mixed stability. Taylor--Hood is deferred, not rejected generally.

### Stabilized equal-order P1/P1

Equal-order interpolation has fewer space types, but stability depends on an
additional stabilization operator and parameter. That would test a chosen
stabilization more than the core mixed-space contract and would obscure which
terms are physical versus Realization-owned. Rejected for the first slice.

### Pin one pressure coefficient

A pinned vertex removes the constant nullspace cheaply but makes the selected
mesh identity part of the pressure gauge and breaks permutation symmetry. The
global mean constraint states the actual continuous normalization and fails
cleanly for disconnected domains. Rejected.

### General operator plus BiCGSTAB or faer

Treating a structurally symmetric KKT matrix as `General` discards useful
mathematical truth and permits solver tuples that do not evidence the intended
symmetric-indefinite seam. The dedicated property plus reference MINRES has a
smaller and more falsifiable capability boundary. External solver adapters may
add the same exact tuple under their own evidence later.

### Canonical and packaged Stokes in the same change

The current Semantic Kernel can express much of the homogeneous Stokes
relation, but nonzero shaped body-force authoring, field-wise Realization
selection, physical fluid Ports, and package identity introduce independent
failure modes. Combining them would make it unclear whether a failure belongs
to meaning, approximation, or execution. Deferred until this numerical oracle
is stable.

## Compatibility and migration

The numerical slice changes no Semantic Model node or wire, ModelPackage
schema, Connection meaning, or artifact wire. It adds a P1-bubble discrete
space, positive triangle quadrature, a numerical Stokes entry point, and the
solver vocabulary/capability needed to state and execute one symmetric-
indefinite reference problem. Existing SPD conjugate-gradient behavior remains
unchanged; exhaustive matches over the additive public enums require the
normal 0.x source migration.

No automatic migration maps a former General/BiCGSTAB plan to MINRES. Exact
solver selection is a Realization decision and must fail closed when a backend
does not advertise the complete tuple.

## Security, safety, and governance

The implementation uses safe Rust and finite caller-owned values. Mesh sizes
and degree counts pass checked arithmetic before allocation. Numerical
callbacks execute linked code only; there is no runtime plugin loading or
Python callback in the solver loop. A failed callback, validation gate,
assembly, or solve exposes no accepted solution evidence.

Any future extension to physical units, nonessential boundaries, multiple
pressure nullspaces, stabilized methods, a new solver adapter, or durable
artifacts changes the capability claim and requires its own registered
falsifier.

## Unresolved questions

- Which field congruence-scaling contract should make mixed SI systems
  numerically comparable without changing physical meaning?
- Whether the first canonical fluid slice should use MINI for direct numerical
  continuity or Taylor--Hood as an independent higher-order realization.
- How field-wise spaces, nullspace policy, and solver block structure enter a
  future Realization artifact without adding physics-specific solver fields.
- Which trace-transfer contract should connect a fluid velocity trace to a
  solid kinematic trace before monolithic and partitioned FSI are compared.

## Nonclaims

This RFC does not implement or claim:

- canonical Stokes recognition, a fluid ModelPackage, or package resolution;
- a physical fluid boundary Port, traction exchange, ALE, or FSI;
- dimensional/SI mixed algebra or field congruence scaling;
- Taylor--Hood, stabilized equal-order, discontinuous, curved, adaptive, or
  three-dimensional spaces;
- natural, traction, pressure, outflow, slip, periodic, or partial essential
  boundaries;
- Navier--Stokes advection, transient flow, turbulence, or multiphase flow;
- block/Schur preconditioning, distributed assembly, MPI, CUDA, or GPU solve;
- faer MINRES or any other non-reference backend for this operator; or
- a durable Realization/Run/MINRES artifact-v1 execution path.
