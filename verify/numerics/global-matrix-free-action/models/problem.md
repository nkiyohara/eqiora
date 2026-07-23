# Problem definition

For each spatial dimension `d = 1, 2, 3`, solve

```text
-div(1.25 grad(u)) = 0  on (-0.75, 1.25)^d
u = 1 + sum_i ((i + 1) / 8) x_i  on the complete boundary.
```

The affine boundary field is harmonic and belongs exactly to the Cartesian Q1
space. Three cells per axis leave at least one unconstrained interior vertex
in every dimension. The same entity-local stiffness coefficients and
constraint maps feed both the packet operator and the independent reference
CSR assembler.

The separate nonsymmetric three-DOF fixture is algebraic rather than a second
physical model. It exists to falsify transpose, skipped-row, duplicate-scatter,
and fixed-column projection mistakes that symmetric diffusion cannot reveal.
