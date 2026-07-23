# Analytic problem

On the unit square, solve

```text
-div(grad(u)) = 2 pi^2 sin(pi x) sin(pi y),
u = 0 on the complete boundary.
```

Twice differentiating `u(x, y) = sin(pi x) sin(pi y)` gives the stated
source. The homogeneous boundary values follow because either sine factor
vanishes on every side. The fixed 16-by-16 Cartesian mesh gives Q1 FEM and
cell-centered TPFA different method-native unknowns and reconstructions while
both remain projections of the same immutable Model artifact.
