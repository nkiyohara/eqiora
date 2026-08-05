# Expected evidence

The positive witness has exactly the authored names `inlet`, `outlet`,
`walls`, and `cylinder`, plus all `fluid` cells. Boundary memberships come only
from the accepted correspondence; they are nonempty, pairwise disjoint,
exterior, and cover every exterior facet exactly once. Inlet, walls, and
cylinder are homogeneous essential velocity boundaries. Outlet is homogeneous
traction, so pressure uses `BoundaryTraction` and no gauge block exists.

The portable graph contains one fixed 2D scaled domain, two replicated F64
fields in MINI/P1 spaces, imported exact-mesh CG with the accepted triangle
quadrature, backward Euler and energy-skew transformations, one general
two-field system, one BiCGSTAB linear solve, one nonlinear root, and one-worker
host/offline placement. It contains no moving-geometry action or output,
observer, artifact, persistence, or environment ordinal.

One step from exact-zero data yields two states and one evidence record. Time
advances by the selected duration; all stored values are finite and exactly
`0.0`; final, momentum, continuity, convective, convective-power, and
conservative-advection-defect evidence is exactly zero; and checked assembly
materializes at least one packet.

There are no reference arrays, nonzero expected values, numerical tolerances,
digests, or generated artifacts in this directory. Rejection witnesses must
fail before checked materialization. They intentionally assert no
initial-state equality, direct replay-call count, ledger, marker, or duplicate
equality.
