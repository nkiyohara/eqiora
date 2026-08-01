# Authored-surface sweep volume mesh

This case freezes the first body-volume capability on the common immutable CAD
authoring graph. One accepted source-bound rectangular surface mesh on any of
the six full outer faces of the same exact immutable uncut one-body authored
rectangle-extrusion graph is swept inward through the complete body into an
accepted conforming positively oriented affine-tetrahedron mesh. One no-wire
owner retains the accepted source surface, the exact target graph/body, the
inward orientation, the layer count, the source-relative growth rate, the
binary64 layer offsets, the caller maximum tetrahedra, the reused
`SimplicialMesh` quality gate/report, and the volume mesh.

The case is frozen as `specified`: implementation has not begun. Its two oracle
routes were derived independently from the frozen public claim, without reading
production mesh code, and agreed before any implementation lane started. The
complete frozen results are in
[`expected/independent-oracles.md`](expected/independent-oracles.md). The
implementation slice must bind the planned evidence target named in
`case.toml` (`[planned_evidence]`: `package = "eqiora"`,
`test = "cad_authored_surface_sweep"`), promoting it to `[evidence]` without
changing any frozen value; an implementer who believes a frozen value is wrong
stops and returns the proof.

## Contract boundary

Only the current exact axis-aligned rectangle-extrusion graph with one body and
six full rectangular faces is admitted. The graph revision and the handle and
source surface must exactly match the target. A circular-through-cut target
rejects, and its outer bounds must never be filled. Complete-body and
all-six-boundary correspondence is accepted only through the existing generic
Cartesian Model/Geometry/Mesh artifact boundary; the generic correspondence
uses bounded face membership, not infinite plane membership. The target model
is [`models/box.eqi`](models/box.eqi), whose `body` and six derived boundary
domains name the inventory rows in `case.toml`.

Input gates: layers positive; growth finite and `>= 1`; `maximum_tetrahedra`
in `[3, 1000000]`; required tetrahedra checked as
`3 * source_triangles * layers` and `<=` the caller maximum. Every check runs
before volume topology allocation.

## Layering and cell order

Inward is the negated parent outward normal, and the sweep distance is the
target box width on the normal axis. Layer thicknesses grade geometrically,
`delta(k+1)/delta(k) = growth` from the source inward, normalized
overflow-safely; `offset_0 = 0` exactly, the final offset equals the distance
exactly, and the target normal coordinate snaps to the opposite exact bound.
Generated offsets must be strictly increasing and finite; a collapse rejects.

Volume vertex `layer * V + source index` stacks the source surface. Cells are
emitted with the source triangle as the outer loop and the slab as the inner
loop. Each triangle's global vertex labels are sorted `s0 < s1 < s2` and each
slab emits `[b0,b1,b2,t2]`, `[b0,b1,t1,t2]`, `[b0,t0,t1,t2]`; a negative
signed global xyz determinant swaps entries 1 and 2, and a zero or non-finite
determinant rejects. This total order makes every shared vertical quad take the
bottom-min to top-max diagonal, so adjacent prism stacks conform. Quality is
the existing `SimplicialMesh` definition
`q = 3*abs(det J)^(2/3)/sum(J_ij^2)`, evaluated in production exactly as Rust
left-associated `dimension * det.abs().powf(2.0 / dimension) /
frobenius_squared`.

## Primary witness

Sweeping the accepted end-cap surface (`origin (-2,-1,4.5)`, `u=+x`, `v=+y`,
outward `+z`, 4-by-3 grid, 20 vertices, 24 triangles) with 2 layers, growth 3,
and a 144-tetrahedron maximum gives offsets `[0, 1, 4]`, the middle plane at
`z = 3.5`, and the target plane snapped to `z = 0.5`. The volume mesh has
60 vertices, 255 edges, 340 facets, and 144 tetrahedra — 104 boundary facets,
236 interior facets, Euler characteristic 1, volume 60 m³, and all 144 cells
assigned to the body. Determinants are exactly `5/4` (72 cells) and `15/4`
(72 cells). The boundary inventory is x-low 12, x-high 12, y-low 16, y-high 16,
z-low 24, z-high 24. The first and last six oriented cells are frozen in
`case.toml`; they kill slab-direction and growth reversals that topology and
quality alone would miss.

The exact minimum quality is `2*30^(2/3)/83 = cbrt(7200)/83` with
multiplicity 12. Three distinct values are frozen and must not be conflated:
the exact expression; its correctly rounded binary64 value
`0.23264804448328427` (`0x1.dc7693f445c0dp-3`); and the existing Rust powf
evaluation `0.23264804448328424` (`0x1.dc7693f445c0cp-3`), one ulp lower
because the exponent `2.0/3.0` is itself rounded before `powf` runs. The
acceptance gate `0.23` accepts and the strict gate `0.24` rejects; both carry
slack many orders of magnitude wider than one ulp, so no gate decision depends
on libm bits. The powf value is x86-64 Linux/glibc evidence; cross-platform
mesh-byte identity is not claimed.

