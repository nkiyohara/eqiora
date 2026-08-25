# Revision-bound Python Geometry selection

This case projects one existing Rust-owned `NamedEntitySet` as an immutable
installed-Python `GeometrySelection` retaining the exact owning Geometry
digest, canonical name, and topological dimension.

The ordinary path resolves `inlet` once from the accepted exact circular-hole
`Geometry` and passes that handle to the existing `Mesh` correspondence query.
It returns the same membership count as the bounded string compatibility path.
An unknown name rejects before a handle is returned, and a handle from the
same authored shape with different role identity rejects against the Mesh as a
foreign or stale Geometry revision.

This is a non-persisted reference-like projection. It reuses the canonical
Geometry, named entity set, accepted Mesh, and Geometry-to-Mesh correspondence
owners indexed in `case.toml`; it defines no second selection wire, registry,
entity graph, or membership semantics. It adds no scientific expected value,
tolerance, or falsifier.

The boundary is the accepted planar rectangle-with-circular-hole Geometry and
the existing common Mesh correspondence. Arbitrary CAD topology, mutable or
composed selections, physics and boundary-condition authoring, visualization
selection, persistence, and scientific results remain outside the claim.

Run the registered installed-wheel evidence in the existing exact-Gmsh
environment:

```bash
mise run affected -- --case interfaces.python-geometry-selection-handle
```
