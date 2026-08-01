# Independent pre-implementation oracles

The implementer did not author, tune, or relax these values. Two independent
provider lanes received only the frozen public claim: the admitted graph and
its six full faces, the input gates, the inward/distance/grading/offset rules,
the vertex-index, loop-order, sorted-split, and orientation rules, the quality
definition already owned by `SimplicialMesh`, and the resource limits. Neither
lane read production mesh code or existing fixtures. Both lanes agreed on every
frozen value and every rejection boundary on 2026-08-01, before implementation
began. If the implementation cannot reproduce a value below, the slice stops
and returns the proof; the values are owned by the evidence lanes and are not
adjusted to match an implementation.

## Analytic route — Fable 5

Closed-form derivation over the prism stack. For a source grid of `nu` by `nv`
squares: surface `V = (nu+1)(nv+1)`, `T_surf = 2*nu*nv`; with `L` layers the
volume mesh has `(L+1)*V` vertices and `3*T_surf*L` tetrahedra. For the
primary end cap (`nu=4`, `nv=3`, `L=2`): 60 vertices and 144 tetrahedra, all
assigned to the body. Facet incidences `4*144 = 576 = boundary + 2*interior`
with the caps contributing `2*T_surf = 48` and each lateral wall
`divisions * L` quads split in two, giving 104 boundary and 236 interior
facets, 340 facets, 255 edges, and Euler characteristic
`60 - 255 + 340 - 144 = 1`, as a solid ball requires.

Offsets follow the geometric grading: with distance 4 and growth 3,
`delta_1 = 4/(1+3) = 1` and `delta_2 = 3`, so offsets are `[0, 1, 4]` — all
exactly representable, with the final plane snapped to the opposite bound
`z = 0.5` and the middle plane at `z = 3.5`. Each prism over a triangle of
area `A = (5/4)(1)/2 = 5/8` splits into three tetrahedra of equal volume, so
`|det J| = 2*A*h`: `5/4` in the thin slab and `15/4` in the thick slab, 72
cells each, minimum determinant `5/4`, and total volume
`72*(5/4)/6 + 72*(15/4)/6 = 60 m³`, the exact box volume.

The minimum quality lies in the thick slab on the `[b0,t0,t1,t2]` cell whose
Frobenius sum takes the doubled longer leg: from its stored vertex 0 the edge
rows are `(0,0,-3)`, `(5/4,0,-3)`, `(5/4,1,-3)`, so
`frob² = 3h² + 2a² + b² = 27 + 2*(25/16) + 1 = 249/8` and `|det J| = 15/4`.
With `q = 3|det|^(2/3)/frob²`, cubing gives
`q³ = 27*det²/(frob²)³ = 27*(225/16)/(249/8)³ = 7200/83³`, hence exactly

```text
q_min = 2*30^(2/3)/83 = cbrt(7200)/83
```

Of the two triangle parities per square, only one attains this Frobenius sum,
so the multiplicity is 12. The same algebra for the lateral pairs gives
`frob² = 6731/144` with `det = 5` on the x pair (`q_min = 432*5^(2/3)/6731`)
and `frob² = 731/36` with `det = 15/4` on the y pair
(`q_min = 27*30^(2/3)/731`).

## Clean-room enumeration route — Opus 5

The independent route enumerated every configuration with exact rational
coordinates from the frozen rules only: grid indexing `k = j*(nu+1)+i`
(u-fast), per-square cells `(b,c,a)` then `(d,a,c)`, volume vertex
`layer*V + source index`, triangle-outer/slab-inner emission, sorted labels
`s0<s1<s2`, the three-tetrahedron split, and the negative-determinant swap of
entries 1 and 2. It reduced undirected edges and facets to multisets, counted
facet multiplicities (none above two), classified every boundary facet into
exactly one bounded face, accumulated exact determinant volumes, and compared
qualities in exact arithmetic via `27*det²/(frob²)³`. It independently
obtained
every count, inventory, determinant multiset, exact minimum quality, and
multiplicity above, for the end cap, the start cap, and all four lateral
faces, and confirmed the same six first and six last oriented cells.

## Frozen ordered cells

End cap, first six (triangle `{0,1,6}`, slabs in order) and last six
(triangle `{13,18,19}`):

```text
[0,6,1,26] [0,1,21,26] [0,21,20,26] [20,26,21,46] [20,21,41,46] [20,41,40,46]
[13,18,19,39] [13,38,18,39] [13,33,38,39] [33,38,39,59] [33,58,38,59] [33,53,58,59]
```

The swap pattern differs between the two triangle parities, which is why both
ends are frozen. These cells, together with the exact offsets, kill
slab-direction and growth reversals that preserve topology, quality, and even
the boundary inventory. The start cap must produce distinct coordinates,
ordering, and handle identity (its first sorted triangle is `{0,1,5}` on the
transposed grid), while reproducing the same counts and multisets.

## Binary64 layer offsets

