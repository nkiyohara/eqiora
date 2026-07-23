# Python control-plane verification

This case exercises the private PyO3 module as an in-process Python client and
then crosses back through the public Rust facade. A source-compiled current-v6
`Model` must replay exactly: canonical bytes, semantic digest, typed Model ID,
and revision identity are preserved rather than recreated by Python.

The same opaque `Model` previews one exact-base scalar value edit and commits
it atomically into a new immutable child. The base remains byte-for-byte
unchanged, the child advances the revision, and replaying the stale edit
against that child fails with structured diagnostics. Equal graph-local edits
prepared on divergent child artifacts retain distinct exact-plan identities.

Falsifiers also require malformed exact wire, invalid source, invalid run
policy, and a deliberately panicking boundary operation to become the stable
compatibility, validation, execution, and internal Python exception families.
The guarded-thread panic hook must not emit the payload or Rust location, and
the Python diagnostic is `EQ0002`. Unrelated threads retain the process's
previous panic hook.
Initializing the native module must not import NumPy, PyTorch, or JAX, keeping
control-plane use independent of optional data and framework adapters.

This evidence does not claim independent compilations have equal IDs or
digests. Native modeling vocabulary belongs to
`language.native-modeling` and its Python-specific follow-up. Async execution,
cancellation, progress, array exchange, DLPack, and framework integration are
also outside this case.

Run:

```bash
cargo test --locked -p eqiora-python --test python_control_plane
cargo run --locked -p eqiora-verify -- run --case interfaces.python-control-plane
```
