# Planar rectangle Cartesian common Mesh

This case verifies exact, tolerance-free production of one structured
Cartesian common Mesh from `PlanarRectangleV2`. Independent tensor-product
enumeration establishes every canonical coordinate, ordered cell/facet
connectivity, exact source-edge facet ID, boundary parent incidence, local
side ordinal, and orientation for the fixed `2 x 3` witness. The independently
partitioned boundary and interior inventories are disjoint and cover every
facet.

The correspondence stores only direct source-edge facet sets. Replay binds
the exact Geometry and canonical Mesh, then reconstructs completeness,
exclusivity, one-parent boundary incidence, local side, and orientation from
topology. Side-set swaps; facet-ID, connectivity, local-ordinal, and
orientation mutations; non-rectangle Geometry; zero/overflow/substituted
policy; and foreign Geometry, Mesh, correspondence, or lineage resources
reject.

This is not a coordinate classifier, generic planar-bounds mesher, Model or
physics admission, Q1/TPFA finalization, or scientific-output claim.

Run it with:

```bash
cargo run --locked -p eqiora-verify -- run \
  --case geometry.planar-rectangle-cartesian-common-mesh
```
