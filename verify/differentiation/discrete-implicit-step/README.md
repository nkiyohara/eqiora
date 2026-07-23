# Discrete implicit-step differentiation

This case differentiates one accepted residual-native implicit-Euler step,
without differentiating the Newton iterations that produced it:

```text
canonical F(t, y, y_dot, p) = 0
    -> y_dot = (y_next - y_previous) / h
    -> G(y_next; y_previous, p) = 0
    -> paired step JVP / VJP
    -> normal forward solve / transposed adjoint solve
```

`y_next` is the solved unknown. The selected parameter vector is explicitly
ordered as `[y_previous, canonical model Parameters]`; model time and step size
are frozen realization data. The projection composes the independently lowered
Field, Derivative, and Parameter coordinates by the chain rule and its
transpose. It does not modify scalar Operator IR roles or introduce a second
differentiation implementation.

The canonical state-dependent-coefficient DAE has a nonsymmetric step
Jacobian. The test checks JVP/VJP duality, forward sensitivity, and an actual
VJP-backed transposed adjoint. Both derivative modes are compared with centered
finite differences of the independent closed-form implicit-Euler solution map.
An off-manifold next state, an invalid step, and malformed tangent shape fail
closed.

This verifies one accepted `h = 0.1` implicit-Euler step. It does not establish
BDF-history differentiation, adaptive-controller gradients, continuous DAE
sensitivity, or event composition. Multi-step reverse accumulation over one
fixed-step semantic restart is a distinct verified case; this single-step case
does not imply that wider capability.

Run:

```bash
cargo test -p eqiora --test discrete_time_differentiation
cargo run -p eqiora-verify -- run --case differentiation.discrete-implicit-step
```

See [RFC 0011](../../../rfcs/0011-implicit-differentiation-contracts.md).
