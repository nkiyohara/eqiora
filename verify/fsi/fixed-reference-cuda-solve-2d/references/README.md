# Reference construction

The semantic, spatial, assembly, and physical reference is the registered
[`fixed-reference-monolithic-step-2d`](../../fixed-reference-monolithic-step-2d/README.md)
case. The same prestrained state, fixed affine mesh, scale profile, and
backward-Euler step are finalized twice; only execution placement and reduction
policy differ.

The independent CPU oracle executes the complete finalized system with the
reference reproducible MINRES backend. The CUDA candidate is separately
reapplied and residual-accepted on the serial host before either result is
compared or reconstructed as physical Fields.

The typed device movement, completion, solution-generation, and generic
receipt definitions are prerequisites already falsified by
[`canonical-cartesian-poisson-cuda`](../../../numerics/canonical-cartesian-poisson-cuda/README.md).
This case verifies their exact FSI composition without copying their detailed
synthetic trace reconstruction.

