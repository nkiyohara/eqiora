# Eqiora.Solid.LinearElasticity 0.5.0

This immutable release preserves every two-dimensional declaration from 0.4.0
and adds the method-neutral three-dimensional dynamic pair required by the
first tetrahedral FSI slice.

`IsotropicElastodynamicsWithPotential3d` owns small-strain kinematics,
isotropic momentum balance, and the density and Lamé Parameters over one exact
three-dimensional volume. `ElastodynamicMechanicalInterface3d` separately
binds velocity trace and parent-outward elastic traction to
`Eqiora.Mechanics.Interfaces@0.2.0::VelocityTractionBoundary` over the complete
exterior.

The release owns no initial state, element, quadrature, mass treatment, time
method, ALE map, mesh-motion law, interface transfer, coupling algorithm,
nonlinear iteration, solver, target, or schedule.
