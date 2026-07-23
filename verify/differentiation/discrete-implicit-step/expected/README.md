# Acceptance criteria

- The discrete primal residual is accepted at `1e-12`.
- Step JVP/VJP duality agrees within `2e-14`.
- Forward sensitivity and total adjoint gradient agree with centered finite
  differences within `2e-8`.
- Solve reports retain normal and transposed orientations respectively.
- An off-manifold next state is rejected before sensitivity analysis.
- Invalid step data and malformed tangent shapes fail closed.
