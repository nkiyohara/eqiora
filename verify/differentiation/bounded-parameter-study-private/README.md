# Bounded Parameter study private composition evidence

This companion case is required with
`differentiation.bounded-parameter-study`. The primary case executes the
unchanged public `eqiora/bounded_parameter_study` authority; this case selects
one exact crate-private `eqiora-api` library test that executes all seven
injected evaluator and complete-construction falsifiers.

The private oracle checks canonical serial call order and depth, immediate
failure termination, between-point cancellation, completion after the final
member, exact complete membership, and rejection of foreign or substituted
evaluations. It adds no pointwise scientific value, digest, or tolerance.

Run both required authorities with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case differentiation.bounded-parameter-study \
  --case differentiation.bounded-parameter-study-private
```

The exact source selected by this case is packaged with `eqiora-api`; no
cross-package source include or public test seam is used.
