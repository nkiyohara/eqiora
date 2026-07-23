# Acceptance criteria

- Tsitouras 5(4) samples agree with `exp(-2t)` within `3e-8` relative scale.
- BDF stiff samples agree with `cos(t)` within `2e-6` relative scale.
- BDF DAE samples agree with the analytic `x,z` trajectory within `2e-6` and
  satisfy `x + z = 1` within `5e-9`.
- Tsitouras and BDF parameter sensitivities agree with `-t exp(-kt)` within
  `3e-6` relative scale.
- Both methods propose roots at `t = 1` and, after an external reset/restart,
  `t = 1.5`, with the proposed pre-event state on the zero surface.
- The report retains the Diffsol backend identity, selected method, exact
  equation class, and consistent-initialization policy.
- General implicit DAE and mass-matrix plus explicit-RK admissions fail closed.
