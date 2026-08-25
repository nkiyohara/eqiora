# Construction-proven planar Geometry v2

This case verifies the narrow scale-independent Geometry foundation for the
accepted rectangle extrusion followed by one circular through-all cut. The
accepted analytic build receipt is the only lineage authority. It projects the
surviving positive-z section into one opaque result face and five opaque result
edges, each bound to the exact graph/build identity and retaining its dimension
in the Rust type.

The caller may atomically group those handles under arbitrary names only when
the mapping covers complete result membership exactly once. Construction
lineage, not coordinates, proximity, provider-local indices, mesh labels, or
an absolute threshold, determines membership. The v2 geometry validates finite
strictly increasing bounds, a finite positive circle radius, and strict positive
side clearance. It stores no `tolerance_m`.

The scale family applies factors `2^-40`, `1`, and `2^40` uniformly. Every
member retains identical name, dimension, and source-lineage membership. The
canonical geometry bytes and digest correctly differ across these metrically
different geometries, while each value replays byte-for-byte through the closed
v2 decoder.

The existing `eqiora.planar-circular-hole-envelope/v1` decoder, 511-byte DFG
witness, digest, and classification-tolerance bits remain exact compatibility
falsifiers. V1 and v2 are never artifact-equal. This case intentionally does
not freeze the new v2 byte count or digest; that exact-artifact admission is a
separate independently derived review input.

Run:

```console
cargo test -p eqiora-geometry --lib \
  cad_authored_result_topology::tests::registered_construction_geometry_v2_evidence
cargo run -p eqiora-verify -- run \
  --case geometry.construction-proven-planar-geometry-v2
```

This is not a generic B-rep, general Boolean or lineage system, coordinate or
provider-index recovery route, imported-mesh classifier, Python naming API,
mesh, solver, or scientific benchmark claim.
