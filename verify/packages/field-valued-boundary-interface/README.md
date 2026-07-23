# Field-valued boundary interface

This case closes one exact-package, mesh-independent two-dimensional physical
interface. Two component occurrences bind the same generic Connector to the
upper face of one Cartesian parent and the lower face of another. The
boundaries denote the same point set while retaining distinct parent identity.

The ordinary package compiler must produce one specialized `[2]`
`SpatialCartesian` Connector, two boundary Ports, and one maximal conserving
Connection. The accepted semantic junction is typed once and lowered to four
scalar Operator rows: two trace-continuity components followed by two
outward-flux-balance components.

Analytic observations measure trace defect, outward flux imbalance, and the
interface power defect implied by those two laws. Each observation has a
nonzero falsifier. Source variants exercise package-alias, declaration, binding, and
connection-member invariance. Noncoincident geometry and wrong parent binding
fail before a packaged Model is exposed. The complete accepted Model replays
through explicit wire v3; v1 and v2 reject the same vocabulary.

A second root routes each public field-valued Port through one transparent
wrapper. The compiler eliminates those ownerless exposures, while a versioned
catalog preserves their exact occurrence identities, one-Port interior cuts,
nominal connector, distinct boundary supports, and complete package source
origins. Artifact replay rejects a support substituted from the coincident
peer boundary; pointwise values and mesh transfer remain separate concerns.

Run:

```bash
cargo test --locked -p eqiora --test field_valued_boundary_interface
cargo run -p eqiora-verify -- run --case packages.field-valued-boundary-interface
```

The claim does not include a mesh, numerical trace space, transfer map,
field-valued result storage, Stokes or elasticity discretization, or an FSI
solve. The durable projection claim is limited to identity, exact cut,
connector/support, provenance, and replay.
