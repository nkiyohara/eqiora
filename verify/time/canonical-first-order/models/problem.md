# Problem definition

The canonical residual network represents

```text
x' = -2 x,  x(0) = 1
z' = x,     z(0) = 0
```

with residual order `[z, x]` and derivative Jacobian

```text
[ 0  2]
[-1  0].
```

Thus the analytic solution is

```text
x(t) = exp(-2t)
z(t) = (1 - exp(-2t)) / 2.
```

The rejection fixture uses `y y' + k y = 0`. Its derivative coefficient is
state-dependent, so it is a valid implicit Relation but not an admitted
first-order projection. Residual-native execution is verified independently by
`time.general-implicit-dae`.

The mass-matrix fixture represents

```text
x' = -x + z
0  = x + z - 1
```

with the inconsistent initial guess `(0,0)`. Its exact derivative Jacobian is
`diag(1,0)`. Consistency yields `z=1-x` and
`x(t)=(1-exp(-2t))/2`.
