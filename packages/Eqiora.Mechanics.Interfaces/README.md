# Eqiora.Mechanics.Interfaces

A deliberately small, method-neutral package for one translational mechanical
boundary. `VelocityTractionBoundary` pairs a spatial velocity trace with its
parent-outward traction. Their Euclidean boundary duality is power-conjugate;
it is intentionally distinct from the displacement/traction virtual-work
connector in `Eqiora.Solid.LinearElasticity`.

`ZeroVelocity2d` and `ZeroTraction2d` are exact semantic terminals over one
occurrence-bound face. They prescribe only a zero trace or zero flux. They do
not select an element, boundary quadrature, elimination rule, pressure gauge,
solver, transfer operator, schedule, or coupling algorithm.

Version `0.1.0` does not define displacement-to-velocity conversion, structural
dynamics, moving geometry, ALE, nonzero data, or FSI. A physical law that uses
this Connector must state its own trace and traction Relations explicitly.
