# Interval Cartesian common Mesh

This case verifies exact, tolerance-free production of one structured
Cartesian common Mesh from an interval Geometry. Independent enumeration
establishes all four coordinates, all three ordered segment cells, both exact
endpoint memberships, their sole parent incidences, local-side ordinals, and
identity orientations.

Replay binds the exact Geometry, canonical Mesh, direct correspondence,
dimension-parametric v2 Cartesian policy, and production lineage. Geometry,
policy, endpoint membership, and foreign-resource mutations reject.

This is not Model or physics admission, Q1 or TPFA finalization, a PDE result,
convergence, or performance evidence.

Run it with:

```bash
cargo run --locked -p eqiora-verify -- run \
  --case geometry.interval-cartesian-common-mesh
```
