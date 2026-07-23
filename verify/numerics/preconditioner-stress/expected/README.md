# Acceptance

The executable contract, rather than frozen floating-point iteration counts,
defines the portable envelope:

- Jacobi reference counts differ by at most two iterations over the sequence;
- at contrast `10^6`, identity needs at least twice the Jacobi iterations for
  both the reference and faer CG backends; and
- every true residual is no larger than the target derived from the sole
  `SolverPlan`.

The evidence intentionally does not compare wall time.
