# Eqiora-authored structural mesh fixtures

`primary.geo` is one parameterized straight-edge DFG-shaped recipe. The three
checked-in MSH 4.1 outputs use `(segments, mesh_size)` equal to `(8, 0.12)`,
`(16, 0.07)`, and `(32, 0.04)`. `bias.geo` realizes the same 32-chord Geometry
with a different Gmsh algorithm. Neither recipe declares a physical group.

The fixtures were generated with the Linux64-sdk Gmsh 4.13.1 wheel, build date
2024-05-24, one thread, ASCII MSH 4.1, and `Mesh.RandomFactor = 0`. The complete
`libgmsh.so.4.13` receipt is
`0a923f7069d3ab91d142ed7afcc9e933144c88034e2119067146d2dd87cb4cac`.
Recipe and output hashes are authoritative in `expected/family-identities.toml`.
Only Gmsh-emitted trailing ASCII blanks are removed. An immediate clean
regeneration followed by that normalization produced byte-identical outputs.

These files contain only coordinates, topology, and Gmsh entity syntax. They
contain no canonical source-owner bytes, imported boundary meaning, source
page, third-party mesh, or production-resolution claim. Boundary membership
is reconstructed only from the accepted authored Geometry correspondence.
