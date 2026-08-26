# Common Mesh production lineage v1

This case verifies one canonical artifact that records which exact provider
occurrence produced an accepted common Mesh. It remains separate from
provider-neutral Geometry, Mesh, and Geometry-to-Mesh correspondence identity.

The current closed providers are the exact external Gmsh CLI 4.15.2 adapter
and the deterministic in-process planar circular-hole reference producer v1.
The artifact retains provider identity and version, every effective planar
policy value, and exact Geometry, Mesh, and correspondence digests.

The registered test round-trips canonical bytes and rejects provider/version,
policy, and bound-resource substitution. It freezes no numerical output,
tolerance, mesh inventory, or provider performance. It does not claim raw
imports, Cartesian meshing, a provider registry, physics, Stokes, Gallery, or
scientific validation.

Run it with:

```bash
cargo run --locked -p eqiora-verify -- run \
  --case artifacts.mesh-production-lineage-v1
```
