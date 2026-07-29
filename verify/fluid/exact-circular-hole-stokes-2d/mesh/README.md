# Immutable source-bound copy of the RFC 0082 chordal mesh

The frozen contract requires that both oracle routes consume *one* immutable, source-bound
copy of the accepted RFC 0082 mesh and independently recheck its coordinates,
positive cell orientation, boundary partition and source-policy facts before
assembly. `mesh.json` is that copy, and `check_mesh.py` is the checker that
proves it.

It is reconstructed from the **public** RFC 0082 construction rule. No Rust
source, no production output and no existing fluid oracle was read.

## The construction

RFC 0082 fixes, and `build_mesh.py` reproduces:

- the circular loop is the regular inscribed polygon with phase
  `theta_i = 2 pi i / n`, `i = 0 .. n-1`;
- every circular direction is cast from the circle centre to the rectangle, and
  the **cast-axis coordinate is assigned the exact rectangle bound** rather than
  reconstructed by the rounding-sensitive `c + ((bound - c)/d) * d`; only the
  transverse coordinate is reconstructed as `c + t * d`;
- for adjacent ray indices `i` and `j = (i + 1) mod n`, with inner circle
  vertices `I_i`, `I_j` and outer rectangle hits `O_i`, `O_j`, **the shared quad
  diagonal is `O_i--I_j`** and the two cells are `(O_i, O_j, I_j)` and
  `(O_i, I_j, I_i)`, stored in positive orientation;
- rectangle corners crossed between rays are inserted in boundary-angle order,
  and one fan triangle per crossed corner fills the area between the outer ray
  chord and the exact rectangle sides;
- a radial hit within the source classification tolerance of a corner reuses
  that corner (no ray does, and `build_mesh.py` rejects if one ever did).

For the RFC 0081 DFG source with a `1e-4 m` requested boundary error, `1e-5`
minimum mean-ratio quality and caller limit 50, this reproduces every accepted
RFC 0082 count independently:

| Quantity | Reconstructed | RFC 0082 accepted |
| --- | ---: | ---: |
| segments | 50 | 50 |
| vertices | 104 | 104 |
| triangles | 104 | 104 |
| boundary / interior edges | 104 / 104 | 104 / 104 |
| Euler characteristic | 0 | 0 |
| x-low (`inlet`) facets | 14 | 14 |
| x-high (`outlet`) facets | 2 | 2 |
| y-low / y-high (`walls`) facets | 19 / 19 | 19 / 19 |
| circular (`cylinder`) facets | 50 | 50 |
| face (`fluid`) cells | 104 | 104 |
| allowance scale | `2.2 m` | `2.2 m` |
| evaluation allowance | `6.252776074688882e-14 m` | same |
| effective epsilon | `9.999999993747225e-05 m` | same |

## The quad diagonal, and why it had to be stated

An earlier revision of this directory returned instead of freezing: RFC 0082 then
said only that *"adjacent inner and outer ray pairs form two positive
triangles"*, which does not say which diagonal splits the quad
`(I_i, O_i, O_j, I_j)`. Positivity cannot decide it — **all 50 ray-pair quads are
strictly convex** (smallest turning cross product `9.4e-4`), so both splits give
104 strictly positive triangles and both satisfied every predicate the RFC then
stated.

RFC 0082 now fixes the diagonal as `O_i--I_j`. That makes exactly **one** mesh
admissible, and it turns the previously unstated convention into a checkable
predicate, which `check_mesh.py` now checks:

- each `O_i` is re-identified by re-casting the published ray rule and matching
  the unique stored outer vertex within `1e-14 m`, and the four outer vertices no
  ray matches are required to be exactly the four rectangle corners;
- every ray pair must carry `(O_i, O_j, I_j)` and `(O_i, I_j, I_i)`;
- every `O_i--I_j` edge must be interior (shared by two cells), and every
  `I_i--O_j` edge must be **absent from the complex**.

## Two files, one mesh

| File | Role | What it is |
| --- | --- | --- |
| `mesh.json` | `accepted` | the one admissible reading of RFC 0082; the only mesh an oracle route may consume |
| `falsifier-wrong-diagonal.json` | `wrong-contract-falsifier` | the excluded `I_i--O_j` split, kept **only** as the negative input of the route's `wrong_quad_diagonal` falsifier |

