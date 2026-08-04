# Bounded Parameter study composition

This case freezes one composition oracle over the accepted two-dimensional
generated-Cartesian Poisson differentiable program. The study retains the
finite-element program with ordered inputs `source_scale`, `diffusion`, and
`boundary_offset`; varies only `diffusion`; and canonicalizes the deliberately
permuted caller values `1.25, 0.75, 1.0` into `0.75, 1.0, 1.25`.

The positive oracle executes the existing `DifferentiableProgram` separately
at all three canonical complete points before executing the study. It compares
every member's exact program identity, ordered Parameter IDs, point bits,
complete Field coefficient bits, state-system and accepted-output
fingerprints, solve report, and exposed primal evidence with the corresponding
separate accepted evaluation. No coefficient, scientific digest, expected
value, or tolerance is added here.

Planning tests freeze exact-bit duplicate rejection, the 2--64 point bound,
the mandatory default anchor, selected-Parameter ownership, finiteness, plan
equality, and signed-zero ordering. Public execution tests freeze failure
atomicity and cancellation immediately before the first point and between
accepted points. The required companion case
[`differentiation.bounded-parameter-study-private`](../bounded-parameter-study-private/README.md)
injects evaluators and member vectors to reject parallel or repeated calls,
foreign members, missing, duplicate, inserted, reordered, or substituted
members, and continuation after a failure.

The first slice does not claim Python exposure, general batching, parallel or
remote scheduling, alternate bases, several varying Parameters, caching or
solver reuse, derivatives across the study axis, optimization, UQ,
persistence, or a general Study abstraction. The complete executable boundary
and falsifiers are recorded in [`case.toml`](case.toml).

Run the composed evidence with:

```console
cargo test --locked -p eqiora --test bounded_parameter_study
cargo run --locked -p eqiora-verify -- run \
  --case differentiation.bounded-parameter-study \
  --case differentiation.bounded-parameter-study-private
```

At the preimplementation base revision, the production module and public
registrations intentionally do not exist, so both Rust authorities are
expected not to compile or resolve. They must pass after composition without
changing either oracle.
