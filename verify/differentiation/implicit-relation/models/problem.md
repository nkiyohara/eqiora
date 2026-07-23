# Problem definition

At the accepted point `w = (2, 1)`, `p = (5, 7)`, verify the implicit relation

```text
R1 = w0^2 + w1 - p0 = 0
R2 = 2 w0 + 3 w1 - p1 = 0.
```

Its state and parameter Jacobians are

```text
R_w = [[4, 1], [2, 3]],
R_p = -I.
```

The nonsymmetry is intentional: the adjoint must solve the VJP-backed system
`R_w^T lambda = J_w^T`, not the forward system.
