# Canonical tensor structure operators

This case verifies the bounded semantic bridge from shaped continuum source to
two physics-neutral tensor structure operations. The fixture expresses

```text
-div(
  2 * mu * symmetric_part(grad(displacement))
  + lambda * isotropic_lift(div(displacement))
) = 0
```

as one ordinary implicit Relation. `symmetric_part` means
`(T + transpose(T)) / 2` for an exact spatial Cartesian `[d,d]` tensor on a
Cartesian volume. `isotropic_lift` means `s I_d` for an invariant scalar on
that volume. Both preserve physical dimension and exact nominal support; the
latter obtains `d` only from the admitted support.

The integration target checks that source lowering preserves the two canonical
operators and that committed semantic validation accepts their exact composed
types. It separately exercises the public identity-parametric typing rules and
the exact pointwise component maps: symmetric off-diagonal coordinates read
both direct and transposed inputs, while an isotropic lift reads the scalar only
on the diagonal and emits a dimensioned zero elsewhere. Nonsquare extent,
wrong frame, boundary/no support, and shaped/global-scalar misuse fail closed.

The accepted Relation and its ordered transaction cross only explicit wire
v4:

```text
eqiora.model-envelope/v4
eqiora.model-transaction-envelope/v4
```

Canonical bytes, domain-separated digests, and typed replay must agree exactly.
V1, v2, and v3 reject the new expression nodes; relabeling a v4 payload as v3
does not admit it. The separate `eqiora-artifact` `legacy_v3_golden` regression
freezes the pre-v4 v3 byte count and digest without expanding this case's
evidence target.

Run:

```bash
cargo test --locked -p eqiora --test canonical_tensor_operators
cargo run -p eqiora-verify -- run --case language.canonical-tensor-operators
```

This is not an elasticity execution case. It does not define a material
package, boundary conditions, mesh, finite element, weak form, local assembly,
solver, displacement/stress result, patch test, convergence rate, or FSI. The
full displayed Relation demonstrates semantic composition only; pointwise
component scalarization does not claim a realization of its `grad` or `div`.