## Start cap and all six faces

The start cap (`origin (-2,-1,0.5)`, `u=+y`, `v=+x`, outward `-z`, 3-by-4
grid) sweeps inward `+z` with the same offsets `[0, 1, 4]` to target
`z = 4.5`, middle plane `1.5` instead of `3.5`. It reproduces the same counts,
determinant/quality multisets, volume, and boundary inventories while requiring
a distinct source handle, coordinates and order, inward direction, and mesh
identity.

At surface target edge 2 m, two layers, growth 3, quality gate `0.18`, and
maximum 144, every face of the same box sweeps to a full-body mesh:

| pair | source grid | offsets | V/E/F/T | boundary | min det | exact min quality |
| ---- | ----------- | ------- | ------- | -------- | ------- | ----------------- |
| x    | y3 x z3     | 0, 1.25, 5 | 48/197/258/108 | 84 | 5/3 | `432*5^(2/3)/6731` |
| y    | x4 x z3     | 0, 0.75, 3 | 60/255/340/144 | 104 | 5/4 | `27*30^(2/3)/731` |
| z    | x4 x y3     | 0, 1, 4    | 60/255/340/144 | 104 | 5/4 | `2*30^(2/3)/83` |

Per-face boundary inventories are frozen in `case.toml`. Opposite faces give
the same counts through distinct source, inward, and mesh identities. The
lateral-pair f64 quality values (`about 0.18766537853334692` and
`about 0.35661030621548570`) are the correctly rounded values of the exact
expressions, frozen as approximate because the 4/3 m grid spacing is not
dyadic, unlike the primary witness whose every coordinate, determinant, and
Frobenius sum is exact in binary64.

## Falsifiers

A foreign or stale surface or graph, the circular-through-cut target, an
outward or reversed sweep, zero layers, growth below one, zero, NaN, or
infinite growth, caller maxima 2 or 1000001, maximum 143 for the primary
witness, a checked-arithmetic overflow, and offset underflow or collapse each
reject before volume topology exists. A quality gate of `0.24` rejects the
primary witness. Half-depth and cavity mutants fail the frozen boundary and
interior facet counts; a partial body or any missing boundary assignment fails
complete correspondence; a per-triangle local-order prism split cracks shared
vertical quads and fails the frozen facet counts; slab-direction and growth
reversals are killed by the exact ordered cells and exact offsets even when
topology and quality still pass; and correspondence by infinite plane
membership instead of bounded face membership is outside the frozen semantics.

## Not claimed

No cut or holed-body sweep, prism or wedge cell, wire or persisted mesh
policy, arbitrary accepted surface, arbitrary, rotated, or approximate CAD
frame, curved or high-order cell, boundary-layer semantics beyond geometric
source-relative grading, local refinement, production unstructured mesher,
solver, Python, Studio, demo, or cross-platform mesh-byte identity is claimed.
The total-order prism kernel is compatible with future triangle surfaces, but
only the six exact faces are claimed here.

## Why one no-wire owner

The owner reuses the existing `SimplicialMesh` quality gate/report and the
existing generic Cartesian Model/Geometry/Mesh correspondence rather than
introducing structure:

- a loose tuple of surface, parameters, and mesh leaves the binding invariant
  — that source, target, orientation, offsets, budget, and result stay
  consistent — with no single owner, so every consumer revalidates it;
- a new wedge/prism cell family would add a public cell variant, a second
  quality theory, and a second conformance vocabulary for a claim that affine
  tetrahedra already close;
- a universal mesh-policy registry or persisted wire is a durable contract no
  claim here needs — the owner persists nothing, which is what "no-wire"
  means; and
- a new correspondence schema would widen a shared seam the existing generic
  boundary already expresses: complete body plus six bounded-face boundary
  assignments.

Curved or rotated face membership is a later Geometry capability with its own
evidence, not a reason to generalize the bounded-face rule now.

## Run

```bash
# Future evidence binding once the implementation slice lands.
cargo test -p eqiora --test cad_authored_surface_sweep
cargo run -p eqiora-verify -- run --case geometry.cad-authored-surface-sweep
```

While the case is `specified`, the runner reports it as not-runnable ("case
status does not declare executable evidence") with a passing exit, so this
evidence package cannot wedge the affected gate. These commands become the
registered target when the implementation slice adds the `[evidence]` table.
The case returns to review, not to relaxation, if any frozen value cannot be
reproduced.
