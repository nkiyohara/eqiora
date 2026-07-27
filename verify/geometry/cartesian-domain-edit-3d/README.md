# Exact Cartesian Domain edit in 3D

This case proves one deliberately narrow geometry-authoring action. A current
Model wire v6 document with exactly one three-dimensional Cartesian body
changes one axis interval through the ordinary versioned transaction path.
Preview is immutable and content-bound; commit replays the exact transaction
against the same Model digest and graph revision.

The oracle is not produced by the edit implementation. `models/target.eqi` is
compiled independently, compared by the alpha-normalized structural
fingerprint, and used to evaluate the expected volume. The test separately
checks the exact interval, retained body and boundary Domain IDs, all six
Cartesian boundary roles, Model membership, and every edge incident to the
body.

The child has a new Model digest and a separately derived Geometry Identity
digest. Retention is admitted only through the existing explicit total
one-to-one geometry-revision association, which replays both geometry/mesh
correspondences. An edge-only wire mutant proves that invalid Model replay
cannot enter Geometry Identity. A second mutant removes that boundary
occurrence as well, producing a replayable Model with five roles; Geometry
Identity rejects the incomplete Cartesian exterior.

Fail-closed cases cover a structurally equivalent same-revision sibling with a
different exact digest, a stale child revision, no-op and out-of-axis edits, a
boundary passed as the body target, non-finite or reversed typed bounds, an
older Model codec, a two-dimensional body, and multiple Cartesian bodies. Two
equal previews have identical plan, transaction, and child identities; a
different interval changes all three. The immutable base bytes and digest
remain unchanged.

Run:

```bash
cargo test --locked -p eqiora --test cartesian_domain_edit
cargo run --locked -p eqiora-verify -- run --case geometry.cartesian-domain-edit-3d
```

This case does not claim source rewriting, parameter-driven geometry,
multi-axis edits, dimensions other than 3D, multiple bodies, topology changes,
CAD feature history, mesh regeneration as a product API, ALE, optimization, or
shape sensitivity.