All frozen offset lists are exactly representable: `[0, 1, 4]` (caps, distance
4), `[0, 1.25, 5]` (x pair, distance 5), `[0, 0.75, 3]` (y pair, distance 3).
`offset_0 = 0` and the final offset equal to the distance are exact by rule,
not by luck, and the target normal coordinate snaps to the opposite exact
bound (`-2 + 5 = 3`, `-1 + 3 = 2`, `4.5 - 4 = 0.5` are also exact here). The
grading normalization must be overflow-safe so that an admitted but extreme
growth produces either a valid strictly increasing offset list or a rejection
— never a silent collapse; collapsed or non-increasing generated offsets
reject even when the tetrahedron budget is satisfied.

## The primary quality trio

Three values are frozen and must never be conflated:

| role | value | hex |
| ---- | ----- | --- |
| exact expression | `2*30^(2/3)/83 = cbrt(7200)/83` | — |
| correctly rounded binary64 of the exact real | `0.23264804448328427` | `0x1.dc7693f445c0dp-3` |
| existing Rust left-associated powf evaluation | `0.23264804448328424` | `0x1.dc7693f445c0cp-3` |

The exact real value is `0.232648044483284279002393442...`; it sits about
`2.1e-17` above the rounding midpoint, so the correctly rounded slot is
unambiguous. Every primary coordinate, determinant (`15/4`), and Frobenius sum
(`249/8`) is exactly representable, so `powf` is the only inexact step in the
production expression `3.0 * det.abs().powf(2.0/3.0) / 31.125`. The binary64
exponent `fl(2/3) = 0x1.5555555555555p-1` is strictly below `2/3`, so the
mathematically exact `3.75^fl(2/3)` is already below `3.75^(2/3)`; on x86-64
Linux/glibc, `pow` returns the correctly rounded `3.75^fl(2/3)` and the two
left-associated rounding steps land the final value one ulp below the
correctly rounded exact quality. That platform observation is recorded, not
claimed cross-platform: the acceptance gates `0.23` (accept) and `0.24`
(reject) carry slack of order `1e-2` against a one-ulp sensitivity of order
`1e-17`, so no gate decision depends on libm bits anywhere.

## Lateral-pair references

The frozen decimals `0.18766537853334692` (x pair) and `0.35661030621548570`
(y pair) are the correctly rounded binary64 values of the exact expressions
`432*5^(2/3)/6731` and `27*30^(2/3)/731`. They are frozen as approximate
("about") deliberately: the 4/3 m grid spacing is not dyadic, so the accepted
source coordinates are the predecessor capability's realized binary64
coordinates (`x_i = i*s` with the endpoint snapped), and the production
minimum quality may sit a few ulps from the ideal-real reference. Only the
wide `0.18` gate is claim-bearing for these pairs; no lateral value is
bit-pinned. This applies in advance the predecessor case's realized-coordinate
lesson: certify what generated arithmetic produces, or leave slack.

## Falsifier derivations

Each precommitted falsifier names the plausible wrong implementation it kills
and the frozen value that kills it:

- **foreign/stale surface or graph** — exact revision and handle/source match
  rejects before any volume topology exists.
- **circular-through-cut target** — admission requires the uncut one-body
  graph with six full rectangular faces; the cut graph rejects and its outer
  bounds must never be filled.
- **outward/reversed sweep** — the frozen inward normal, the middle plane
  (`3.5` end cap versus `1.5` start cap), and the ordered cells all fail.
- **layers 0; growth 0.5, 0, NaN, +infinity; maximum 2 or 1000001** — scalar
  gates reject before allocation.
- **maximum 143 for the primary** — required is `3*24*2 = 144`; one below
  rejects, exactly at the cap accepts.
- **checked overflow** — `3*source_triangles*layers` uses checked arithmetic
  and rejects before allocation.
- **offset underflow/collapse despite budget** — strictly increasing finite
  generated offsets are required independently of the budget.
- **quality gate 0.24 on the primary** — the generated minimum
  `2*30^(2/3)/83 ≈ 0.2326` is strictly below `0.24` and rejects, while `0.23`
  accepts.
- **half-depth and cavity mutants** — artificial exterior facets break the
  frozen 104/236 boundary/interior split and the per-face inventory.
- **partial body or any missing boundary assignment** — all incomplete
  correspondence rejects; 144 body cells and all six inventories are frozen.
- **local triangle-order prism split** — a split ordered per-triangle instead
  of by global sorted labels cracks shared vertical quads: facet multisets
  stop matching and conformance fails.
- **slab direction/growth reversal** — preserves counts, quality multisets,
  and inventories; only the exact ordered cells and exact offsets kill it,
  which is why they are frozen.
- **infinite plane membership** — the generic correspondence semantics are
  bounded face membership; a plane-membership variant is outside the frozen
  rule and diverges on any target whose face does not fill its plane, such as
  the rejected cut graph.

## Agreement

The analytic lane (Fable 5) and the clean-room enumeration lane (Opus 5)
derived all of the above separately and agreed exactly — every count,
inventory, determinant, offset list, ordered cell, exact quality, multiplicity,
binary64 value, and rejection boundary — before this package was frozen.
