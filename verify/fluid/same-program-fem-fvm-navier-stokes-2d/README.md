# Same-Program FEM/FVM transient Navier--Stokes 2D

This case compiles one exact canonical source once and borrows the resulting
`KernelProgram` into two different spatial Realizations: affine-triangle
MINI/P1 finite elements and collocated Cartesian cell-centered finite volumes.
Both trajectories retain the same Model revision, velocity and pressure Field
identities, complete homogeneous velocity boundary meaning, physical time,
and backward-Euler transformation.

The comparison uses an analytic conservative-load equilibrium rather than an
arbitrary coefficient transfer between unlike spaces. The canonical force
potential is `(1 Pa/m) x`; under the zero-integral pressure gauge, the exact state
is

```text
velocity = 0
pressure = x - 0.5 Pa.
```

Both methods start from zero velocity and zero pressure. The initial momentum
residual is therefore nonzero: execution must construct the affine pressure
response and balance the canonical body force. The test checks every FEM
velocity coefficient and every FVM velocity component, evaluates both
pressures at the same Cartesian cell centers, and compares each method both to
the analytic oracle and directly to the other method. It also requires a
nonzero pressure span, so a no-op or omitted force cannot pass.

The FVM path originally failed this falsifier because boundary traction used a
cell-center pressure at the boundary face. The operator now reconstructs the
boundary pressure with its one-sided Cartesian gradient; deleting that
linearly exact reconstruction reintroduces a nonphysical velocity and fails
both the operator unit test and this composition case.

Run:

```sh
cargo test --locked -p eqiora \
  --test same_program_fem_fvm_navier_stokes_2d --features faer
cargo run --locked -p eqiora-verify -- run \
  --case fluid.same-program-fem-fvm-navier-stokes-2d
```

This closes the same-Program comparison gate in
[RFC 0072](../../../rfcs/0072-collocated-incompressible-finite-volume.md). It
does not claim a
general cross-mesh projection API, equality of method-native coefficients,
nonzero-velocity cross-method transient convergence, method superiority,
production accuracy, wider boundaries, GPU/MPI, ALE, or turbulence. Those
require their own physical observables and falsifiers.
