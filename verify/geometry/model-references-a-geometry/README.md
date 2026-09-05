# Geometry-referencing Semantic Model verification

This case verifies the graph-authored Semantic Model boundary for Domains that
name an authored geometry. A `GeometryRegion` carries the full geometry digest
and one entity-set name. A `GeometryBoundary` carries its own entity-set name
and has exactly one `BoundaryOf` parent, which must be a geometry region.

The positive fixtures prove a region alone and a region with a boundary,
deterministic current Model and Transaction round-trip, current vocabulary
inheritance, and the current structural fingerprint. Independently allocated
occurrence IDs change exact Model identity while preserving the structural
fingerprint.

Falsifiers change the digest's final byte, either entity-set name, or the
boundary's parent among two distinct regions. Missing, multiple, Cartesian, or
wrong-kind parents fail whole-Model validation. Malformed digest text fails as
`EQ0901`.

Fields, Relations, and boundary-physical Ports cannot yet use geometry Domains
as spatial support. The Model lacks the admitted geometry artifact needed to
prove that an entity set exists and has the required dimension, so those uses
fail as `EQ0302` instead of guessing a dimension or treating the quantity as
global.

Run:

```bash
cargo test -p eqiora --test geometry_referencing_model
cargo run -p eqiora-verify -- run --case geometry.model-references-a-geometry
```

This case does not claim source or native-draft syntax, artifact admission,
entity-set inspection, geometry-to-mesh correspondence, realization,
lowering, curves, general 3D geometry, or a change to `box(...)`.
