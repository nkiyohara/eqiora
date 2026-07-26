# Compiled Cartesian Poisson Q1 verification

This case compiles the checked-in `org.example.poisson` package through the
ordinary explicit Cartesian Q1 Realization. It verifies that the eligible
relation takes the private proof-carrying form path, while preserving the
precommitted local-shadow, convergence, and conservation contracts from
RFC 0075.

The registered package-path evidence owns the role-inventory, source-omission,
and local-to-global mapping falsifiers. The complementary non-square,
off-origin element fixture is a focused `eqiora-numerics` test because the
ordinary uniform package Realization can produce only square cells.

Run:

```bash
cargo run -p eqiora-verify -- run --case numerics.compiled-cartesian-poisson-q1-2d
```

The claim is limited to the exact scalar, homogeneous-essential-boundary,
two-dimensional Cartesian HypercubeQ1 slice described in the manifest. It
does not establish a public or general weak-form compiler.
