# RFC 0069: Conservative cell-centered scalar transport

- Status: Implemented and verified for the bounded Phase-A/Phase-B 2D slices
- Authors: Eqiora contributors
- Created: 2026-07-22
- Depends on: [RFC 0057](0057-canonical-pure-operator-definitions.md) and
  [RFC 0058](0058-portable-realization-and-execution-graphs.md)

## Summary

The finite-volume fluid slice lowers one canonical two-dimensional scalar
advection--diffusion Relation to conservative cell-centered face balances.
Phase A selects implicit first-order upwind convection, implicit orthogonal
diffusion, and a backward difference. Phase B selects previous-state Cartesian
minmod reconstruction while retaining implicit diffusion, giving a linear IMEX
Euler step. The Semantic Model retains the transported Field,
potential-derived advector, diffusion, and exact boundary Relations. Mesh, P0
space, face reconstruction and evaluation time, time method, linear solver,
and execution placement remain typed Realization choices.

The slice admits inflow trace, outflow diffusive flux, and impermeable-wall
diffusive flux. Spatial periodic identification is deliberately deferred: the
current Semantic Kernel has no typed map identifying noncoincident boundaries,
so wrapping opposite mesh faces in the FVM adapter would invent model meaning.

## Canonical meaning

The admitted volume meaning is

```text
derivative(c)
  + div(c * grad(psi))
  - div(kappa * grad(c)) = 0,

psi - psi_exact(x) = 0.
```

`c` is one scalar differential Field. Its spatially uniform scalar canonical
initial value is the only admitted initial state; a caller cannot substitute
fixture-local or mesh-shaped state.
`psi` is one scalar potential Field eliminated by its exact definition
Relation. The advecting velocity is the coordinate gradient of that immutable
scalar tape. The closed lowerer requires a structurally affine potential and retains its
coefficients rather than fabricating a method-owned velocity array. `kappa` is
one positive, structurally coordinate-independent coherent-SI expression.

Every side of the exact Cartesian box has exactly one boundary Relation with
structurally coordinate-independent data:

- `trace(c) - prescribed = 0`; or
- `normal(kappa * grad(c)) - prescribed = 0`.

The boundary name carries no numerical role. At each face, the finalizer
derives the parent-outward velocity sign from `grad(psi)`:

- negative normal velocity requires an exact trace law;
- positive normal velocity requires an exact diffusive-flux law; and
- zero normal velocity requires an exact diffusive-flux wall law.

A side with an incompatible law fails before assembly. Backflow is therefore
rejection, never a silent switch to an extrapolated boundary value.

## Realization contract

The common field-wise spatial contract admits a second exact tuple without
mixing tuple members:

```text
CellCenteredFiniteVolume
+ GeneratedUniform
+ CellCentroid
+ CellConstant
```

The reusable transient transport contract composes that tuple with:

- one `BackwardEulerRelationStep` bound to the exact Relation and Field;
- one `CellCenteredConvection` bound to the same identities and carrying one
  exact `CellCenteredConvectionScheme`;
- one `OrthogonalTwoPointDiffusion` bound to the same identities;
- one `General` linear operator;
- no nonlinear solve.

The admitted convection schemes are closed and make evaluation time explicit:

- `ImplicitFirstOrderUpwind` uses the endpoint donor value; and
- `ExplicitPreviousStateCartesianMinmod` reconstructs a limited face value only from
  the accepted previous state.

The latter is not hidden coefficient lagging inside an implicit scheme. In
combination with the backward difference and endpoint TPFA diffusion it is an
IMEX Euler method. A limiter evaluated at the endpoint would be nonlinear and
requires a different solve contract.

The registered reference evidence resolves this reusable contract to replicated
`f64`, one host worker, and reproducible Jacobi-preconditioned BiCGSTAB. Those
execution choices are exact evidence selections, not hidden restrictions in
the method-neutral transport plan. Resolution also requires an explicit
transport capability witness containing the exact supported convection-scheme
set: generic Field-wise mesh and solver capability does not imply
implementation of either reconstruction, the backward difference, or TPFA.

The existing MINI/P1 transient Navier--Stokes plan remains unchanged. A sibling
transport plan is preferable to widening an energy-skew, nonlinear contract
until its name no longer describes its invariant.

The portable Realization graph contains distinct backward-difference,
cell-centered-convection, and orthogonal two-point-diffusion transformation
nodes feeding one general algebraic system and one linear solve. The
convection node carries the exact scheme, including its evaluation time. The
finalizer validates that complete graph and derives duration, scaling, solver,
and placement from it. The graph is the execution admission input, not a
decorative projection.

## Conservative face assembly

Every generated cell contributes its backward-difference mass and every
canonical facet contributes exactly one oriented flux packet. Every retained
advective trace has one private sparse affine form

```text
c_hat_f = sum_i alpha_i c_i^(n+1) + beta_f.
```

Assembly and independent operator/physical replay interpret that same typed
trace separately. For Phase-A first-order upwind on an interior face with
signed lower-to-upper volume flux `phi`, the advective action is

```text
F = max(phi, 0) c_lower + min(phi, 0) c_upper.
```

The packet scatters `+F` and `-F` to the two cells in one ordered assembly
operation. Centered orthogonal TPFA supplies the diffusive jump. Boundary
packets combine the canonical law with the derived outward sign. No boundary
condition enum enters the Semantic Kernel.

