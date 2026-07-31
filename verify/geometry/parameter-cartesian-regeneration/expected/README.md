# Expected evidence

`oracle.json` freezes the complete exact before/after bounds, the coupled x/y
width checksums, and the full and partial-update volumes. These values are
consumed by the integration test; the regeneration implementation does not
produce or tune them.

The case additionally expects one ordinary current Model transaction, one immutable
child revision, unchanged Domain definitions and graph topology, changed Model
and Geometry Identity digests, and an explicit total retained-selection
association.
