# Acceptance criteria

- The checkpoint and restart edge replay against the canonical lowering.
- Four step primal residuals are accepted at `1e-12` before reverse solves.
- Every adjoint solve records transposed orientation.
- Initial-state and model-Parameter gradients agree with centered differences
  within `3e-8` absolute tolerance.
- A checkpoint state that differs from either adjacent accepted step is
  rejected before reverse accumulation.
