# Authored-face reference mesh

This case freezes the first face-scoped realization on the common immutable CAD
authoring graph. A caller supplies one graph-bound rectangular face handle, a
finite positive target edge length, a cell-quality gate, and a triangle budget.
The result keeps the exact source geometry and correspondence beside one
intrinsic two-dimensional triangular mesh and its exact affine lift into the
selected three-dimensional face.

The case was frozen as `specified` before implementation. Its two oracle routes
were derived independently without reading production mesh code and agreed
before the implementation lane began. The complete frozen results are in
[`expected/independent-oracles.md`](expected/independent-oracles.md); the case is
now `verified` by the registered facade evidence.

## Contract boundary

This is a surface realization, not a body-volume mesh or a durable mesh-policy
schema. It admits rectangular faces only. All six faces of the rectangle
extrusion are eligible; after a circular through-cut, only the four unchanged
lateral rectangular faces remain eligible. Annular caps and the cylindrical cut
wall reject because they do not expose a rectangular face cycle.

The geometry-classification tolerance is fixed independently at `5e-10 m`. It
is not reused as a sizing tolerance. Per-axis subdivision uses the least
positive integer `n` for which the actual binary64 predicate
`(L / n).hypot(L / n) <= h` holds. The caller's triangle budget and all scalar,
handle, face-classification, and checked-arithmetic validation occur before mesh
allocation.

## Primary witness

The rectangle spans `x = [-2, 3] m`, `y = [-1, 2] m`, starts at `z = 0.5 m`,
and has depth `4 m`. Selecting the end cap with target `h = 2 m`, minimum
quality `0.95`, and maximum 24 triangles gives a 4-by-3 interval grid with 20
vertices, 24 triangles, 43 edges, 14 boundary edges, 29 interior edges, Euler
characteristic 1, area 15 m², and boundary perimeter 16 m. Every triangle has
area `5/8 m²`; the minimum quality is exactly `40/41`.

The start cap is a second orientation witness. It swaps the intrinsic axes,
reverses the outward normal, and therefore must not reuse the end-cap coordinate
array, connectivity, or handle.

## Mutation witnesses

Two adjacent binary64 targets at the length-5 subdivision boundary freeze the
actual comparator: `0x1.2db2eaabf5c80p+1` accepts three intervals and its
predecessor `0x1.2db2eaabf5c7fp+1` requires four. A separate
`L = 8.375`, `h = 1.692005512124953` witness kills implementations that return
the common ceiling estimate without correction: both estimate orderings give
eight, while the frozen predicate's least acceptable answer is seven.

A 23-triangle budget rejects the primary witness before allocation, while 24
accepts it. A quality threshold of `0.98` rejects because the generated value
`40/41` is strictly lower. Foreign handles, unknown provenance, non-finite or
non-positive sizing, invalid budgets, annular caps, and the cut wall also reject
before topology is exposed.

## Not claimed

No volume mesh, per-face override hierarchy, persisted policy wire, annular or
cylindrical surface mesh, curved element, exact arc realization, adaptive,
anisotropic, boundary-layer, or production unstructured mesher, Python or
Studio surface, solver integration, or demo is claimed by this case.

## Run

```bash
cargo test -p eqiora --test cad_authored_face_reference_mesh
cargo run -p eqiora-verify -- run --case geometry.cad-authored-face-reference-mesh
```

The evidence composes the source-bound owner through the existing Geometry,
Mesh, and complete region-correspondence envelopes. It exercises both cap
orientations, all supported and unsupported v2 face classes, exact topology and
quality values, sizing-boundary mutants, stale/foreign handles, work budgets,
quality rejection, and independent Geometry-versus-Mesh policy identity.
