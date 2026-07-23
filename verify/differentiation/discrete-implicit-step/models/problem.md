# Problem definition

The canonical Relation is

```text
(1 + z) (x_dot + p x) = 0,
z - x^2 = 0,
```

with `p = 1`, accepted previous state `(x_previous, z_previous) = (1, 1)`,
and implicit-Euler step `h = 0.1`. The accepted next state is

```text
x_next = x_previous / (1 + h p),
z_next = x_next^2.
```

The step relation is linearized with unknown `(x_next, z_next)` and selected
parameter order `(x_previous, z_previous, p)`. Its Jacobian with respect to the
next state is nonsymmetric, so substituting the normal action for the
transposed VJP action cannot pass the adjoint evidence accidentally.
