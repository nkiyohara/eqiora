# Provider-neutral authored rectangle extrusion

This case proves one immutable authored CAD graph with exactly four dependent
operations: an XY sketch plane, a rectangle profile constrained by
construction, one face closed from that loop, and a positive-z extrusion.  The
graph owns exact analytic geometry and six provenance selections; no provider
object, output-coordinate query, or mesh entity participates in identity.

Two non-implementing agents derived the witness independently before
implementation.  The analytic route closes the extents, volume, face
centroids, areas, normals, and total surface area.  The oriented-polyhedron
route closes outward cycles, two-use edge incidence, Euler characteristic, and
signed-tetrahedral volume.  Both yield one 8-vertex, 12-edge, 6-face closed
shell with volume `60 m^3` and area `94 m^2`.

A third independent construction freezes the compact 731-byte graph encoding
and domain-separated digest
`919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36`.
Changing only the requested modeling tolerance changes that identity while
leaving every geometric observation equal.  A handle from either the
tolerance-only or depth-only predecessor rejects before selection lookup.

The existing bounded STEP/box workflow consumes this graph as the sole owner
of its rectangle, face, and extrusion meaning.  That temporary workflow still
owns import and intersection; this case does not generalize or delete it.

Run:

```bash
cargo test -p eqiora-geometry --test cad_authored_rectangle_extrusion
cargo run -p eqiora-verify -- run --case geometry.cad-authored-rectangle-extrusion
```

This is not a general sketch solver, profile or feature DAG, import or Boolean
contract, circle or curved-geometry model, B-rep, healing or selection-
rebinding system, mesh, solver, provider registry, or Python/Studio surface.
