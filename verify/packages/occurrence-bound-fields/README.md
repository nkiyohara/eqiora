# Occurrence-bound component Fields

This case verifies the hierarchy-only Field-slot contract from
[RFC 0040](../../../rfcs/0040-occurrence-bound-field-slots.md). A Component may
require an existing continuum Field, use it in ordinary typed Relations, and
forward the same exact Field through a nested Component occurrence. The slot
is eliminated before the Semantic Kernel.

The local fixture binds one invariant scalar Field and one two-dimensional
spatial-vector Field through a wrapper. A passing implementation produces only
the two Model-owned Fields. Both expanded Relations refer to those exact Field
identities, and every expanded declaration retains the Field-binding source
spans in occurrence provenance.

The exact-package path uses the same definitions through locked offline
resolution. Dependency-alias, declaration, and binding permutations must not
change canonical flattened Model meaning. Invalid definitions and occurrence
bindings must fail before a Model, Transaction, graph mutation, or numerical
allocation is exposed.

Run:

```bash
cargo test --locked -p eqiora --test occurrence_bound_fields
cargo run --locked -p eqiora-verify -- run --case packages.occurrence-bound-fields
```

This case does not claim a discrete function-space binding, public mutable
Field member, PureOperator, mesh array, boundary partition, physical Port,
elasticity solve, fluid solve, or FSI.

