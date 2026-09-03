# References

- [Diffsol 0.16.2 crate documentation](https://docs.rs/diffsol/0.16.2/diffsol/)
  describes Tsitouras 5(4), BDF, mass matrices, and consistent solver-state
  construction.
- [`OdeSolverProblem::bdf`](https://docs.rs/diffsol/0.16.2/diffsol/ode_solver/problem/struct.OdeSolverProblem.html#method.bdf)
  constructs a consistent BDF state before integration.

The analytic solutions in this case are derived directly from the equations
in `models/problem.md`; no Diffsol-generated reference trajectory is used.
