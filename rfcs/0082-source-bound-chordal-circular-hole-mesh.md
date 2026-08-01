# RFC 0082: Source-bound chordal circular-hole reference mesh

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-07-28
- Related RFCs and evidence:
  [RFC 0079](0079-authored-planar-geometry-artifact.md),
  [RFC 0081](0081-exact-circular-hole-geometry.md),
  [`geometry.circular-hole-chordal-reference-mesh`](../verify/geometry/circular-hole-chordal-reference-mesh/README.md),
  and
  [`geometry.circular-hole-chordal-realization-binding`](../verify/geometry/circular-hole-chordal-realization-binding/README.md)

## Summary

One opaque in-memory value derives an error-controlled straight-edged region
and affine-triangle reference mesh from the exact circular-hole geometry of RFC
0081. It retains the exact source digest beside the requested and accepted
boundary error, selected segment count, measured area and perimeter deficits,
named chordal region, and accepted mesh.

Circle centre and radius remain Model geometry. Sampling phase, segment count,
coordinates, quality policy, and approximation evidence remain Realization
state. This distinction lets a later curved-element or production mesher
replace the reference path without changing the exact source or Model.

## Motivation

Treating a polygonal hole as the Model's circle would make mesh refinement
change geometry identity. Returning an unbound `(region, mesh)` pair would
preserve the exact source only in caller convention and would let downstream
solver or presentation code lose which circle it approximates.

A generic mesher interface, Delaunay dependency, or high-order geometry
contract would establish compatibility and numerical claims the first cylinder
path does not need. This RFC adds one bounded owner and reuses existing
`PlanarRegion`, `SimplicialMesh`, mesh quality, artifact, and correspondence
contracts.

## Owned realization

`CircularHoleChordalMeshV1` is constructible only from a validated
`CanonicalGeometryV1`. It privately owns:

- the exact source `GeometryRevisionReference`, derived from source digest;
- requested maximum circular-boundary error;
- a separate binary64 boundary-evaluation allowance;
- the accepted measured boundary-error bound;
- the selected circular segment count;
- measured circle-minus-polygon area and perimeter deficits;
- a canonical straight-edged `PlanarRegion`; and
- a validated `SimplicialMesh`.

The value exposes immutable observations only. There is no constructor from an
independent digest, region, mesh, or metric tuple, and the owner itself has no
durable encoding. A separate binding envelope described below captures only a
validated owner and its exact resources.

## Durable realization binding

`CircularHoleChordalRealizationEnvelopeV1` is the closed canonical durable
binding for this dedicated reference path. It captures the exact source
geometry digest, the owner's request and bit-exact observations, and the
digests of the realized authored planar region, a conforming simplicial mesh,
and its Model-free `authored-planar-region-v1` correspondence.

Its schema domain is
`eqiora.circular-hole-chordal-realization-envelope/v1`. RFC 0008 digest framing
applies to this exact canonical field order:

```text
schema, encoding,
source_geometry_sha256, realized_geometry_sha256,
mesh_sha256, correspondence_sha256,
requested_max_boundary_error_m,
boundary_evaluation_allowance_m, boundary_error_bound_m,
circle_segments, circle_area_deficit_m2, circle_perimeter_deficit_m,
required_minimum_mean_ratio
```

Bounded canonical admission precedes resource access. Replay regenerates the
owner from the supplied exact source, stored request, stored circle-segment
count as a work limit, and stored required minimum mean-ratio threshold. It
then requires bit-exact owner observations, exact region equality, successful
authored-region correspondence validation, and equality of all four resource
digests. A coherent policy change always changes binding identity even when
deterministic regeneration lands on the same resources.

The binding does not serialize or generalize `CircularHoleChordalMeshV1`, and
it does not admit a caller-provided observation tuple. It is the minimum
durable relational witness needed to keep the exact source and all realized
resources inseparable in the future installed Python and Result lineage.

## Approximation contract

The circular loop is a regular inscribed polygon with fixed phase:

```text
theta_i = 2 pi i / n,  i = 0, ..., n - 1
```

At least eight segments are required. For the ideal loop:

```text
sagitta(n)           = 2 r sin^2(pi / (2 n))
area_deficit(n)      = pi r^2 - (n / 2) r^2 sin(2 pi / n)
perimeter_deficit(n) = 2 pi r - 2 n r sin(pi / n)
```

The initial segment candidate uses the stable half-angle inverse:

```text
n0 = ceil(pi / (2 asin(sqrt(epsilon_effective / (2 r)))))
```

and direct stable-sagitta corrections make it minimal for that ideal budget.
When `epsilon_effective >= 2 r`, selection branches to eight before evaluating
the inverse. The cancellation-prone `acos(1 - epsilon/r)` route is forbidden.

## Floating-point boundary

Source classification tolerance remains geometry identity and is used only by
`PlanarRegion` admission and ray-to-corner reuse. It is not mesh approximation
policy.

The reference path instead precommits this binary64 evaluation allowance:

```text
scale_m = max(
  abs(all source bounds),
  abs(all source centre coordinates),
  source radius,
  f64::MIN_POSITIVE
)
evaluation_allowance_m = 128 * f64::EPSILON * scale_m
epsilon_effective = requested_max_error - evaluation_allowance_m
```

The request must be finite and strictly greater than the allowance. The
allowance is a checked policy margin, not an exact-real proof of each libm
operation.

