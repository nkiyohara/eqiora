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
- explicit Model v4 and Transaction v4 replay with identical canonical bytes
  and domain-separated digests; and
- every legacy encoder rejects the new nodes, and v3 rejects v4 and a forged
  v3 tag.

No numerical field, stress, residual norm, convergence rate, or solver output
is expected from this case. Exact preservation of the pre-v4 v3 writer is
owned by the separate `eqiora-artifact` `legacy_v3_golden` regression.
