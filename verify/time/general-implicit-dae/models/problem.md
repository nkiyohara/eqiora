# Analytic problem definition

For `t in [0, 1]`, solve

```text
(1 + z) (x_dot + x) = 0
z - x^2 = 0
```

The initial guess is

```text
x = 1, z = 0, x_dot = 0, z_dot = 0.
```

Because `1 + z` is positive on the admitted solution, the differential
equation gives `x_dot = -x`. The algebraic equation gives `z = x^2`; therefore

```text
x(t) = exp(-t)
z(t) = exp(-2t).
```

The derivative Jacobian contains the state-dependent coefficient `1 + z`, so
the canonical residual cannot be represented by the existing constant
mass-matrix projection. The differential/algebraic partition is `(x; z)`.
Consistent initialization holds `x = 1` and `z_dot = 0` fixed and solves the
two residuals for `x_dot = -1` and `z = 1`.

The reference backend applies backward Euler (BDF1) with 10, 20, 40, and 80
uniform steps. The terminal error is the Euclidean norm of the errors in
`(x, z)`. The constraint quantity is `abs(z - x^2)` at the terminal sample.
