# Frozen Gmsh 4.15.2 observations

The independent evidence owner projected the pre-existing accepted chordal
`PlanarRegion` to a Built-in Gmsh recipe, with hole traversal before outer
traversal, shortest-roundtrip binary64 coordinates, no point mesh size,
Algorithm 6, linear elements, ASCII MSH 4.1, `RandomFactor=0`, `SaveAll`, and
one thread.

Official Linux64 and PyPI Gmsh 4.15.2 produced byte-identical MSH output on an
immediate clean replay. The pre-existing Eqiora importer and mesh envelope
then produced:

- 662 vertices, 1,210 triangles, and 114 boundary edges;
- `cylinder=50`, `inlet=14`, `outlet=2`, `walls=48`, and `fluid=1210`;
- minimum affine-map mean ratio `0.5236522686855336`;
- minimum signed measure scale `2.6093038450074273e-5`;
- 42,388 canonical bytes;
- raw canonical SHA-256
  `9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2`;
- domain-separated Mesh digest
  `5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b`;
- C-order binary64 coordinate-buffer SHA-256
  `42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d`;
  and
- C-order native little-endian u32 triangle-buffer SHA-256
  `05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642`.

A separate Python MSH decoder recomputed the accepted `AffineMapQuality`
formula from node and local-cell order and agreed exactly in binary64. These
values describe the exact one-thread Linux witness, not raw-MSH or
cross-platform portability.
