# Reference provenance

The semantic, geometry, mesh, correspondence, material, previous-state, and
Realization fixture is reused from
[`fsi.fixed-reference-monolithic-step-2d`](../../fixed-reference-monolithic-step-2d/README.md).
The distributed assembly subject is the accepted physical MPI evidence path
defined by
[`fsi.fixed-reference-distributed-assembly-mpi-2d`](../../fixed-reference-distributed-assembly-mpi-2d/README.md).

The independent oracle executes complete ordered CPU reference assembly,
reference identity-preconditioned reproducible MINRES, and the ordinary FSI
finish without observing MPI solver shards, iterates, reductions, or gathered
values. Reduced and full CSR/RHS values are compared bit-for-bit before solve.
After solve, dimensionless algebraic coefficients and physical Fields divided
by their exact Realization scales are compared with fixed `2e-10` absolute and
relative tolerances. Both paths must separately satisfy their true-residual
and physical FSI acceptance contracts.
