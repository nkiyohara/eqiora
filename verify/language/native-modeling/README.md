# Native model construction verification

This case defines the scalar decay model twice: once as Eqiora source and once
from client-neutral immutable Rust declarations. It compares their complete
accepted graph through the shared generation-v2 structural semantic fingerprint
and compares the reference trajectories. Their fresh exact occurrence/artifact
identities intentionally remain distinct.

The native model also crosses the versioned transaction boundary, commits
atomically, and round-trips the canonical model artifact. Falsifying paths use
a same-named but omitted Field and a dimensionally invalid Relation. They must
return declaration `graph_path` diagnostics without a fictional source span or
an observable partial model.

This verifies the shared `ModelDraft` and compiler/application path. The PyO3
adapter is covered separately by installed-wheel tests on every claimed CPython
version. This case does not claim spatial, vector/tensor, clocked, event,
connection, component, or Python-callback construction.

Run:

```bash
cargo test -p eqiora --test native_modeling
cargo run -p eqiora-verify -- run --case language.native-modeling
```
