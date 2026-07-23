# Problem definition

On the unit square, solve

```text
-div(grad(u)) = 2 pi^2 sin(pi x) sin(pi y),
u = 0 on the complete boundary.
```

The analytic solution is `u = sin(pi x) sin(pi y)`. The device gate uses a
fixed 16-by-16 Cartesian mesh so Q1 FEM and cell-centered TPFA exercise
different method-native unknowns and reconstructions from the same canonical
revision.
