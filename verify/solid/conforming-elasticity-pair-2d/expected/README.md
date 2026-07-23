# Acceptance contract

The executable assertions, rather than a sampled solution vector, are the
expected data for this case.

## Semantic and numerical identity

- direct and exact-package Models retain two distinct volume Domains and one
  two-Port conserving interface Connection;
- both authoring forms lower to the same geometrically ordered, package-neutral
  body contracts and interface description;
- the interface is the upper side of the lower-coordinate body and the lower
  side of the upper-coordinate body, with one shared axis and identical
  tangential interval; and
- direct and package forms produce equal assembled quotient systems, body-local
  systems, solutions, and reported mechanical evidence.

## Realization-owned quotient map

For `n x n` cells on each half-domain, the two meshes each have `(n + 1)^2`
vertices. The interface map must contain exactly `n + 1` ordered vertex pairs,
and the quotient must contain

```text
2 (n + 1)^2 - (n + 1) = (n + 1) (2n + 1)
```

global vertices. With two displacement components and both components fixed on
the left exterior side, the reduced system has exactly `4n(n + 1)` rows.
Interface matching is derived from exact Cartesian topology, not a coordinate
tolerance or semantic-Domain merging.

## Manufactured displacement and convergence

For refinements `n = 2, 4, 8`, every reconstructed nodal value must equal
the piecewise exact displacement to scaled floating-point tolerance. Continuous
quadrature must reproduce

```text
||u - u_h||_L2^2 = h^4 / 192,
|u - u_h|_H1^2   = 5 h^2 / 96,
h = 1 / (2n).
```

The three exact identities imply L2 order two and H1-seminorm order one without
estimating a noisy fitted rate. Every CG solution must pass an independently
recomputed true-residual target.

## Interface and global equilibrium

Applying each body-local full system to the reconstructed local displacement
produces its cut residual. Summing interface entries gives `[3, 0]` for the
negative body and `[-3, 0]` for the positive body, within a scaled tolerance of
`2e-11`; their sum must close to the same tolerance. These weak actions are
not inferred from recovered stress samples.

Every interface vertex in this oracle is free. A separate falsifier constrains
one interface endpoint and requires that row to be unavailable for coupling
equilibrium, because its cut residual also contains a support reaction.

Independent body-load integration must give `[3, 0]` on each body. The external
constrained reaction must be `[-6, 0]`, and the global reaction-plus-load sum
must close within `2e-11` after scaling.

## Raw traction recovery

Independent Q1 stress facet quadrature must reproduce

```text
t_left_interface  = [ 3 + 3h, 0],
t_right_interface = [-3 + 3h, 0],
t_left_interface + t_right_interface = [6h, 0].
```

The raw imbalance must converge at first order. It must not replace, or be
reported as, the exactly balanced weak interface action.

## Fail-closed boundary

The registered integration target rejects an exact member-count mismatch,
same-side Ports, an additional uninterpreted live Port Relation, an unanchored
system, and one-point reduced integration. Lower-level quotient tests reject
unmatched tangential topology before assembly and exercise both interface axes;
this case does not widen those tests into unsupported integration claims.
