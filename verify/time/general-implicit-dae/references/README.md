# Reference provenance

The exact solution, consistent initial pair, and algebraic constraint are
derived directly in [`models/problem.md`](../models/problem.md). No external
dataset is used.

The residual-native contract follows the current SUNDIALS IDA formulation
`F(t, y, y_dot) = 0`, its differential/algebraic variable partition for
consistent initialization, and its combined Newton Jacobian
`F_y + alpha F_y_dot`:

- [SUNDIALS 7.8.0 release and IDA documentation](https://computing.llnl.gov/projects/sundials/sundials-software)
- [IDA mathematical considerations](https://sundials.readthedocs.io/en/latest/ida/Mathematics_link.html)

`expected/convergence.csv` records deterministic reference-backend output. CI
also enforces monotone error reduction, order above the declared threshold,
consistent initialization, JVP values, and the independent constraint bound.
The table is regression evidence, not a second model semantics.
