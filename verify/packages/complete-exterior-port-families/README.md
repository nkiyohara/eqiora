# Complete-exterior Port families

This verified case is the conformance root for
[RFC 0041](../../../rfcs/0041-complete-exterior-port-families.md). It verifies
an explicit exact Cartesian complete exterior, statically elaborated
boundary Port, Relation, Activation, and Connection families, nested
forwarding, deterministic identity, complete provenance, and fail-closed
diagnostics.

The registered conformance root checks ordinary flattened Port, Relation,
Activation, and Connection counts; exact member selection; nested pointwise
forwarding; explicit-singular semantic bijection; source, binding,
declaration, and dependency-alias order invariance; transitive provenance;
nominal Connector identity; absence of implicit connections; independent
resource bounds; and twelve source-level falsifiers. Every rejection returns
diagnostics without exposing a partial Model.

Run:

```bash
cargo test --locked -p eqiora --test complete_exterior_port_families
cargo run --locked -p eqiora-verify -- run --case packages.complete-exterior-port-families
```

This case will not claim arbitrary subsets, general arrays or loops, a Kernel
collection, mesh facets, trace spaces, mixed-boundary elasticity numerics,
live coupled execution, Stokes, or FSI.
