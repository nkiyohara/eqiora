# Transversal hybrid-event differentiation

This case lowers the canonical bouncing-ball flow, scalar guard, and two split
implicit reset Relations through the same scalar Operator IR used by primal
execution. The runtime discovers all Event Activations with the same structural
guard and direction, solves their `Next` Relations as one reset, and obtains
guard/reset derivatives from Operator-IR JVP actions.

At a localized event, with `n = g_y` and

```text
d = g_t + n^T f^-,
```

the checked event-time and saltation formulas are

```text
tau_p = -(n^T S^- + g_p) / d,
Xi = rho_y + (f^+ - rho_y f^- - rho_t) n^T / d,
S^+ = rho_y S^- + rho_p + (rho_y f^- + rho_t - f^+) tau_p.
```

The integration test compares gravity and restitution sensitivities, the reset
Jacobians, and the complete saltation matrix with analytic first-impact values.
It also rejects a crossing-direction mismatch and a point outside the declared
guard-localization band.

This is a deliberately narrow verified claim. This test supplies the localized
point directly; the separate `hybrid.registered-event` case connects the same
event class to a content-linked production root proposal and explicit restart.
General DAE events, mode-dependent post-event flow, distinct simultaneous
guards, grazing derivatives, trajectory adjoints, and checkpoint lineage
remain unsupported.

Run:

```bash
cargo test -p eqiora-runtime --test canonical_hybrid_sensitivity
```
