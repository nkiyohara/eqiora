# Expected structural evidence

The positive witness retains one exact Model artifact reference, one exact
Cartesian mesh reference, one parent, one nominal Connector, three distinct
axis-oriented `SpatialPeriodic` Connections, and six distinct lower/upper
Ports. Every pair passes the existing semantic composition before group
admission. Three pair-commutator receipts and all six three-generator order
receipts retain identity fiber, anchor, closure, and incidence meaning.

Independent enumeration observes 24 vertices, 72 edges, 72 faces, and 24
cells in the quotient; 192 complete box orbits with 315 total memberships;
648 ordered closure-vertex references; 72 positive-axis packets; seam counts
12, 8, and 6; exactly two distinct incident cells per face; six oriented face
incidences per cell; connected periodic adjacency; and zero exterior faces.
Orbit sizes one, two, four, and eight all occur, while every cell orbit remains
singleton. Every local orientation is identity.

For a seam normal to axis `d`, the positive packet belongs to the
upper-adjacent cell, its neighbor is the lower-adjacent cell lifted by the
positive parent period, and its centre distance is exactly
`(h_last + h_first) / 2`. Ordered owner and lifted-neighbor face points agree
exactly; there is no geometric tolerance.

The expected package is Rust structure and assertions in
`crates/eqiora-numerics/src/cartesian_periodic_3d/tests.rs`. This directory
contains no JSON golden, producer table, numerical field, fitted value, or
tolerance. The Model, Transaction, mesh, Result, and public facade inventories
contain no persisted quotient, orbit, or packet state.
