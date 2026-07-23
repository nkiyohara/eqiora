# Fixed-reference interface geometry identity in 2D

This case verifies the narrow geometry-to-mesh identity seam needed by a later
fluid--structure interaction Realization. Two adjacent Cartesian semantic
bodies, fluid and solid, retain distinct interface Boundary Domains joined by
one ordinary conserving Connection.

One content-addressed two-dimensional affine-triangle mesh is partitioned
into disjoint fluid and solid cell subsets. Each semantic body and boundary
maps first to an exact entity in one geometry revision and then to the exact
cell or facet set in that mesh revision. The two interface boundaries map to
the same complete facet set, but orientation is derived independently from
oriented incidence relative to each parent cell subset. The resulting outward
orientations must be opposite; no normal sign is accepted from the caller.

The artifact chain binds exact Model, geometry, correspondence, and mesh
digests. Reordering selected body inputs or explicit association candidates
cannot change canonical bytes.
One geometry-capable Semantic Model is also decoded explicitly as Model v1,
v2, v3, and v4. All four wires replay through one sealed geometry boundary and
derive the same Domain roles and mesh memberships, while their wire-domain
digests remain distinct and cannot substitute for one another.

The geometry producer owns one coherent-SI classification precision. It is
part of Geometry Identity, and correspondence reuses it without accepting a
second mesh-local tolerance. A deliberately displaced interface proves that a
tight geometry revision rejects membership while a looser, differently
identified revision admits it.

Source and target Domain IDs may differ; cross-revision retention is accepted
only from an explicit total one-to-one geometry-entity successor proof.
Missing, split, merged, or ambiguous selections fail closed.

Run:

```bash
cargo test --locked -p eqiora-artifact --test geometry_identity_fsi_2d
cargo run --locked -p eqiora-verify -- run --case geometry.fixed-reference-interface-identity-2d
```

This case does not claim wire auto-detection or migration, CAD import, Gmsh
physical-group meaning, topology-name heuristics, CAD healing tolerance, trace
transfer, ALE, moving geometry, or an FSI solve.
