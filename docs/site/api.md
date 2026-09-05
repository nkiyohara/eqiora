# API index

Eqiora keeps its public surfaces deliberately smaller than its internal crate
and adapter graph.

## Python

The exact
[generated Python API reference](https://github.com/nkiyohara/eqiora/blob/main/docs/python/api.md)
is produced from the public type stubs shipped in the distribution. This page
is an index, not a second copy of that generated reference. Begin with the
[Python guide](python/index.md) for concepts and then use the generated
reference for exact signatures.

## Rust

The `eqiora` facade is the supported entry point for model construction and
the bounded public control plane. Internal crates remain implementation
boundaries unless a contract explicitly says otherwise.

Until versioned hosted Rust API documentation is part of the release process,
generate documentation from an exact checkout:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --locked -p eqiora --no-deps
```

API presence alone is not a capability claim. Consult
[Capabilities](capabilities.md) and the [verification guide](evidence/index.md)
before relying on a numerical or execution path.
