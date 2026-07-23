# Residual-native checkpoint and restart lineage

This case records one accepted residual-native point as a content-addressed
checkpoint, independently replays its residual from canonical Operator IR, and
links a parent run to a restarted child run without creating a digest cycle:

```text
parent run --output--> checkpoint
                         |
                         v
               Provided initial data --> child run
                         \______________/
                         restart manifest
```

The checkpoint owns model time, canonical state and derivative order, replayed
residual norm, and acceptance tolerance. It deliberately does not own a run
identity. The separate restart manifest proves that the parent emitted the
checkpoint, the child plan starts at checkpoint time, and both child initial
digests are the exact `Provided` artifact derived from that checkpoint.

The test compares the restarted two-step reference trajectory with an
uninterrupted two-step solve. It also rejects checkpoint value drift, an
over-limit dimension, a missing parent output edge, child start-time drift,
and a direct parent/child cycle. Canonical JSON containing a nontrivial finite
`f64` must round-trip byte-for-byte and retain its digest.

This is semantic restart from an accepted `(t, y, y_dot)` point. It does not
claim preservation of an adaptive controller, BDF/Nordsieck history, Newton or
linear-solver state, bitwise backend continuation, multi-step adjoints,
hybrid-event lineage, or a backend-native durable checkpoint payload.

Run:

```bash
cargo test -p eqiora --test implicit_time_restart_lineage
cargo run -p eqiora-verify -- run --case artifacts.implicit-time-restart-lineage
```

See [RFC 0014](../../../rfcs/0014-production-time-backend-contracts.md).
