# Materialized direct-output differentiation

This case binds one accepted generic relation/output pair to its exact
canonical CSR coefficient source and sends the separately derived derivative
right-hand side through faer `SparseLu` in normal and transposed orientation.
The canonical source retains the distinct structural-zero primal right-hand
side throughout both calls.

The ordinary positive path loads and digest-checks the existing exact-rational
5x5 sparse-LU fixture. For `R(w,p) = A w - b p` and
`J(w,p) = b^T w` at `(w,p)=(0,0)`, forward mode recovers the fixture's `x`
from `A x = b`, adjoint mode recovers its `y` from `A^T y = b`, and the output
tangent and Parameter gradient are evaluated as the fixture-derived
projections `b^T x` and `b^T y`. No reduced scalar expectation or additional
tolerance is stored.

Only after both ordinary calls pass, the same executable package checks three
plausible failures:

- a provider that factors the original `A` but reads primal `q` instead of
  derivative `b` reaches factor-and-solve and fails backend true-residual
  acceptance against `b`;
- a transposed route that reports `Transposed` but returns normal solution `x`
  fails the accepted relation's independent VJP replay; and
- a valid same-shape cyclic-row canonical source passes its own direct solve
  but fails the accepted relation's independent JVP replay.

This is one host-local real-`f64` faer reference. It does not establish Stokes
E2, prepared factors, cross-call reuse, explicit transpose storage, other
backends, complex/device/distributed execution, persistence, or performance.
The immutable preimplementation authority and its exact bounds are in
[`references/README.md`](references/README.md).

Run the registered evidence with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case differentiation.materialized-direct-output
```
