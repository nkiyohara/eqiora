# Eqiora.Mechanics.Interfaces 0.2.0

This immutable release preserves the method-neutral
`VelocityTractionBoundary` connector and the exact two-dimensional terminals
from 0.1.0. It adds `ZeroVelocity3d` and `ZeroTraction3d` as the same semantic
trace and flux laws over an occurrence-bound three-dimensional body's face.

The connector remains dimension-parametric: each occurrence receives its
spatial-vector extent from the exact parent Domain. The terminals prescribe
only zero velocity or zero parent-outward traction. They select no mesh,
quadrature, elimination, time method, coupling algorithm, solver, target, or
schedule.
