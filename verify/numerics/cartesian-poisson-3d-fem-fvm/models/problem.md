# Analytic problem definition

On `Omega = (0, 1) x (0, 1) x (0, 1)`, solve

```text
-div(grad(u)) = 3 pi^2 sin(pi x) sin(pi y) sin(pi z)
u = 0 on the complete boundary.
```

For `u(x, y, z) = sin(pi x) sin(pi y) sin(pi z)`, each of the three second
derivatives equals `-pi^2 u`; therefore `-Delta u = 3 pi^2 u`. The field
vanishes on all six faces. The Dirichlet Poisson operator is coercive, so this
solution is unique.

Uniform Cartesian meshes with 4, 8, and 16 cells per axis are used. FEM and
FVM receive one canonical source tape, one mesh revision per level, the same
assembly and solver contracts, and method-specific local operators. The exact
field is used only for evidence calculation.
