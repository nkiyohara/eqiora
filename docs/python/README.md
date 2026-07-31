# Python SDK guide

Eqiora's Python package is a typed client of the canonical Rust
implementation. Python supplies ergonomic declarations and array/framework
adapters; model meaning, validation, execution, differentiation, and evidence
remain in the shared native implementation.

Start with the
[package README](https://github.com/nkiyohara/eqiora/blob/main/bindings/python/README.md),
then use the focused guides:

- [Modeling and realization](modeling.md) covers native declarations,
  spatial support, immutable revisions, and the bounded scalar-elliptic path.
- [Execution, diagnostics, and arrays](execution-and-arrays.md) covers
  synchronous and awaitable runs, cancellation, errors, NumPy, and DLPack.
- [Differentiation and framework adapters](differentiation.md) covers the
  framework-neutral program and the bounded PyTorch and JAX projections.
- [Modeling and realization](modeling.md#exact-cylinder-pressure-still) also
  shows the accepted exact-cylinder, mixed-boundary structural, and
  fixed-reference FSI Result/Matplotlib workflows.
- [Generated API reference](api.md) is derived only from the public type
  stubs shipped in the distribution.

The exact verified capability boundary and important nonclaims remain
authoritative in the
[capability matrix](https://github.com/nkiyohara/eqiora/blob/main/docs/capability-matrix.md).
These guides explain how to use accepted capabilities; they do not widen
them.
