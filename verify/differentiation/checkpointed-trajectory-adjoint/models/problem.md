# Problem definition

The canonical residual is

```text
(1 + z) (x_dot + p x) = 0,
z - x^2 = 0,
```

with `p = 1`, initial state `(x, z) = (1, 1)`, and four accepted
implicit-Euler steps of size `0.125`. The semantic checkpoint at `t = 0.25`
stores the accepted state/derivative pair and links a parent run to a child run
through `ImplicitTimeRestartManifestV1`.
