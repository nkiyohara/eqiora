# Ordered map composition falsifiers

This companion is required with
[`differentiation.bounded-parameter-study`](../bounded-parameter-study/README.md).
It invokes the production-private evaluator and membership admission with independently
accepted members, then injects concrete composition failures.

The exact aggregate test is
`evaluation_map::tests::registered_composition_oracle_executes_all_private_falsifiers`
in `eqiora-api`. It proves one serial call per reached request position, including duplicates,
rejects foreign Model/Plan/method/input-order/point members, and rejects missing, inserted or
wrongly reordered members. Declared equal-point occurrences remain valid.

Failure stops immediately with original diagnostics and independently inspectable accepted
members. Cancellation names the next unstarted occurrence and leaves subsequent members not
started. Empty completion never polls; final completion wins; a numerical failure raised within
evaluation wins over cancellation raised during that action. An accepted prefix cannot be
constructed into a complete collection.

Structural input and checked retained-numerical-storage admission precede numerical execution.
The resource probes include overflow, an exact admitted byte limit, one byte below the limit,
and inventories larger than the removed historical 64-point bound.

The previous immutable-oracle/precommitment wording described a superseded contract. This
case is migrated in place with the current API, keeps pointwise scientific evidence unchanged,
and makes no new numerical tolerance or solver claim.

```bash
mise run affected -- \
  --case differentiation.bounded-parameter-study \
  --case differentiation.bounded-parameter-study-private
```
