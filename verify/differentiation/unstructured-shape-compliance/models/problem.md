# Manufactured problem

The canonical model is

```text
-Delta u = 1                  in (0, Lx) x (0, Ly)
u = 0                         on the complete boundary.
```

The derivative point is `Lx = 1.15`, `Ly = 0.85`. The P1 realization uses a
distorted triangular mesh whose connectivity and normalized vertex pattern are
held fixed when either upper Domain bound varies.

The objective is compliance, `J_h = integral u_h dx` for the unit source. Its
continuous comparison is the rectangular Fourier-sine series

```text
J = sum_(m,n odd)
    64 Lx Ly /
    [m^2 n^2 pi^6 (m^2/Lx^2 + n^2/Ly^2)].
```
