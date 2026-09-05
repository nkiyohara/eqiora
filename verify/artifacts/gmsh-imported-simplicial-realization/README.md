# Gmsh-imported simplicial Realization verification

This case imports ASCII and little-endian 64-bit binary MSH 4.1 fixtures
emitted by Gmsh 4.15.2 from the same checked-in `.geo` source. A bounded,
isolated adapter accepts only the declared affine-simplex subset and
reconstructs both representations through Eqiora's shared `SimplicialMesh`
invariants. Their accepted mesh, canonical bytes, and fixed artifact digest
are identical. Only that artifact's content digest is bound into the
Realization.

The four-triangle mesh has one free degree of freedom. Its independently
derived P1 value is exactly `1 / 12`, and integrated source plus boundary
reaction balance to roundoff. The end-to-end test imports through the public
`eqiora::io::gmsh` facade without an optional feature and rejects every truncated
prefix of the official binary fixture. Unit evidence additionally covers
sparse tags, multiple blocks, four- and eight-byte `size_t`, both endian
orders, every truncated prefix of all four binary representations, exact and
extreme binary and ASCII count budgets, aggregate decoded-byte/work exhaustion,
the independent ignored-element boundary, a structural 3D tetrahedron,
malformed input, resource excess, unknown references, embedded coordinates,
unsupported cells, orientation, and quality rejection. Decoded bytes are a
conservative logical account of known simultaneous structures, not exact
allocator RSS. Token-dense ASCII format and entity records are parsed without
token-vector allocation and fail closed under a small decoded budget.

This case does not assign meaning to Gmsh physical groups, result fields,
paths, entity tags, or importer state. It does not claim partitioning,
periodic links, embedded manifolds, curved/high-order or mixed cells, global
non-overlap, adaptivity, source provenance, or export.

Run:

```bash
cargo test -p eqiora-io-gmsh
cargo test -p eqiora --test gmsh_imported_simplicial_realization
cargo run -p eqiora-verify -- run --case artifacts.gmsh-imported-simplicial-realization
```
