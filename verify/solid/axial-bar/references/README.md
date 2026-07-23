# Analytic reference

For a prismatic bar with constant Young's modulus `E`, area `A`, length `L`,
zero distributed load, a fixed lower endpoint, and tensile end force `P`,
equilibrium gives a constant axial force:

```text
N = E A du/dx = P
u(x) = P x / (E A)
stress = N / A = P / A
reaction at x = 0 = -P
```

With `E = 200e9 Pa`, `A = 0.01 m^2`, `L = 2 m`, and `P = 10000 N`,
the reference quantities are `u(L) = 1e-5 m`, `stress = 1e6 Pa`, and
`reaction = -10000 N`. The derivation is elementary and contains no
third-party redistributable data.
