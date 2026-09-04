# Python guide

Eqiora's Python package is an ergonomic, typed client of the canonical Rust
implementation. It does not implement a second model semantics.

## Read in this order

1. [Modeling and realization](modeling.md) explains immutable declarations,
   spatial support, revisions, and the bounded scalar-elliptic path.
2. [Execution, diagnostics, and arrays](execution-and-arrays.md) explains
   synchronous and awaitable runs, cancellation, NumPy, and DLPack.
3. [Differentiation and framework adapters](differentiation.md) explains the
   framework-neutral program and bounded PyTorch and JAX projections.
4. The [API index](../api.md) links to exact signatures generated from the
   public type stubs shipped in the wheel.

## The boundary to remember

Python exposes a small set of model and execution concepts. Native code owns
canonical validation, compilation, execution, differentiation, and structured
diagnostics. CPU arrays and device exchange use explicit data-plane contracts;
the library does not promise an implicit copy or device transfer.

The
[repository Python guide](https://github.com/nkiyohara/eqiora/blob/main/docs/python/README.md)
has the complete API examples. The [capability matrix](../capabilities.md)
summarizes supported workflows and their maturity.
