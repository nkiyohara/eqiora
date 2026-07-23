# Problem definitions

The executable test defines three parameter-bound lowered systems:

1. `y' = -2y`, `y(0) = 1`, with `y(t) = exp(-2t)`.
2. `y' = -1000(y - cos(t)) - sin(t)`, `y(0) = 1`, with
   `y(t) = cos(t)`.
3. `diag(1, 0) [x', z']^T = [-x + z, x + z - 1]^T`, using
   the inconsistent initial guess `(0, 0)`. Consistency gives `z = 1 - x`
   and the trajectory is `x(t) = (1 - exp(-2t))/2`.

All right-hand-side Jacobian actions and the DAE mass action are supplied
analytically through the backend-neutral contract.

The forward-sensitivity case uses `y' = -k y`, `y(0) = 1` at `k = 0.7`.
Its independent parameter derivative is `dy/dk = -t exp(-kt)`.

The root/reset boundary uses `y' = -1`, first from `y(0) = 1`, then from an
Eqiora-side reset to `y = 0.5`. The two analytic root times are `1` and `1.5`.
