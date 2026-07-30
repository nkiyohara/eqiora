# Expected outcome

The native composition publishes exactly one immutable presentation built from
the exact checked-in fixture: 9 ordered mesh vertices, 8 nondegenerate affine
triangles partitioned as 4 fluid and 4 solid cells, and 2 interface facets on
the complete conforming side at `x = 1 m`. Shared velocity is carried on the 3
interface vertices with a bit-identical fluid and solid trace; exactly one of
them, the free midpoint at `(1 m, 0.5 m)`, carries a recovered interface
action. Pressure is P1 on the 6 fluid-closure vertices only, the fluid MINI
velocity retains both its vertex and its cell-bubble block, and solid
displacement is exact positive zero outside the solid closure.

Two accepted steps are published at `0.05 s` and `0.10 s`, in that order, from
two genuine consecutive solves. The second must be a distinct accepted state,
not a repeat of the first. Values are finite and coherent-SI: velocity in
`m/s`, pressure in `kg m^-1 s^-2`, displacement in `m`, intrinsic
two-dimensional interface action in `N/m`, and intrinsic energy in `J/m`.
Action and energy are never presented under a shared unit.

Solver stopping evidence — the frozen MINRES/identity/reproducible tuple, its
target, and the backend stopping report — is published as its own group,
separate from physics acceptance. Physics acceptance is the independently
reapplied evidence and reuses the registered thresholds unchanged: true
residual no greater than the solver-owned target, numerical residual and
continuity below `1e-9`, kinematic residual below `1e-14`, a shared-trace
velocity jump of exactly zero, interface action imbalance below `1e-9 N/m`,
and absolute energy defect below `1e-9 J/m`.

Exact Model, geometry, correspondence, mesh, Realization, final Run, state,
and trajectory identities agree across the whole composition and are built
from the same accepted program, plan, and states. A wrong cardinality,
connectivity order, support, block, unit, execution tuple, residual, threshold,
step order, case attribution, foreign or stale lineage digest, or superseded
asynchronous response prevents ready-state publication. Browser preview
publishes no scientific substitute.

No expected coefficient, interface action, energy term, residual, or digest
value is copied here. Studio recomputes no physics: every presented number is
a retained solver-owned value, and its scientific acceptance remains owned by
`fsi.fixed-reference-monolithic-step-2d` and
`artifacts.fixed-reference-fsi-spatial-trajectory`.
