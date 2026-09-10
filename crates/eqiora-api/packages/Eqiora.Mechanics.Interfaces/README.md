# Eqiora.Mechanics.Interfaces

`VelocityTractionBoundary` pairs spatial velocity with parent-outward traction
through Euclidean boundary duality. It represents a power-conjugate boundary,
distinct from the displacement/traction connector used by quasistatic solids.

`ZeroVelocity2d`, `ZeroTraction2d`, `ZeroVelocity3d`, and `ZeroTraction3d`
prescribe zero trace or flux on an occurrence-bound face. The enclosing Model
supplies the body and boundary; the Realization selects numerical treatment.
