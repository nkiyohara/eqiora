# Acceptance criteria

- The primal residual norm is at most `1e-14` before analysis.
- Forward sensitivity agrees with the analytic linear solve within `1e-13`
  and centered finite differences within `2e-8`.
- The adjoint solve report records transposed orientation.
- The adjoint and total parameter gradient agree with analytic values within
  `1e-13` and centered finite differences within `2e-8`.
- A point with residual `0.1` is rejected before any sensitivity solve.
