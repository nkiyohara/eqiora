# Model

The canonical model is the constant-load Poisson problem

```text
-div(grad(u)) = 1  on (0, Lx) x (0, Ly)
u = 0              on the complete boundary.
```

`Lx` and `Ly` are selected by their typed Domain-bound coordinates at each
model revision. They are not inferred from node kind and are not represented
as realization-local mesh-vertex IDs.

The verification point is `Lx = 1.15`, `Ly = 0.85`. The optimization view uses
`Lx = exp(s)`, `Ly = exp(-s)` so positive lengths and unit area hold by
construction.
