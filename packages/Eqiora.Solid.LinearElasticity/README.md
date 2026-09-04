# Eqiora.Solid.LinearElasticity

A deliberately small, ordinary Model Package for reusable linear-elasticity
Relations. `IsotropicBalanceWithPotential2d` declares one intrinsic
two-dimensional volume support, two occurrence-bound continuum Fields, two
Lamé Parameters, and the isotropic small-strain balance Relation.

`IsotropicMechanicalInterface2d` separately exposes the complete exact
exterior of one occurrence-bound body through
`QuasistaticMechanicalBoundary` Ports. It binds
the displacement trace and parent-outward isotropic traction to each Port.
The Connector types the displacement/traction dimensions, spatial-vector
shape, Cartesian frame, and Euclidean boundary duality. Each occurrence-bound
Port supplies the exact Boundary support; parent-outward orientation is
derived from that Boundary's `BoundaryOf` relation. For quasistatic
displacement, the pairing represents virtual work.

`FixedDisplacement2d` and `ZeroTraction2d` prescribe zero complete displacement
trace or zero parent-outward traction on one occurrence-bound Boundary. An
enclosing Model supplies the exact body, complete boundary set, occurrence
Fields, loads, and terminal connections. Mesh, element, quadrature, solver,
target, and schedule remain ordinary Realization choices.

Version `0.4.0` adds first-order small-strain dynamics. The
`IsotropicElastodynamicsWithPotential2d` Component owns the two canonical
Relations

```text
derivative(displacement) - velocity = 0
density * derivative(velocity) - div(stress(displacement))
  - grad(load_potential) = 0
```

for one mass-density Parameter. The package declaration types that Parameter;
the canonical lowered subset admits it only after proving a finite, strictly
positive value. Its separate
`ElastodynamicMechanicalInterface2d` Component binds the velocity trace and
parent-outward elastic traction to the exact
`Eqiora.Mechanics.Interfaces@0.1.0::VelocityTractionBoundary` Connector. This
is a power-conjugate dynamic boundary, distinct from the quasistatic Connector.

The enclosing Model and Realization supply initial fields, spatial and time
discretization, solver, and execution target.
