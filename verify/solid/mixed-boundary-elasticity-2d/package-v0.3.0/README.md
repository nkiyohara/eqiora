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
displacement, the pairing represents virtual work rather than an implicit
claim about velocity power.

The balance and interface Components own no Domain, Field, load definition,
or prescribed boundary data. `FixedDisplacement2d` and `ZeroTraction2d` own
only the exact zero semantic boundary laws: they prescribe zero complete
displacement trace or zero parent-outward traction on one occurrence-bound
Boundary. No Component owns a mesh, element, quadrature rule, assembly policy,
solver, execution target, or schedule. An enclosing Model supplies the exact
body, its explicit complete boundary set, occurrence Fields, loads, and
terminal connections, then realizes the ordinary flattened Relation network
independently. The package has no privileged compiler or execution path.

Version `0.3.0` intentionally does not claim a live coupled boundary
Realization, nonzero prescribed boundary data, arbitrary boundary subsets,
plane-stress or plane-strain reduction, anisotropy, three dimensions,
nonlinear kinematics, dynamics, contact, or a broad solid-mechanics library.
Those features must extend the same Model/Realization boundary rather than
enter through package-specific numerical behavior.
