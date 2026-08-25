# Planar circular-hole Geometry v2 foundation

This case verifies one provenance-neutral, scale-independent exact Geometry v2
content family: finite axis-aligned rectangle bounds, one finite positive
circle with strict positive side clearance, and complete named membership for
five edges and one face. Its closed wire contains no classification tolerance,
provider identity, authored graph digest, build receipt, or lineage.

The accepted authored-CAD graph is one separate producer route. Its analytic
build projects retained and created construction lineage into crate-private,
dimension-carrying result handles bound to the exact graph identity. Atomic
naming accepts only complete, exactly-once, homogeneous membership. This proves
how that producer obtains Geometry content; it does not turn provenance into
persisted Geometry meaning.

The scale family applies factors `2^-40`, `1`, and `2^40` uniformly. Every
member retains identical name and dimension membership, while each metrically
different Geometry has its own canonical bytes and digest and replays through
the closed v2 decoder. Unknown and duplicate fields, tolerance/classification
fields, reordered keys or members, and equivalent noncanonical numeric spelling
reject.

The existing `eqiora.planar-circular-hole-envelope/v1` decoder, 511-byte DFG
witness, digest, and classification-tolerance semantics remain unchanged. V1
and v2 are never artifact-equal. Exact bytes and digest for the renamed,
provenance-neutral v2 wire remain pending a fresh independent derivation.

Run:

```console
cargo test -p eqiora-geometry --lib \
  cad_authored_result_topology::tests::registered_planar_circular_hole_geometry_v2_evidence
cargo run -p eqiora-verify -- run \
  --case geometry.planar-circular-hole-geometry-v2
```

This foundation exposes no public result-topology naming API. It is not a
source-owned mesh correspondence, imported-mesh classifier, generic B-rep,
general Boolean/lineage system, Python workflow, mesh, solver, or scientific
benchmark claim.
