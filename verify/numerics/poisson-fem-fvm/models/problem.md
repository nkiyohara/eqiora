# Analytic problem definition

On `Omega = (0, 1)`, solve

```text
-d2u/dx2 = pi^2 sin(pi x)
u(0) = 0
u(1) = 0.
```

Twice differentiating `u(x) = sin(pi x)` gives
`u''(x) = -pi^2 sin(pi x)`, so this function satisfies the differential
equation and both boundary values. The one-dimensional Dirichlet Poisson
operator is coercive, hence the solution is unique.

Uniform meshes with 8, 16, 32, 64, and 128 cells are used. FEM and FVM
receive the same mesh, canonical lowered source tape, four-point
Gauss–Legendre cell quadrature, sparse assembler, and conjugate-gradient
controls. The exact function is used only by the evidence calculation.
