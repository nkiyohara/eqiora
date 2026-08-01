# Acceptance

The registered integration target must prove all of the following:

- source compilation emits one shaped Relation containing both
  `symmetric_part` and `isotropic_lift` without an elasticity-specific node;
- source lowering preserves both canonical operators and their dimensions,
  while identity-parametric and committed semantic typing enforce exact shape,
  frame, and Cartesian-volume support;
- symmetric component scalarization reads `(i,j)` and `(j,i)` with equal
  weight, while isotropic scalarization reads its scalar exactly on diagonal
  components and emits dimensioned zero off diagonal;
- the current Model and Transaction replay with identical canonical bytes and
  domain-separated digests through the one current owner.

No numerical field, stress, residual norm, convergence rate, or solver output
is expected from this case. Historical Model and Transaction bytes remain
negative specimens in `artifacts.current-model-canonical-identity`.
