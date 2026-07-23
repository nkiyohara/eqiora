# Checkpointed discrete trajectory adjoint

Four accepted residual-native implicit-Euler step relations are composed in
reverse. A content-addressed semantic checkpoint and restart manifest split
the primal trajectory after step two. The artifact edge is validated first;
the numerical adjoint contract then independently requires exact canonical
state, time, state-field, and model-Parameter continuity on both sides of that
boundary before any transposed solve begins.

The terminal-state cotangent is propagated through all four method-specific
step residuals. Contributions to the common model Parameter are accumulated,
and the resulting initial-state cotangent and Parameter gradient are compared
with centered differences of an independent four-step solution map.

This is fixed-step implicit Euler over one semantic restart. It does not claim
adaptive-controller differentiation, BDF-history adjoints, optimal checkpoint
schedules, backend-native checkpoint payloads, derivative-run artifacts, or
intermediate running-objective terms.
