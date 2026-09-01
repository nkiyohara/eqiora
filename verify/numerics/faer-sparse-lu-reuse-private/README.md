# Faer sparse-LU reuse private state evidence

This required companion to
[`numerics.faer-sparse-lu-reuse`](../faer-sparse-lu-reuse/README.md) selects one
exact `eqiora-backend-faer` library test. It observes the production-private
phase ledger and shared validation/state-transition seams; its `cfg(test)`
support must not implement a parallel state machine or factorization path.

The oracle freezes the exact p0, p1, and p2 phase traces; the ordered final
counter inventory `(3, 3, 1, 2)`; and the
`p0 -> singular candidate -> p1` retention trace. Injected failures stop at
numeric factorization, candidate solve, solver acceptance, and execution
acceptance. None may expose a candidate identity or reach `state-commit`, and
all retain the last accepted binding and factors.

Six component mutants are precommitted. The complete baseline rejects the
right-hand-side, structure, policy, provider, and complete portable-graph
mismatches. A coefficient mismatch instead takes the live full P2 numeric
rebuild path; complete coefficient equality denies retained-numeric reuse,
while omitting only that equality authorizes the invalid retained reuse. For
every mutant, omitting only the named equality is the sole authorization
change.

Injected stale, foreign, partially constructed, failed, and singular factor
states all reject with `EQ0807` during preflight. They reach no factor solve,
consume no numerical attempt, change no private phase counter, and cannot
replace a committed identity.

Run the paired evidence with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case numerics.faer-sparse-lu-reuse \
  --case numerics.faer-sparse-lu-reuse-private
```

At the frozen preimplementation revision, `sparse_lu_reuse.rs`, its private
test support, and module registration do not exist. This exact library target
is intentionally red until the implementation and integration-owned wiring
are composed. The private case adds no public type, wire, durable ledger,
telemetry surface, general cache, timing claim, or phase-count
order-independence claim.
