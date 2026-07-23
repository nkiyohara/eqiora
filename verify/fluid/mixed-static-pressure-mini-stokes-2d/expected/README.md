# Expected evidence

The registered integration test must accept all of the following from fresh
direct and exact-package compilation:

- boundary roles: left/bottom/top zero velocity, right normal pressure;
- no `ZeroIntegral` constraint, multiplier scale, algebraic gauge row, or
  fabricated gauge observation;
- direct/package/alias/order equality of finalized CSR, RHS, fields, and
  physical evidence;
- bit-identical dimensionless matrices for scale profiles
  `(L,U,P)=(4,0.5,0.75)` and `(4,1,1.5)`, with distinct RHS and equal physical
  reconstruction;
- physical pressure integral `24 Pa m^2`;
- integrated body force `(6,0) N/m`;
- integrated applied traction `(-9,0) N/m`;
- essential reaction `(3,0) N/m`;
- reaction plus body plus traction within the independently derived bound;
- right free midpoint facet action `(-4.5,0) N/m`;
- loaded/full/volume-only algebra proving that the facet changes only admitted
  velocity RHS rows; and
- exact package, Model, Field, mesh, Realization, and solver lineage.

Near misses listed in `case.toml` must fail before accepted solution evidence.
