# Acceptance contract

The public integration target uses a refined conforming two-domain triangle
mesh with a genuine unconstrained fluid-interior vertex. A faer CG solve seals
the component-wise P1 harmonic influence action. Faer BiCGSTAB then advances
the prestrained coupled state monolithically for at least two accepted steps.

Every accepted step must satisfy:

- independently reassembled nonlinear residual at or below its frozen target;
- centered verification of every analytic Jacobian column below `1e-3`, with
  color, globally coupled singleton, and complete residual-assembly counts
  retained as evidence;
- weak continuity within the residual-scaled acceptance bound;
- solid kinematic defect below `1e-12`;
- exact shared interface velocity and fluid-plus-solid action and power
  imbalance below `1e-6`;
- affine metric-identity defect below `1e-10`;
- a nonempty GCL-active moving-cell probe, compatible constant-stream residual
  below `1e-12`, and omitted-correction witness above `1e-8`;
- current mean-ratio quality above `0.3`; and
- positive current and complete-path signed Jacobians.

Before publication, every numerical state must replay as `reference +
harmonic(absolute solid displacement)`. Consecutive coordinates must reproduce
the sole mesh velocity, and their solid-interface quotient must match backward-
Euler velocity within `2e-13` relative scale. Exact zero motion must give
reference coordinates, zero grid velocity, zero divergence and GCL correction,
and identical endpoint maps. The artifact projection separately replays exact
lineage, topology, quality/path evidence, driver identity, complete Field
inventory, and immutable trajectory prefixes without importing the numerical
mesh-motion implementation into the artifact layer.

At common final time `0.02 s`, step widths `0.02`, `0.01`, and `0.005 s` must
produce decreasing successive differences in the consistent solid P1
reference-mass norm, with observed base-two order greater than `0.75`. This is
a bounded first-order regression threshold, not an asymptotic theorem for
other meshes, materials, loads, or nonlinear tolerances.
