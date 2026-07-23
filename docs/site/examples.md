# Examples

Use examples to learn the shape of the public API. Use registered evidence to
decide whether a capability exists for your exact problem.

## Orientation

The
[quickstart example](https://github.com/nkiyohara/eqiora/blob/main/crates/eqiora/examples/quickstart.rs)
constructs and commits a small canonical model through the public Rust facade.
The [example index](https://github.com/nkiyohara/eqiora/blob/main/examples/README.md)
keeps orientation paths intentionally small.

## Numerical and physical slices

The repository's verified cases include bounded paths for:

- Cartesian and simplicial finite-element and finite-volume problems;
- time integration, DAE initialization, events, and reset;
- packages and conserving physical networks;
- fluid, solid, and fixed-reference or moving-mesh FSI;
- CPU, MPI, and CUDA execution contracts;
- Python modeling, arrays, differentiation, and framework adapters.

Those bullets are navigation categories, not blanket support claims. Choose a
case from the [evidence catalog](evidence/index.md), then read its manifest and
README for its exact claim and nonclaims.

## Build a new vertical slice

Contributors should follow the repository's
[vertical-slice definition of done](https://github.com/nkiyohara/eqiora/blob/main/docs/development/vertical-slice-development.md):
typed contract, execution path, falsifier, registered evidence, capability
matrix update, and verification.
