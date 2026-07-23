# Models

`valid-components.eqi` contains a reusable Component with one required public
Parameter, a symbolic default, and a wrapper that forwards the free value to a
nested instance. `valid-model.eqi` supplies the value only at the selected
Model occurrence. The two `*-permuted.eqi` files retain the same declarations
in another order.

`valid/physical-closure.eqi` closes a child's initially open physical owner
and membership slots in its immediate parent without constructing a Model
occurrence during package admission.
`valid/closed-child-physical-reconnect.eqi` proves that an equivalent outer
fragment is an idempotent statement about the same boundary partition, not a
second Connection membership.

The `invalid/` fixtures isolate definition-time falsifiers. None needs to be
selected or instantiated to fail package preparation. The integration test
generates the 64-Component over-depth chain so its structural pattern remains
visible beside the assertion that owns the compiler limit. Physical fixtures
separately falsify open private and Model endpoints, multiple Relation owners,
one missing Connection membership, and distinct obligations for repeated
instances.
