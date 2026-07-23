# Problem definition

The canonical residual is

```text
(1 + z) (x_dot + p x) = 0,
z - x^2 = 0,
```

with `p = 1`, consistent initial pair
`(x, z, x_dot, z_dot) = (1, 1, -1, 0)`, and fixed implicit-Euler step
`h = 0.1`. The parent advances from `t = 0.0` to `t = 0.1`; the child is
constructed from the accepted checkpoint pair and advances to `t = 0.2`.

The checkpoint residual is evaluated by re-lowering the immutable canonical
model, not copied from the time backend's report. State and derivative vectors
follow the lowering witness's canonical field order.
