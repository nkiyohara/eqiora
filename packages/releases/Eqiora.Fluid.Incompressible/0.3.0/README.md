# Eqiora.Fluid.Incompressible 0.3.0

This immutable release preserves the steady two-dimensional Newtonian law and
complete-exterior mechanical interface from 0.2.0. It adds two independent
three-dimensional semantic Components:

- `ConservativeNavierStokesWithPotential3d` owns transient conservative
  momentum, incompressibility, and their physical Parameters;
- `NewtonianMechanicalInterface3d` binds velocity trace and parent-outward
  Cauchy traction to the exact
  `Eqiora.Mechanics.Interfaces@0.2.0::VelocityTractionBoundary` connector.

The conservative outer product is declared as an ordinary pure operator and
lowers with the Component's Relations. This package owns no ALE map, mesh
velocity, element, quadrature, stabilization, pressure gauge, time method,
nonlinear iteration, coupling algorithm, solver, target, or schedule.
