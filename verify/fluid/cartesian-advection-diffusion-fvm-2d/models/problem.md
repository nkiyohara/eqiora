# Analytic zero-initial inflow-step transport pair

Let `Omega = (0, 1)^2`, `a > 0`, and `kappa > 0`. The direct problem is

```text
dc/dt + a dc/dx - kappa (d2c/dx2 + d2c/dy2) = 0,
c(0, y, t) = 1,
kappa dc/dx (1, y, t) = 0,
kappa n . grad(c) = 0 on y = 0 and y = 1.
c(x, y, 0) = 0.
```

Define

```text
beta   = a / (2 kappa),
mu_n cos(mu_n) + beta sin(mu_n) = 0,
  (n + 1/2) pi < mu_n < (n + 1) pi,
N_n = 1/2 - sin(2 mu_n) / (4 mu_n),
A_n = -mu_n / ((beta^2 + mu_n^2) N_n),
lambda_n = a^2 / (4 kappa) + kappa mu_n^2.
```

For every positive time, the direct exact solution is the convergent spectral
series

```text
c_plus(x, y, t)
  = 1 + exp(beta x)
      sum_(n=0)^infinity A_n exp(-lambda_n t) sin(mu_n x).
```

Writing `c - 1 = exp(beta x) v` removes the first spatial derivative. The
Robin condition `v_x(1) + beta v(1) = 0` gives the roots above. Expanding the
transformed initial value `v(x, 0) = -exp(-beta x)` in those sine modes gives
`A_n`: the boundary relation cancels the exponential endpoint term in the
projection integral. Thus the series has the authored zero initial condition,
unit inflow trace, zero outflow diffusive flux, and no manufactured source.
It is independent of `y`, so both horizontal diffusive fluxes vanish.

The reverse-flow oracle is the exact reflection

```text
c_minus(x, y, t) = c_plus(1 - x, y, t).
```

It solves the same equation with velocity `(-a, 0)`, prescribed trace at
`x = 1`, and zero diffusive flux at `x = 0`. Direct and mirrored numerical
solutions on reflected Cartesian cells must therefore agree after index
reflection within the declared linear-solve and floating-point tolerances.

Spatial refinement uses `dt = h^2 / 2`. Backward Euler therefore contributes
`O(h^2)` time error while the first-order upwind spatial error is `O(h)`;
`dt / h = h / 2` tends to zero and the measured leading order is spatial.
For the previous-state minmod/implicit-diffusion path, the same step scaling
keeps first-order IMEX time error at `O(h^2)` while the nominally second-order
spatial reconstruction is measured by the registered greater-than-1.6
observed-order gate.
Temporal refinement fixes one spatial mesh and uses the common-final-time
step-doubling difference quotient, so the unchanged spatial truncation error
cancels rather than contaminating the measured backward-Euler order. A
separate canonical model with initial state `1 K` checks exact constant-state
preservation; the inflow-step run checks that the implicit upwind/TPFA solution
remains in the `[0 K, 1 K]` initial-and-inflow hull.
