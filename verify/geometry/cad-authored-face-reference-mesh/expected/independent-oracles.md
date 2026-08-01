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

## Post-review realized-coordinate addendum

The first complete-diff review found an oracle gap rather than an implementation
value mismatch: `L/n` can round down while the final generated coordinate is
snapped to the exact endpoint `L`. The maximum realized interval can therefore
exceed the nominal spacing. Opus 5 and Fable 5 independently re-derived the
repair using distinct bit-level and exact-rational/enumeration routes before the
implementation was changed.

Both routes freeze the same per-axis rule. Compute `s = L/n`; generate
`x_i = i*s` for `i<n` and `x_n = L`; measure every generated binary64 gap and
let `D(n)` be their maximum. The least accepted count satisfies
`D(n).hypot(D(n)) <= h`. The old nominal count is a lower bound; under the
50,000-division cap the realized answer is that count or its successor, so at
most two O(n) coordinate scans are required.

For `L = 3` and `h = 0x1.3651a0eb63341p-1`, nominal n=7 uses
`s = 0x1.b6db6db6db6dbp-2`, but endpoint snapping produces
`D(7) = 0x1.b6db6db6db6e0p-2` and realized diagonal
`0x1.3651a0eb63345p-1 > h`. The least repaired count is eight. A 3 m square then
has `V/T/E = 81/128/208`, 32 boundary and 176 interior edges, and maximum edge
`0x1.0f876ccdf6cd9p-1` (0.5303300858899106 m).

The replacement estimate falsifier is `L = 4.875`
(`0x1.3800000000000p+2`), `h = 0x1.f844a57e8134bp-1`.
`ceil(sqrt(2)*L/h)` and `ceil(L/(h/sqrt(2)))` both give seven. At n=7,
nominal spacing is
`0x1.6492492492492p-1`, maximum generated spacing is
`0x1.6492492492498p-1`, and the realized diagonal
`0x1.f844a57e81353p-1` rejects. At n=8 the exact spacing
`0x1.3800000000000p-1` gives diagonal `0x1.b93c10ceb10e1p-1` and accepts.

The primary, start-cap, and length-5 one-ulp witnesses are unchanged. The old
`L = 8.375`, `h = 0x1.b12745f33a78cp+0` witness changes from nominal n=7 to
realized n=8 and is no longer an estimate falsifier; it must remain a regression
check for endpoint-aware correction rather than evidence for n=7.
