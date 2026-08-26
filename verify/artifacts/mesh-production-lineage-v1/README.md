# Common Mesh production lineage v1

This case verifies one canonical artifact that records which exact provider
occurrence produced an accepted common Mesh. It remains separate from
provider-neutral Geometry, Mesh, and Geometry-to-Mesh correspondence identity.

The closed providers are exact Gmsh CLI 4.15.2, the in-process planar
circular-hole reference producer v1, and structured Cartesian v1. The
artifact retains provider identity and version, an explicitly tagged closed
planar-quality or Cartesian-cell policy, and exact Geometry, Mesh, and
correspondence digests.

The registered test round-trips canonical bytes and rejects provider/version,
every retained policy field, Cartesian provider/policy mismatch, and
bound-resource substitution; malformed, unknown, and semantically valid but
noncanonical encodings also reject. It freezes no numerical output,
tolerance, mesh inventory, or provider performance. It does not claim raw
imports, a provider registry, physics, Stokes, Gallery, or
scientific validation.

Run it with:

```bash
cargo run --locked -p eqiora-verify -- run \
  --case artifacts.mesh-production-lineage-v1
```
