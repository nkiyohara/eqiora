# Compiled Cartesian elasticity Q1 verification

This case verifies the second consumer of Eqiora's private proof-carrying FEM
derivation. The ordinary canonical two-dimensional isotropic-elasticity path
must produce the accepted Cartesian Q1 local contribution and the same private
operator must own its state and `(mu, lambda, pressure_gradient)` differential
actions and paired transpose actions.

The evidence is split deliberately:

- the crate-private compiler child owns the exact local derivative oracle,
  certificate mutations, and fail-closed admission checks; and
- `compiled_cartesian_elasticity` reaches the ordinary public solid-mechanics
  path, checks the exact lower-left-cell contribution, and retains the separate
  affine-stress and loaded homogeneous-boundary patch balances.

Run:

```bash
cargo test --locked -p eqiora-numerics --test compiled_cartesian_elasticity
cargo run -p eqiora-verify -- run --case numerics.compiled-cartesian-elasticity-q1-2d
```

The claim is limited to the frozen two-component, two-dimensional, square
affine HypercubeQ1 cell with two-point tensor Gauss quadrature, constant Lamé
coefficients, affine conservative load potential, and complete homogeneous
essential boundary. It does not establish a public or general weak-form or
differential-map API.
