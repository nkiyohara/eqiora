# Frozen observations

- Vertex `v(i,j) = 17i+j` is `(i/16,j/16)` for `i,j = 0..16`.
- Cell `c(i,j) = 16i+j` is
  `[17i+j, 17(i+1)+j, 17i+j+1, 17(i+1)+j+1]`.
- The body owns cells `0..255`.
- For `t = 0..15`, boundary facets are `x_lower=272+t`,
  `x_upper=528+t`, `y_lower=17t`, and `y_upper=17t+16`.
- The displacement snapshot is vertex-associated continuous Q1 with shape
  `(2,)`, coherent-SI length dimension, spatial-Cartesian frame, and 578 f64
  coefficients in entity-major/component-last order.
- Every zero coefficient has the positive-zero bit pattern. The full
  coefficient sequence is bit-identical to the already accepted solver
  projection; it is not pinned to analytic rational bytes.
- The Run output inventory is the singleton displacement snapshot digest.

The Rust oracle freezes the new JSON schemas and top-level key order
relationally. No preimplementation full-byte or digest fixture is claimed.