For Phase B, order cells along the one active Cartesian flow axis as upstream
`U`, donor `D`, and downstream `N`. The accepted previous state defines

```text
s_D = minmod(c_D^n - c_U^n, c_N^n - c_D^n)
c_hat_f = clamp(c_D^n + s_D / 2, previous-and-inflow hull).
```

The exact inflow trace supplies the upstream ghost closure; outflow uses a
mirror-symmetric one-sided extrapolation followed by the same hull limiter.
The face value is an explicit constant `beta_f`, so convection moves to the
right-hand side while mass and TPFA diffusion remain implicit. This bounded
slice admits at most one nonzero Cartesian velocity component, at least three
cells along that axis, and rejects an advective Courant number above one half
before assembly. These restrictions are execution facts, not Model meaning.

Before solver exposure, the implementation validates the assembly receipt's
packet count, target count, and graph-admitted placement, then reconstructs
the complete matrix and right-hand side independently from the retained mass
and face contracts. Its ordered sparse structure and coefficients must agree
with the captured CSR within a declared roundoff bound. At the accepted
solution it additionally reapplies the complete CSR and independently
reconstructs every physical cell residual before it verifies

```text
(new mass - old mass) / dt
  + total outward advective-diffusive flux = 0
```

within a declared floating-point tolerance. It also records extrema,
face-packet counts, solver evidence, and the exact resolved Realization.

This follows the standard finite-volume owner/neighbor face-sum structure and
upstream donor selection described by the OpenFOAM documentation:
[Gauss divergence](https://doc.openfoam.com/2306/tools/processing/numerics/schemes/divergence/implementation-details/)
and [first-order upwind](https://doc.openfoam.com/2312/tools/processing/numerics/schemes/divergence/rtm/upwind/).
The limited reconstruction is the bounded Cartesian specialization of the
MUSCL/TVD construction introduced by
[van Leer](https://doi.org/10.1016/0021-9991%2879%2990145-1) and characterized by
[Sweby](https://doi.org/10.1137/0721062).

## Evidence

The registered case `fluid.cartesian-advection-diffusion-fvm-2d` uses the
positive-time spectral solution of a zero-initial, unit-inflow, zero-source
problem with right outflow and horizontal impermeable walls. A mirrored
canonical model reverses velocity and exchanges the two vertical boundary
laws.

The case must prove:

- first-order spatial convergence of the upwind/TPFA realization with
  `dt = h^2 / 2` to isolate the spatial error;
- first-order temporal convergence of the Phase-A backward-Euler path on one
  fixed mesh;
- greater-than-1.6 observed spatial order for the minmod/IMEX realization with
  `dt = h^2 / 2`, while the same canonical Model and semantic revision are
  retained;
- per-step and accumulated global conservation;
- consumption of the exact canonical initial value;
- constant-state preservation and boundedness for the zero-source profile;
- exact equal-and-opposite interior face scatter;
- complete CSR/right-hand-side agreement with independent physical operator
  reconstruction and fail-closed assembly receipts;
- mirrored donor selection and solution under flow reversal;
- invariance under non-unit coordinate, state, and weak-functional scales;
- unchanged Model identity when only Realization choices are reconstructed;
- bounded minmod face traces as a method invariant; bounded accepted states
  for the registered zero-diffusive-flux profile; active limiter evidence,
  exact scheme evidence, and rejection above its declared Courant limit;
- rejection of missing trace closure, backflow, method/space/quadrature drift,
  relation/Field drift, non-affine velocity, spatially varying boundary data,
  duplicated face work, forged assembly receipts, and
  solver/orientation/execution substitution.

## Why spatial periodicity is later

`ClockKind::Periodic` describes model time, not spatial identification.
Cartesian boundary Domains on opposite sides have different embeddings, and
field-valued conserving connections correctly reject them as noncoincident.
The Kernel has no translation/orientation/phase map that would make the two
sides one typed interface.

The later periodic slice must first define that semantic pairing and only then
realize it as mesh-face correspondence. This RFC does not encode periodicity as
a mesh option, string label, or wraparound index.

## Alternatives rejected

- **Put upwind in the Model.** Reconstruction is an approximation choice and
  would give FEM and FVM different model identities.
- **Reuse the nonlinear energy-skew flow plan.** Scalar transport is a linear
  general operator and has no pressure constraint or Newton root.
- **Treat boundary names as inflow/outflow.** Names do not survive composition
  as physics; the canonical Relation plus outward flux sign does.
- **Use only fixture-local arrays.** That would bypass canonical lowering,
  Realization admission, assembly, solver, and evidence lineage.
- **Wrap opposite faces in the adapter.** It invents periodic model meaning.
- **Call a previous-state limiter implicit.** That hides a time-level change
  and makes the portable graph lie about the executed method.
- **Add a general unstructured gradient contract now.** Weighted least-squares
  and multidimensional limiting need nonorthogonal geometry evidence; a name
  without that vertical slice would be premature.

## Nonclaims

This slice does not claim spatial periodicity, a general vector advector Field,
source terms, QUICK, endpoint-nonlinear or multidimensional MUSCL, limiter
families beyond the exact Cartesian minmod profile, multidirectional bounded
transport, unstructured or nonorthogonal FVM, MPFA, incompressible
pressure--velocity coupling, FEM/FVM comparison, turbulence, compressibility,
shocks, ALE, remeshing, adjoints, GPU, MPI, production preconditioning, or
performance.