The falsifier is **not** a second admissible mesh and nothing may consume it as
one. Each file declares its own `role`, the checker requires exactly one file to
declare `accepted`, and it proves the falsifier *violates* the frozen rule rather
than merely differing from the accepted file: `0/50` ray pairs satisfy the frozen
split, `50/50` carry the excluded one, and no `O_i--I_j` edge is interior.

Both files carry **identical vertices and identical boundary facets**, and the
checker asserts that too. The difference is entirely interior connectivity — 100
of the 104 cells, with the four crossed-corner fans shared. That is exactly why a
flux-only check cannot detect the wrong diagonal; see
[`../routes/python/README.md`](../routes/python/README.md).

## Files

| File | SHA-256 |
| --- | --- |
| `canonical_json.py` | `5dda593d8dbdabbdf56e99d2eeeea38a00860b5ac9455f266a3b51082af2d9e9` |
| `build_mesh.py` | `ac82567b69d5d33a12f992ba7d7d6d96733723aeae8c15bbde5fd25e3036645d` |
| `check_mesh.py` | `7c8845b5f29b9e6e8e970d49ccc2831185f4dd38dcac7aaf9e9ffda20ea280d6` |
| `mesh.json` (17774 bytes) | `ada2d08cde5b4e6bd13c97d3b76a45cad810d8eb7acf0f0edc82cd605acd2b39` |
| `falsifier-wrong-diagonal.json` (17924 bytes) | `eccb5642eab811cee1cad0cee8749f7f2a64d16ab300b041fa4efcbe7b61cd2f` |

`build_mesh.py` regenerates both files byte-identically; that was rerun and the
digests above were reproduced.

These are the **packaged** digests. Packaging replaced repository-numbered
tracking prose with stable contract and RFC wording in `role_detail` and
`purpose`, then reran `build_mesh.py`, so the file bytes and therefore these
digests differ from the digests carried in the source worktree. Every one of the
1074 numeric and boolean fields in each file, and the full member order, is
bit-identical to the source; see [`../README.md`](../README.md) for the proof
and the source digests.

## Schema and canonical serialization

The stored schema is `eqiora.verify/exact-circular-hole-stokes-2d/mesh/v1`. It is
closed and small: `schema`, `role`, `role_detail`, `purpose`, `construction`,
`source`, `policy`, `counts`, `vertices_m`, `cells`, `boundary_facets`,
`entity_sets`.

- `vertices_m` — 104 `[x, y]` pairs in metres, binary64 shortest round-trip
  spellings.
- `cells` — 104 positively oriented index triples, each rotated to start at its
  smallest index. Within each ray pair the two triangles are stored in the RFC's
  own listing order, `(O_i, O_j, I_j)` before `(O_i, I_j, I_i)`.
- `boundary_facets` — 104 records `{vertices: [a, b], cell, entity}`. The pair is
  the adjacent cell's **directed** edge, so the fluid lies to its left and the
  right-hand normal `(dy, -dx)` is parent-outward.
- `entity_sets` — `inlet`, `outlet`, `walls`, `cylinder` (facet indices, source
  entities 0, 1, [2,3], 4) and `fluid` (all 104 cells, source entity 0), matching
  the RFC 0081 witness `inlet=[0], outlet=[1], walls=[2,3], cylinder=[4],
  fluid=[0]`.

`canonical_json.py` fixes the one spelling: UTF-8, LF, one trailing newline,
schema-fixed member order (never sorted), two-space indent, arrays of scalars or
of scalar-arrays inline, `repr` for binary64, negative zero normalized, non-finite
rejected. The file bytes *are* the canonical serialization, so the SHA-256 above
is the SHA-256 of the canonical form. `check_mesh.py` re-serializes the parsed
document and requires the stored bytes back.

## What the checker proves

`check_mesh.py` never imports `build_mesh.py`. It proves the stored content from
the RFC 0081 source facts and the RFC 0082 contract — **162 checks, 0 failures**
across both files:

