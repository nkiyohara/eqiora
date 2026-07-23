# Expected evidence

The Rust evidence pins the immutable package semantic and source digests and
requires byte equality with the live `0.3.0` package. It then checks:

- exact direct/package side dispositions independent of Boundary identities;
- exact reduced and unconstrained-full CSR/right-hand-side equality, with only
  the reduced system crossing the solver handoff;
- exact direct/package algebraic and reconstructed displacement equality;
- analytical nodal values and continuous Q1 L2/H1-seminorm errors;
- integrated body force, constrained-DOF reaction, and global balance; and
- direct/package-equal raw recovered Q1 traction, including first-order
  convergence to zero on the right natural side and its deliberate distinction
  from the left algebraic reaction; and
- fail-closed Q1 admission of a retained live multi-Port binding; and
- fail-closed normalization for mismatched stress, an additional direct
  Relation, duplicate exact side identity, and simultaneous trace/flux
  prescription.

The assertions are the executable expected data. No sampled output file is a
second source of truth.
