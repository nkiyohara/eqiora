# Preconditioner stress verification

This case tests whether the stable `Identity` and `Jacobi` policies change a
real coupled SPD solve in the way their mathematics predicts. It does not use
a diagonal matrix, because that would make Jacobi an exact solver and provide
weak evidence for a Krylov implementation.

For the one-dimensional Dirichlet discrete Laplacian `T`, the fixture forms

```text
A(c) = S(c) T S(c),    diag(S)^2 spans [1, c].
```

Congruence by a positive diagonal preserves symmetry and positive
definiteness. The manufactured solution is `x = S^-1 y` for one fixed,
multi-frequency `y`. Consequently, diagonal preconditioning presents the same
scaled `T` problem at every contrast, while identity CG sees the increasing
diagonal scaling. Contrasts `1`, `10^2`, `10^4`, and `10^6` form the stress
sequence.

The executable evidence requires:

- both policies to consume the same backend-neutral `SolverPlan` and operator;
- the reference backend to retain nearly invariant Jacobi iteration counts;
- identity iterations to grow and exceed Jacobi by at least a factor of two
  at the terminal contrast;
- the independent faer backend to show the same terminal ordering; and
- every accepted result to carry the requested policy and satisfy the
  independently recomputed true-residual target.

This is a deterministic, host-local, replicated `f64` correctness and
robustness claim. Iteration ratios are acceptance envelopes, not performance
benchmarks. It does not claim that Jacobi is sufficient for production-scale
PDEs, nor does it admit ILU, IC, AMG, or backend-private tuning into the stable
policy vocabulary.

Run:

```bash
cargo test -p eqiora --test preconditioner_stress
cargo run -p eqiora-verify -- run --case numerics.preconditioner-stress
```
