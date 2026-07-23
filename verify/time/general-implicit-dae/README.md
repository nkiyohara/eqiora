# Canonical general implicit DAE verification

This case closes one canonical Relation through residual-native lowering,
explicit differential/algebraic partitioning, consistent initialization, and
a deterministic implicit-Euler reference backend. The Relation cannot enter
the constant first-order `TimeProblem` seam because its derivative Jacobian is
state dependent.

The verification checks six independent boundaries:

- `FirstOrderProgram` rejects the Relation rather than inventing a constant
  mass matrix;
- `GeneralImplicitProgram` retains `F(t, y, y_dot)` and its analytic JVP;
- consistent initialization holds the differential state fixed while solving
  the algebraic state and differential derivative;
- BDF1 terminal error converges at first order while the algebraic constraint
  remains at roundoff;
- a nonlinear derivative residual `x_dot^2 - 1 = 0` retains the caller's
  explicit `x_dot = +1` branch choice and is not coerced into first-order form;
- general lowering, supplied/accepted initial pairs, and run linkage
  round-trip through distinct versioned artifacts with drift rejection.

- Analytic derivation: [`models/problem.md`](models/problem.md)
- Reference provenance: [`references/README.md`](references/README.md)
- Reproducible table: [`expected/convergence.csv`](expected/convergence.csv)

Run:

```bash
cargo run -p eqiora-verify -- run --case time.general-implicit-dae
```

The verified claim is one small, semi-explicit index-one system with a
state-dependent derivative coefficient and a dense reference Newton solve. It
also checks one provided branch of a scalar nonlinear-derivative residual. It
does not claim a production IDA adapter, arbitrary-index DAE solvability,
automatic branch selection, adaptive BDF, sparse nonlinear algebra, hybrid DAE
events, or DAE sensitivity. Semantic checkpoint/restart lineage is verified by
the separate `artifacts.implicit-time-restart-lineage` case and does not widen
this solver claim.