- canonical round-trip and SHA-256;
- the declared `role`, and `quad_diagonal` / `quad_diagonal_name` against the
  geometry actually stored, so a file cannot misdescribe itself;
- all counts, the Euler characteristic `V - E + F = 0`, and edge incidence
  (every edge used once or twice, every directed edge used exactly once, so the
  complex is orientable, and the once-used edges are exactly the facet set);
- strict positive orientation of all 104 triangles, canonical rotation, no
  orphan vertex;
- **the frozen quad diagonal**, as described above, in both directions: the
  accepted file must satisfy it and the falsifier must violate it;
- the circle predicate on the 50 chord vertices (radius and phase to `1e-14`),
  and that the chord loop is simple, strictly convex and strictly contains its
  centre;
- the exact side predicate on the 54 outer vertices — each lies *exactly* on a
  rectangle bound and inside the closed rectangle — and that every facet's stored
  entity agrees with its coordinates and its right-hand normal points out of its
  own cell;
- the complete named partition: sizes 14 / 2 / 38 / 50 / 104, pairwise disjoint,
  covering every facet exactly once, with the RFC 0081 source-entity mapping;
- the measured symmetric Hausdorff bound `max(r - d_min, R_max - r)` within the
  `1e-4 m` request, the measured area and perimeter deficits against the closed
  forms, total area equal to rectangle minus polygon, and the `1e-5` mean-ratio
  quality gate on every cell;
- the binary64 policy: `allowance = 128 eps scale` with `scale = 2.2 m`,
  `epsilon_effective = request - allowance`, the request strictly greater than
  the allowance, and segment **minimality** — `sagitta(50) <= eps_eff <
  sagitta(49)`;
- the four frozen 50-digit ideal values, re-derived at 60 digits;
- the source predicates: bounds strictly increasing, radius and tolerance
  positive, and every centre-to-side distance strictly greater than
  `radius + tolerance`;
- across the two files: identical vertices, identical boundary facets, and
  exactly `100` differing cells.

The diagonal predicate was checked to be decisive, not decorative: a copy of the
falsifier geometry relabelled `role: accepted` fails five checks —
`quad_diagonal_is_frozen_O_i_to_I_j`, `quad_diagonal_edge_is_interior`,
`quad_excluded_diagonal_absent`, `quad_excluded_split_not_present` and
`quad_cell_order_matches_rfc_listing`.

### Two findings from the re-derivation

1. **The frozen ideal values use the exact decimal radius `1/20`, not binary64
   `0.05`.** Re-deriving with `mpmath.mpf(0.05)` misses all four frozen RFC 0082
   constants by exactly one relative half-ulp — `1x` for the `r`-linear sagitta
   and perimeter deficits, `2x` for the `r`-quadratic area deficit. With the
   exact decimal they agree to the full 50 digits. This is correct: RFC 0082's
   closed forms are statements about the exact source circle. It is recorded
   because a re-deriving implementer will hit it.

2. **The source digest is cited, not re-derived.** RFC 0081 freezes
   `sha256 = b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`
   over 511 canonical bytes, but RFC 0079 publishes no entity-set member
   spelling, so those bytes cannot be rebuilt without guessing field names. The
   mesh records the digest with an explicit `sha256_provenance` saying so. Every
   *other* source fact — bounds, centre, radius, tolerance, clearance strictness,
   entity correspondence — is re-derived and checked here.

## Run

```bash
python3 verify/fluid/exact-circular-hole-stokes-2d/mesh/build_mesh.py   # regenerate; byte-identical
python3 verify/fluid/exact-circular-hole-stokes-2d/mesh/check_mesh.py   # 162 checks
```

`build_mesh.py` is standard library only. `check_mesh.py` additionally needs
`mpmath` for the 60-digit closed forms.

## Not claimed

Cross-platform mesh-byte identity is **not** claimed, and RFC 0082 explicitly
does not claim it either. The transverse coordinates come from the platform
`libm` `cos`/`sin`, so a production comparison of vertex inventories must be
tolerance-based, not bitwise. No curved element, mesher, adaptivity, PDE
convergence, drag, lift or Strouhal claim is made here.
