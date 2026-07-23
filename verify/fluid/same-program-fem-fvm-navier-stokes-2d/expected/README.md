# Acceptance

The executable test owns all tolerances. It requires distinct admitted spatial
methods, exact final Model/Field/time agreement, the nonzero canonical body
force `[1, 0]`, an initial residual greater than `1e-6` in both methods, a
positive nonlinear update count in both methods, and a final FVM pressure span
greater than `0.7 Pa`.

Maximum FEM and FVM velocity coefficients must each remain below `2e-9 m/s`.
Every FEM pressure vertex must reproduce `x - 0.5 Pa` within `2e-9 Pa`. At all
sixteen common Cartesian cell centers, each pressure's error from that oracle
and the direct FEM/FVM pressure difference must also remain below `2e-9 Pa`.

No golden coefficient vector is stored. Exact semantic identity, the analytic
physical equilibrium, and common-location observations are the stable
evidence boundary.
