# Reference provenance

This pre-implementation evidence was derived from exact base commit
`934493bcb487c1753fb4b3ddffaab88d7150aa7d` without reading or consuming the
Gmsh-provider implementation.

It reuses, without changing:

- the accepted exact circular-hole source and deterministic 50-chord
  `PlanarRegion` owner;
- the bounded ASCII MSH 4.1 importer;
- `SimplicialMesh` orientation and `AffineMapQuality`; and
- `SimplicialMeshEnvelopeV1` canonical encoding and digest framing.

The evidence owner independently downloaded the official Linux64 Gmsh 4.15.2
archive (SHA-256
`6c62116e072db29fd1f701fdb9d3d34b46ed5373545063e177b965a008274745`)
and independently installed the PyPI 4.15.2 distribution. Both produced the
same MSH bytes from the owner-derived GEO, and a clean rerun reproduced them.

The GEO is derived rather than checked in because the accepted region remains
the authority. It emits its canonical hole traversal before the outer
traversal, uses shortest-roundtrip binary64 coordinate spelling, and supplies
no point mesh-size value. A recipe that asks Gmsh to reconstruct the circle
with its own trigonometric expressions is not the same input and cannot supply
expected values.

The local GEO and MSH hashes are derivation receipts for this Linux run. The
public test freezes the resulting mesh envelope and NumPy buffers, not a claim
that raw MSH bytes are portable across platforms or Gmsh builds.
