# Compiled Cartesian elasticity Q1 verification

This case verifies the second consumer of Eqiora's private proof-carrying FEM
derivation. The ordinary canonical two-dimensional isotropic-elasticity path
must produce the accepted Cartesian Q1 local contribution and the same private
operator must own its state and `(mu, lambda, pressure_gradient)` differential
actions and paired transpose actions.

The evidence ownership is deliberate:

- the exact registered crate-private aggregate owns the local derivative
  oracle, certificate mutations, fail-closed admission checks, ordinary local
  construction, and both complete patch balances; and
- `compiled_cartesian_elasticity` is supplementary formula-free smoke for the
  outward public solid-mechanics construction and scatter boundary.

Run:

```bash
cargo test --locked -p eqiora-numerics --lib \
  form_compiler::elasticity::oracle::registered_evidence -- --exact
cargo test --locked -p eqiora-numerics --test compiled_cartesian_elasticity
cargo run -p eqiora-verify -- run --case numerics.compiled-cartesian-elasticity-q1-2d
```

The claim is limited to the frozen two-component, two-dimensional, square
affine HypercubeQ1 cell with two-point tensor Gauss quadrature, constant Lamé
coefficients, affine conservative load potential, and complete homogeneous
essential boundary. It does not establish a public or general weak-form or
differential-map API.
