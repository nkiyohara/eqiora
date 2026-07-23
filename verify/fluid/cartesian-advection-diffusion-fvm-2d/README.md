# Conservative Cartesian advection--diffusion FVM 2D

This registered case is the first conservative scalar-transport slice in
[RFC 0069](../../../rfcs/0069-conservative-cell-centered-transport.md). One
canonical Relation is realized as a cell-centered P0 finite-volume method on
a generated Cartesian mesh. Two exact Realization choices execute: implicit
first-order upwind with backward Euler, and previous-state Cartesian minmod
convection with implicit orthogonal diffusion (IMEX Euler). Both use the same
serial-host reference solve.

The Semantic Model owns the transported Field, a potential-derived advector,
diffusivity, and boundary Relations. It does not own a mesh, face donor,
quadrature rule, time integrator, or solver. Boundary names are descriptive
only: the numerical role is derived from the parent-outward sign of
`grad(flow_potential)` and the exact trace or diffusive-flux Relation.
The execution starts from the exact canonical Field initial value; there is no
fixture or caller callback that can replace it.

- Direct flow: [`models/direct.eqi`](models/direct.eqi)
- Reflected flow: [`models/mirrored.eqi`](models/mirrored.eqi)
- Analytic problem: [`models/problem.md`](models/problem.md)
- Model and falsifier inventory: [`models/README.md`](models/README.md)
- Reference provenance: [`references/README.md`](references/README.md)
- Expected-evidence policy: [`expected/README.md`](expected/README.md)

The reflection pair is semantic evidence, not a fixture-local velocity switch.
The direct Model defines `grad(psi) = (1, 0) m/s`; the mirrored Model defines
`grad(psi) = (-1, 0) m/s` and exchanges the vertical trace and flux Relations.
An accepted realization must therefore derive the donor and boundary role from
canonical meaning in both directions.

## Evidence boundary

The executable evidence owns first-order Phase-A spatial/time convergence and
greater-than-1.6 observed spatial order for the nominally second-order minmod
reconstruction against the
positive-time spectral zero-source solution in `models/problem.md`, per-step
and accumulated global conservation, fail-closed assembly receipts,
independent complete-CSR/right-hand-side reconstruction, accepted-solution
CSR versus physical-face residual replay, exact equal-and-opposite interior
scatter, canonical-initial-state consumption, constant-state preservation,
boundedness, reflection symmetry, non-unit coordinate/state/weak-functional
scale invariance, and fail-closed identity and boundary-law checks. The
limited path additionally records the exact scheme, maximum Courant number,
limiter activity, advective face range, and face-hull defect; excessive CFL
and unsupported-scheme substitution fail before assembly or resolution.
The zero-advection model proves that a valid pure-diffusion step has no active
advective face range rather than a fabricated numeric sentinel.

The ordinary canonical-to-Realization-to-assembly-to-solve path is executable
and registered. It verifies all claims above without a fixture-local operator
or hand-entered numerical table:

```sh
cargo test --locked -p eqiora-numerics --test canonical_transport
cargo run --locked -p eqiora-verify -- check \
  --case fluid.cartesian-advection-diffusion-fvm-2d
```

The case does not claim periodicity, a general vector advector Field, source
terms, QUICK, endpoint-nonlinear or multidimensional MUSCL, limiter families
beyond the exact previous-state Cartesian minmod profile, multidirectional
bounded transport, nonorthogonal or unstructured FVM, incompressible flow,
ALE, GPU, or MPI.
