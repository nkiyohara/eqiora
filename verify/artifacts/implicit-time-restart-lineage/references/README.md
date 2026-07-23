# Reference provenance

The reference is the same deterministic residual-native implicit-Euler oracle
run in two ways:

- one uninterrupted interval from `t = 0.0` to `t = 0.2`; and
- a parent interval ending at `t = 0.1`, followed by a semantic restart from
  its independently replayed accepted pair.

The comparison isolates lineage and restart semantics from differences between
numerical methods. Residual acceptance is recomputed from the canonical scalar
Operator IR and is not trusted from serialized backend evidence.
