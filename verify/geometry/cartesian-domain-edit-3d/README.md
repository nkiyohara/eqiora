# Exact Cartesian Domain edit in 3D

This case proves one deliberately narrow geometry-authoring action. A current
Model wire v6 document with exactly one three-dimensional Cartesian body
changes the x- and z-axis intervals as one non-empty canonical axis-keyed set
through the ordinary versioned transaction path. Preview is immutable and
content-bound; commit replays the exact transaction against the same Model
digest and graph revision. The transaction removes and redefines the body
once, then reconnects every incident edge once.

The oracle is not produced by the edit implementation. `models/target.eqi` is
compiled independently, compared by the alpha-normalized structural
fingerprint, and used to evaluate the expected `1.8 m^3` volume. The test
separately checks both changed intervals, the unchanged y interval, retained
body and boundary Domain IDs, all six
Cartesian boundary roles, Model membership, and every edge incident to the
body.

The child has a new Model digest and a separately derived Geometry Identity
digest. Retention is admitted only through the existing explicit total
one-to-one geometry-revision association, which replays both geometry/mesh
correspondences. An edge-only wire mutant proves that invalid Model replay
cannot enter Geometry Identity. A second mutant removes that boundary
occurrence as well, producing a replayable Model with five roles; Geometry
Identity rejects the incomplete Cartesian exterior.

Caller permutations of the same two edits produce an equal canonical plan,
byte-identical transaction and child Model, and equal digests. A cardinality-one
request still travels through this same contract. A partial-application mutant
that changes only x disagrees with the independent target and volume oracle.

Fail-closed cases cover an empty set, duplicate and out-of-range axes, any
no-op member mixed with an otherwise valid member, a structurally equivalent
same-revision sibling with a different exact digest, a stale child revision, a
boundary passed as the body target, non-finite or reversed typed bounds, an
older Model codec, a two-dimensional body, and multiple Cartesian bodies. An
edited-child mutant missing one incident `BoundaryOf` edge also fails closed.
Every rejected preview leaves the immutable base bytes and digest unchanged.

Run:

```bash
cargo test --locked -p eqiora --test cartesian_domain_edit
cargo run --locked -p eqiora-verify -- run --case geometry.cartesian-domain-edit-3d
```

This case does not claim source rewriting, parameter-driven geometry,
dimensions other than 3D, multiple bodies, topology changes, CAD feature
history, mesh regeneration as a product API, ALE, optimization, or shape
sensitivity.