The generated binary64 loop is then measured rather than accepted from the
closed form. Let `d_min` be the minimum endpoint-clamped centre-to-segment
distance over every edge, and `R_max` the maximum centre-to-vertex distance.
Construction proves the loop is simple, strictly convex, and contains its
centre. Its symmetric circle/loop Hausdorff distance is bounded by:

```text
max(r - d_min, R_max - r)
```

Convexity bounds every loop point between those radii. Conversely, each ray
from the interior centre through a circle point meets the loop and supplies the
opposite directed-distance witness. The stored accepted bound adds the
evaluation allowance and must not exceed the caller's request. Binary64 drift
may increase the segment count, never decrease it below the analytic
candidate.

## Reference topology

Every circular direction is cast from the circle centre to the rectangle. The
cast-axis coordinate is set directly to the exact rectangle bound; it is not
reconstructed by the algebraically equal but rounding-sensitive
`c + ((bound-c)/d)*d` expression. Adjacent inner and outer ray pairs form two
positive triangles. Specifically, for adjacent ray indices `i` and
`j = (i + 1) mod n`, inner circle vertices `I_i`, `I_j`, and outer rectangle
hits `O_i`, `O_j`, the shared quad diagonal is `O_i--I_j`. The two cells are
`(O_i, O_j, I_j)` and `(O_i, I_j, I_i)`, with their stored order normalized to
positive orientation. Rectangle corners crossed between rays are inserted in
boundary-angle order, and a deterministic fan fills the area between the
outer ray chord and exact rectangle sides.

A radial hit within source classification tolerance of one corner reuses that
exact corner. Multiple hits within the same corner tolerance, coincident
samples, non-finite intersections, degenerate triangles, invalid loops, and
quality-gate failures reject.

The private reference-path hard limit is 100,000 circular segments. A caller
limit below eight or above that hard limit rejects. A required count above the
caller limit rejects before region or mesh topology allocation. Checked
capacity arithmetic protects the bounded topology construction.

## Exact entity propagation

RFC 0081 fixes four exact rectangle corners, four exact rectangle sides, one
circular boundary, and one face. Source named entity membership expands as:

| Exact source entity | Chordal entity membership |
| --- | --- |
| one rectangle corner | its one exact realized corner |
| x-low/x-high/y-low/y-high | every collinear outer edge on that side |
| circular boundary | every circular chord |
| rectangle-minus-circle face | the one chordal face |

Names, grouping, and aliases are retained. A mesh label never supplies
membership. For the DFG witness, `inlet`, `outlet`, `walls`, `cylinder`, and
`fluid` pass the existing geometry/mesh correspondence and cover every
boundary facet exactly once.

## Independent evidence

Before implementation, a non-implementing Opus 5 Max lane authored the
stdlib-only 80-decimal-digit oracle without reading Rust. It measures the
polygon from coordinates, evaluates the independent closed forms, and checks
the algebraic identities as three routes.

For the RFC 0081 DFG source, a `1e-4 m` requested error, `1e-5` minimum
mean-ratio quality, and caller limit 50 produce:

```text
segments             = 50
outer-loop vertices  = 54
mesh vertices        = 104
triangles            = 104
boundary edges       = 104
interior edges       = 104
Euler characteristic = 0
```

The independent frozen ideal values are:

```text
sagitta(49) = 1.0273036248318289955797595210037224856637053318839e-4 m
sagitta(50) = 9.8663578586421902383159656827472333154739014922844e-5 m
dA(50)      = 2.0654536205467760336685969666957589060533063430286e-5 m^2
dP(50)      = 2.0666771241244346537321549979462280729278040417922e-4 m
```

The DFG evaluation allowance is `6.252776074688882e-14 m`; its dimensionally
separate area-comparison allowance is
`1.9643675380784617e-14 m^2`. The registered test executes the immutable
oracle, verifies its hash and 99 checks, reaches the ordinary artifact and
correspondence path, and exercises the frozen falsifiers. Segment doubling
through 8, 16, 32, and 64 independently demonstrates second-order boundary,
area, and perimeter convergence.

## Compatibility and architecture

No existing exact geometry wire, digest, Model byte, mesh wire, correspondence
wire, or solver contract changes. The new binding is a separate schema domain.
The `eqiora-geometry` public surface rises
from 34 to 35 for the opaque source-bound owner. This is an explicit
architecture change: solver and Studio are its two downstream consumers, and a
loose tuple would duplicate or lose the source-binding invariant.

The `eqiora-artifact` public surface rises by one for the binding envelope. Its
deletion condition, and the owner's, is a future general
geometry-Realization owner that preserves non-forgeable exact-source binding,
error evidence, immutable region/mesh access, canonical bounded admission, and
resource replay while replacing these dedicated types without adding a
parallel wire.

## Nonclaims

This RFC adds no exact curved finite element, isoparametric or NURBS mapping,
general arc, ellipse, spline, B-rep, CSG, multiple hole, arbitrary outer loop,
3D geometry, Delaunay or advancing-front mesher, production mesh quality,
adaptive sizing, boundary-layer mesh, curved quadrature, cross-platform
mesh-byte identity, generic approximate-geometry binding, Model/Run/Result
binding, flow solve, drag, lift, Strouhal reference, PDE convergence, Studio
workflow, or completed cylinder demonstration.
