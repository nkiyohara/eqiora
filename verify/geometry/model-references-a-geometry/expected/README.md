# Expected evidence

- A geometry region without a boundary validates.
- A geometry boundary validates only with exactly one geometry-region parent.
- Model and Transaction v7 replay preserves exact bytes and digests.
- Model and Transaction v6 reject the geometry vocabulary.
- Structural fingerprint generation v2 preserves geometry digest, region and
  boundary entity-set names, and boundary-parent topology.
- Fresh occurrence IDs change exact Model identity but not the structural
  fingerprint.
- Geometry-backed Field, Relation, and boundary-Port support fails with
  `EQ0302` until artifact admission can prove entity-set existence and
  dimension.
