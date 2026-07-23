# Dynamic linear-solid semantics

This case closes the first-order, package-neutral Semantic Model seam required
by a later fixed-reference fluid--structure interaction Realization. Direct
Relations and the exact `Eqiora.Solid.LinearElasticity@0.4.0` package express
the same two-dimensional small-strain body:

```text
derivative(displacement) - velocity = 0
density * derivative(velocity) - div(stress(displacement))
  - grad(load_potential) = 0
```

The package boundary binds velocity, not displacement, to the exact neutral
`Eqiora.Mechanics.Interfaces@0.1.0::VelocityTractionBoundary`. Elastic stress
continues to depend on displacement. Four zero-velocity terminals close the
ordinary conserving network, and the method-neutral lowerer must produce the
same physical coefficients, Field roles, load tape, bounds, and four
`TraceZero` dispositions as direct authoring. Dependency aliases, Cartesian
boundary declaration/family order, and Connection-member order do not change
the admitted lowered meaning.
One zero-traction substitution lowers to `FluxZero`; one compatible unresolved
connection remains an exact `PortBinding`. Neither is silently converted to a
time or coupling policy.

The package fixture binds density and Lamé terms as exact typed literals.
Under RFC 0055 they specialize to constants and expose no occurrence-local
Parameter alias or AD direction; the direct fixture intentionally retains its
root Model Parameters. Their admitted primal coefficients agree, while this
case does not claim direct/package coefficient-sensitivity equivalence.

The dynamic-solid projection normalizes an exact global sign reversal of
either dynamic residual because `R = 0` and `-R = 0` have the same zero set;
this does not rewrite Model identity or canonical bytes. Falsifiers reject
missing or malformed kinematics, inertia on the wrong Field, stress on velocity, nonpositive
density or inadmissible Lamé parameters, mismatched boundary coefficients,
a nominally distinct Connector, and unexpected Model content before any mesh
is inspected.

Run:

```bash
cargo test --locked -p eqiora --test dynamic_linear_solid_semantics_2d
cargo run -p eqiora-verify -- run --case solid.dynamic-linear-solid-semantics-2d
```

The source initializes the scalar load-potential carrier to zero before its
algebraic definition is enforced. It owns no shaped displacement/velocity
initial-state artifact, mass matrix, time method, structural-dynamics solve,
FSI, transfer, ALE, moving geometry, or CAD claim.
