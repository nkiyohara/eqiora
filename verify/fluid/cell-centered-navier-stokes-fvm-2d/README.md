# Cell-centered transient Navier--Stokes FVM 2D

This case verifies the bounded collocated finite-volume path for the exact
canonical transient incompressible Navier--Stokes Model already used by the
MINI/P1 realization. It adds no fluid equation, boundary meaning, time method,
or pressure stabilization to the Semantic Model.

The typed Realization selects cell-constant velocity and pressure on one
generated Cartesian mesh, backward Euler, centered momentum convection,
centered Newtonian traction, and one transient-consistent momentum-weighted
face flux. The BDF1 history term retains the previous accepted face flux; this
prevents the pressure coupling itself from changing spuriously with time-step
size. One monolithic damped Newton solve advances velocity, pressure, and the
zero-integral gauge.

Every interior face action is retained once. An independent replay scatters
that action into physical momentum and continuity blocks, proves exact
equal-and-opposite cancellation, and checks that momentum convection consumed
the same volume flux. Momentum, physical mass, and gauge residuals have
separate targets derived from their own initial block norms.

The evidence also checks every analytic JVP column, affine-pressure exactness,
nonzero action on all Cartesian checkerboard families, a nonzero discrete-curl
initial state, non-unit scaling, coordinate reflection, velocity reversal, and
fixed-mesh first-order BDF1 step refinement.

Run:

```sh
cargo test --locked -p eqiora --test cell_centered_navier_stokes_2d
cargo run --locked -p eqiora-verify -- run \
  --case fluid.cell-centered-navier-stokes-fvm-2d
```

This closes the bounded incompressible pressure--velocity gates of
[RFC 0072](../../../rfcs/0072-collocated-incompressible-finite-volume.md).
The next gate compares FEM and FVM from the same borrowed `KernelProgram`;
that comparison is deliberately not claimed here. Wider boundaries,
unstructured or nonorthogonal meshes, alternate time/coupling algorithms,
restart artifacts, production preconditioners, GPU, MPI, ALE, and FSI remain
outside this case.
