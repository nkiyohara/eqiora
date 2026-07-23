# Implicit differentiation verification

This case verifies the first complete differentiation path without changing
canonical Relation semantics or differentiating solver iterations:

```text
scalar residual SSA
    -> accepted LinearizedRelation at R(w, p) = 0
    -> primal + JVP + VJP
    -> R_w / R_p matrix-free actions
    -> normal and transposed faer BiCGSTAB solves
    -> forward sensitivity and adjoint gradient
```

The manufactured relation has a nonsymmetric state Jacobian so using the
normal action in place of the VJP-backed transpose cannot pass accidentally.
Forward sensitivities and total objective gradients are checked against closed
form values and centered finite differences of the independently evaluated
implicit solution map. The scalar IR unit evidence additionally checks
JVP/VJP duality and malformed-shape rejection.

This is a smooth, static, host-local, `f64` claim. It is not evidence for time
integration, hybrid event-time sensitivity, saltation, spatial coefficients,
other scalar representations, distributed execution, or accelerators.

Run:

```bash
cargo test -p eqiora-ir
cargo test -p eqiora --test implicit_differentiation --all-features
cargo run -p eqiora-verify -- run --case differentiation.implicit-relation
```

See [RFC 0011](../../../rfcs/0011-implicit-differentiation-contracts.md).
