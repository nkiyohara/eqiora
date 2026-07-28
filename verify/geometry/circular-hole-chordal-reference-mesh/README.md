# Chordal circular-hole reference mesh verification

This case covers one source-bound, error-controlled chordal realization of the
exact circular-hole geometry: the caller supplies a finite positive maximum
circular-boundary error, and the realization binds the exact source reference,
the accepted boundary bound, the selected segment count, the measured area and
perimeter deficits, one straight-edged region, and one validated affine-triangle
mesh. The exact source identity stays centre and radius. Segment count, phase,
quality policy, generated coordinates, and approximation metrics are Realization
state and never enter that identity. This is a deliberately bounded reference
path for one DFG-shaped cylinder geometry, not a generic production mesher.

The governing contract is RFC 0082. The exact source this realization consumes
is the sibling case [`../exact-circular-hole-geometry`](../exact-circular-hole-geometry/README.md).

## Frozen oracle

The non-implementing oracle is [`oracle.py`](oracle.py), SHA-256
`0bdbbec6f9ff9c532ba5f30c856d1cd3b25e64949e4b11abf5fa3823e6a25742`. It runs
stdlib-only at 80 decimal digits, reports 99 checks with 0 failures, and was
authored without reading any implementation source. It consolidates three
mutually independent routes and requires them to agree:

- **coordinate** — the inscribed regular polygon is built from vertex
  coordinates only, and every quantity is *measured*: both directed Hausdorff
  distances by search over the boundaries, the perimeter by chord summation, the
  area by the shoelace sum. No closed form is an input.
- **closed form** — the frozen sagitta, area-deficit, and perimeter-deficit
  expressions, evaluated by a high-precision transcendental kernel.
- **identity** — the algebraic identities the selection rule depends on: the
  half-angle sagitta form, the `acos(1 - x) = 2 asin(sqrt(x / 2))` half-angle
  inverse, the chord length, and the shoelace triangle term, each checked as a
  residual against the coordinate construction.

The two directed Hausdorff distances are shown to coincide rather than assumed
to, so the symmetric bound `max(r - d_min, R_max - r)` is measured on both sides.

## Frozen DFG witness

For the 2.2 m by 0.41 m rectangle with a radius-0.05 m circle centred at
[0.2, 0.2] m, a 1e-12 m classification tolerance, a 1e-4 m maximum boundary
error, a 1e-5 minimum cell quality, and at most 50 segments, the oracle freezes:

| Quantity | Value |
| --- | --- |
| accepted segment count | 50 |
| outer loop vertices | 54 |
| mesh vertices | 104 |
| mesh triangles | 104 |
| boundary / interior edges | 104 / 104 |
| Euler characteristic | 0 |
| `sagitta(49)` | 1.0273036248318289955797595210037224856637053318839e-4 m |
| `sagitta(50)` | 9.8663578586421902383159656827472333154739014922844e-5 m |
| area deficit at n = 50 | 2.0654536205467760336685969666957589060533063430286e-5 m² |
| perimeter deficit at n = 50 | 2.0666771241244346537321549979462280729278040417922e-4 m |
| evaluation allowance | 6.252776074688882e-14 m |
| area allowance | 1.9643675380784617e-14 m² |

The approximation policy does not reuse the source classification tolerance: the
evaluation allowance is derived separately from the binary64 epsilon and the
largest source length scale, and the request must be strictly greater than it.
The 104 triangle count is *derived* twice — once from the ray cast plus the
Euler characteristic of the annulus, once from the construction rule of two
triangles per adjacent ray pair plus one fan triangle per crossed corner — and
the two routes are required to agree. The exact cell-quality value is not an
oracle; it must only pass the supplied 1e-5 gate.

## Falsifiers

The oracle freezes, and the Rust evidence must exercise, at least: 49 segments
cannot meet the 1e-4 m request; non-finite, non-positive, allowance-equal, and
one-ulp-below-allowance errors reject before any topology is allocated;
`epsilon_effective >= 2 r` selects eight segments instead of evaluating an
out-of-domain inverse; `max_segments` below the minimum topology count, at 49,
and above the private hard work limit all reject; the naive `acos(1 - x)` route
suffers deep cancellation to an undefined count with no mesh allocated, and
disagrees where it is defined while the stable route stays exact; a quality
threshold above the generated mesh's measured quality is not translated into
success; source digest sensitivity holds; all five exact boundary and region set
mappings propagate without renaming, including grouped walls and at least one
dimension-0 alias; every generated boundary facet is owned exactly once by the
existing correspondence; and doubling the segment count demonstrates
second-order convergence of the boundary, area, and perimeter deficits.

## Run

```bash
python3 verify/geometry/circular-hole-chordal-reference-mesh/oracle.py
```

The oracle passes 99/99 today. The Rust evidence
(`cargo test -p eqiora --test circular_hole_chordal_reference_mesh`) and the
registered case run are not yet claimed: this case is frozen ahead of
implementation, its status is `specified`, and no statement here rests on
production output.

## Not claimed

No exact curved finite element, isoparametric or NURBS mapping, general arc,
ellipse, spline, B-rep, CSG, boolean, CAD, multiple hole, arbitrary outer loop,
3D geometry, Delaunay or advancing-front mesher, production mesh quality,
adaptive sizing, boundary-layer mesh, curved quadrature, durable
realization-to-source binding wire, cross-platform mesh-byte identity, flow
solve, drag, lift, Strouhal reference, PDE convergence, Studio workflow, or
completed cylinder demo is claimed.
