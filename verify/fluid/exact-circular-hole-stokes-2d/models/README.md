# Model input

The executable graph-authored coherent-SI Model is embedded in
`crates/eqiora/tests/exact_circular_hole_stokes_2d.rs`. It references the exact
geometry digest constructed from the dimensions in `case.toml`; the test then
replays the ordinary `CircularHoleChordalMeshV1` owner, geometry artifact, mesh
artifact, and authored-region correspondence before assembly.

The immutable source-bound mesh consumed by both independent routes is
[`../mesh/mesh.json`](../mesh/mesh.json).
