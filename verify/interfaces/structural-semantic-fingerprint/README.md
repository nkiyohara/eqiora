# Structural semantic fingerprint verification

This case verifies one versioned, non-authoritative identity for comparing
accepted Semantic Models built through independent authoring routes. The
generation-v3 projection removes occurrence ULIDs and source presentation but
retains the complete admitted kernel graph, nominal identity relationships,
current values, expression structure, physical connections, and Model boundary
membership. Geometry-region digests, entity-set names, geometry-boundary names,
and Cartesian coordinate source kinds are retained; nominal dependencies and
topology remain in the graph edges.

The positive fixtures compile independently, use distinct names and declaration
orders, and cross source and native Rust authoring. Their exact Model artifact
references remain distinct while their
structural fingerprints and bounded byte-confirmed comparison agree. Exact
artifact replay retains both identities for their respective purposes.

Falsifiers change a Parameter value, residual operator, expression reference,
nominal Domain sharing, dimension, shape/frame, spatial support, periodic clock,
or Model boundary membership. Signed zero is the sole scalar normalization in
this slice. A symmetric graph with an exhausted exact-labeling budget returns a
diagnostic; it never falls back to occurrence order or an approximate hash.

Run:

```bash
cargo test -p eqiora --test structural_semantic_fingerprint
cargo run -p eqiora-verify -- run --case interfaces.structural-semantic-fingerprint
```

The fingerprint is comparison evidence. It is not a Model artifact identity,
execution/replay input, mutation precondition, provenance reference, persistent
entity identity, semantic merge, or proof of mathematical equivalence.
