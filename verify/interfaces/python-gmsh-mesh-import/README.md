# Provenance-bound installed-Python Gmsh Mesh import

This case composes already accepted owners into one narrow user path:

```text
exact circular-hole Geometry + explicit GmshImport policy + complete MSH 4.1 bytes
  -> bounded Gmsh simplex importer
  -> common simplicial Mesh
  -> complete Geometry-to-Mesh correspondence
  -> accepted chordal Realization
  -> ExternalImportManifestV1
  -> existing immutable eqiora.meshing.Mesh
  -> existing steady-Stokes Plan boundary
```

`eqiora.meshing.import_gmsh` takes bytes, not a path, and does not launch
Gmsh. `GmshImport` supplies every boundary-realization and quality choice.
The returned object is the same `Mesh` used by generated paths. Its
`external_import_manifest_bytes` and `external_import_manifest_digest`
properties retain the canonical source-to-normalized-array-to-Mesh assertion;
generated and Cartesian Meshes return `None` for both properties.

The manifest names adapter `eqiora.gmsh` at the exact Cargo release version,
an empty native runtime stack because the owned decoder is pure Rust, the raw
complete source SHA-256, adapter-relative MSH mesh/Nodes/Elements selectors,
the two normalized array identities, and the accepted common Mesh identity.
It remains an assertion rather than a verified persisted-replay handle; this
slice does not strengthen `ExternalImportManifestV1` semantics.

The installed-wheel positive path imports the existing accepted cylinder
source, checks the manifest relationally against the supplied bytes and
returned Mesh, preserves read-only arrays and correspondence-derived named
selections, and feeds the accepted exact witness into the existing
steady-Stokes Plan resolver. Focused failures reject malformed source bytes
and a geometrically foreign exact source before returning a Mesh. Decoder,
provenance, correspondence, exact-mesh, and Stokes scientific oracles remain
owned by the cases referenced in `case.toml`; this case adds no expected
scientific value, tolerance, byte baseline, or falsifier.

The boundary is affine Tri3 in two coherent-SI dimensions for the accepted
exact rectangle-with-circular-hole Geometry. There is no path API, field
import, arbitrary Geometry matching, Tet4/3D, mixed or curved cell support,
repair, renumbering equivalence, verified persisted replay, Studio surface,
performance claim, or new scientific result.

Run the registered installed-wheel evidence in the existing exact-Gmsh
environment:

```bash
mise run affected -- --case interfaces.python-gmsh-mesh-import
```
