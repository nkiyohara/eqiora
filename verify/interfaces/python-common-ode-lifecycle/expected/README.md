# Acceptance boundary

At requested times `0.1 s` and `0.2 s`, each Eqiora value must agree with the
independent `exp(-t / s)` reference using `rel_tol = 2e-8` and
`abs_tol = 2e-10`. The case also requires exact requested time values, exact
FieldRef-only selection, and fresh/replayed resolver agreement.

The comparison tolerances are declared as a conservative factor of 20 over the
requested Tsitouras controls (`rtol = 1e-9`, `atol = 1e-11`). They were chosen
before observing implementation output, are not inherited from the displaced
backward-Euler path, and make no theorem or general claim about adaptive global
error.
