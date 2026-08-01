# Independent pre-implementation oracles

The implementer did not author, tune, or relax these values. Two independent
provider lanes received only the frozen public claim, non-claims, orientation,
indexing, sizing predicate, quality definition already owned by
`SimplicialMesh`, and resource limits.

## Analytic route — Opus 5

The analytic route derived the structured-grid counts and invariants directly:

- `nu = 4`, `nv = 3`, `V = (nu + 1)(nv + 1) = 20`, and
  `T = 2 nu nv = 24`;
- boundary edges `2nu + 2nv = 14`, interior edges `29`, total edges `43`, and
  `V - E + T = 1`;
- per-cell spacings `5/4 m` and `1 m`, per-triangle area `5/8 m²`, total area
  `15 m²`, and perimeter `16 m`;
- maximum edge `sqrt(41)/4 m` and minimum cell quality `40/41` under the
  repository's existing triangle-quality definition;
- end-cap frame `origin=(-2,-1,4.5)`, `u=+x`, `v=+y`, `normal=+z`, with corner
  indices `0,4,19,15`;
- start-cap frame `origin=(-2,-1,0.5)`, `u=+y`, `v=+x`, `normal=-z`, with corner
  indices `0,3,19,16`.

It also required validate-then-reveal handle semantics, a classification
tolerance separate from sizing, exact affine lifting, a strictly-below quality
rejection, and checked budget arithmetic before allocation.

## Exact enumeration route — Fable 5

The independent route enumerated all vertices and triangles with exact rational
coordinates, reduced undirected edges to multiplicities, and accumulated
determinant areas. It independently obtained `V/T/E = 20/24/43`, boundary and
interior counts `14/29`, Euler characteristic 1, triangle area `5/8`, total area
15, perimeter 16, the same corner indices, and the exact `40/41` quality value.
It separately enumerated the start-cap frame and confirmed distinct coordinate
and connectivity orderings under the frozen orientation rule.

## Binary64 sizing checks

For `L = 5`, target `0x1.2db2eaabf5c80p+1` selects three intervals; its
immediate predecessor `0x1.2db2eaabf5c7fp+1` selects four. With the other axis
fixed at `L = 3` and two intervals, the resulting counts are `V/T = 12/12` and
`15/16` respectively.

For `L = 8.375` (`0x1.0c00000000000p+3`) and target
`1.692005512124953` (`0x1.b12745f33a78cp+0`), the actual binary64 values are:

- `hypot(L/6, L/6) = 0x1.f9587c466ee22p+0`;
- `hypot(L/7, L/7) = 0x1.b12745f33a78bp+0`.

Thus seven is the least accepted count, while both common
`ceil(L * sqrt(2) / h)` evaluation orderings produce eight. The implementation
must use the estimate only as a starting point and correct against the frozen
predicate.
