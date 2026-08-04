# Frozen observations

The installed common `Result` must expose the accepted two-state `Trajectory`:
9 coordinates, 8 affine triangles, complete MINI vertex and bubble velocity,
fluid P1 pressure support, and solid displacement. Typed fixed-mesh monolithic
evidence must expose the exhaustive fluid/solid/interface partition and select
each state's interface action, energy, acceptance, solver, and assembly
observations by its exact `TrajectoryState`. All arrays are co-indexed,
memoized, read-only, owner-independent, and finite.

Presentation remains owned by the common trajectory-field-stills case. No
image bytes or scientific values are frozen by this adapter case.
